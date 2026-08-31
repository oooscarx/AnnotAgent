use std::{collections::BTreeMap, time::Duration};

use annotagent_core::{
    CoreError, CoreResult, ModelCapabilities, ModelMessage, ModelRequest, ModelResponse, ModelRole,
    ModelToolCall, TokenUsage, ToolCallId, UsageSource, VisionModelProvider,
};
use async_trait::async_trait;
use reqwest::{Client, StatusCode, header::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiProtocol {
    ChatCompletions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenAiCompatibleConfig {
    pub endpoint: String,
    pub api_key_env: String,
    pub model: String,
    pub protocol: OpenAiProtocol,
    #[serde(default = "default_timeout")]
    pub request_timeout_seconds: u64,
    #[serde(default = "default_output_tokens")]
    pub max_output_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default)]
    pub reasoning_mode: Option<String>,
    #[serde(default = "default_true")]
    pub supports_tool_calls: bool,
    #[serde(default)]
    pub supports_json_schema: bool,
    #[serde(default)]
    pub custom_headers: BTreeMap<String, String>,
    #[serde(default)]
    pub extra_request_fields: BTreeMap<String, Value>,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
}

const fn default_timeout() -> u64 {
    120
}
const fn default_output_tokens() -> u32 {
    4096
}
const fn default_temperature() -> f32 {
    0.1
}
const fn default_true() -> bool {
    true
}
const fn default_retries() -> u32 {
    2
}

pub struct OpenAiCompatibleProvider {
    config: OpenAiCompatibleConfig,
    client: Client,
    temporary_api_key: Option<String>,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: OpenAiCompatibleConfig) -> CoreResult<Self> {
        Self::new_with_api_key(config, None)
    }

    pub fn new_with_api_key(
        config: OpenAiCompatibleConfig,
        temporary_api_key: Option<String>,
    ) -> CoreResult<Self> {
        validate_openai_config(&config)?;
        let client = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_seconds))
            .build()
            .map_err(|error| CoreError::Provider(format!("cannot build HTTP client: {error}")))?;
        Ok(Self {
            config,
            client,
            temporary_api_key,
        })
    }

    #[must_use]
    pub fn endpoint_summary(&self) -> String {
        reqwest::Url::parse(&self.config.endpoint).map_or_else(
            |_| "invalid-endpoint".to_owned(),
            |url| {
                format!(
                    "{}://{}{}",
                    url.scheme(),
                    url.host_str().unwrap_or("unknown-host"),
                    url.port()
                        .map_or_else(String::new, |port| format!(":{port}"))
                )
            },
        )
    }

    fn endpoint_url(&self) -> String {
        format!(
            "{}/chat/completions",
            self.config.endpoint.trim_end_matches('/')
        )
    }

    fn request_body(&self, request: &ModelRequest) -> Value {
        let mut messages: Vec<Value> = request.messages.iter().map(message_json).collect();
        if !request.images.is_empty() {
            let image_parts = request.images.iter().map(|image| {
                json!({
                    "type": "image_url",
                    "image_url": {"url": format!("data:{};base64,{}", image.mime_type, image.data_base64)}
                })
            });
            if let Some(user_message) = messages
                .iter_mut()
                .rev()
                .find(|message| message.get("role").and_then(Value::as_str) == Some("user"))
            {
                let text = user_message
                    .get_mut("content")
                    .map(Value::take)
                    .and_then(|content| content.as_str().map(str::to_owned))
                    .unwrap_or_default();
                let mut content = vec![json!({"type": "text", "text": text})];
                content.extend(image_parts);
                user_message["content"] = Value::Array(content);
            } else {
                let mut content = vec![
                    json!({"type": "text", "text": "Inspect the attached controlled image inputs."}),
                ];
                content.extend(image_parts);
                messages.push(json!({"role": "user", "content": content}));
            }
        }
        let mut body = serde_json::Map::new();
        body.insert("model".to_owned(), json!(request.model));
        body.insert("messages".to_owned(), Value::Array(messages));
        body.insert("max_tokens".to_owned(), json!(request.max_output_tokens));
        body.insert("temperature".to_owned(), json!(request.temperature));
        let native_tool_mode = self.config.supports_tool_calls && !request.tools.is_empty();
        if native_tool_mode {
            body.insert(
                "tools".to_owned(),
                Value::Array(
                    request
                        .tools
                        .iter()
                        .map(|tool| {
                            json!({
                                "type": "function",
                                "function": {
                                    "name": tool.name,
                                    "description": tool.description,
                                    "parameters": tool.parameters
                                }
                            })
                        })
                        .collect(),
                ),
            );
        }
        if self.config.supports_json_schema && !native_tool_mode {
            body.insert(
                "response_format".to_owned(),
                json!({
                    "type": "json_schema",
                    "json_schema": {
                        "name": "annotagent_constrained_action",
                        "strict": true,
                        "schema": json_action_schema(&request.tools)
                    }
                }),
            );
        }
        if let Some(mode) = &self.config.reasoning_mode
            && !request.extra.contains_key("enable_thinking")
        {
            body.insert("reasoning_effort".to_owned(), json!(mode));
        }
        for (key, value) in &self.config.extra_request_fields {
            body.insert(key.clone(), value.clone());
        }
        for (key, value) in &request.extra {
            body.insert(key.clone(), value.clone());
        }
        Value::Object(body)
    }
}

fn validate_openai_config(config: &OpenAiCompatibleConfig) -> CoreResult<()> {
    let endpoint = reqwest::Url::parse(&config.endpoint)
        .map_err(|error| CoreError::Validation(format!("invalid provider endpoint: {error}")))?;
    if !matches!(endpoint.scheme(), "http" | "https")
        || endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
    {
        return Err(CoreError::Validation(
            "provider endpoint must be an http(s) URL without embedded credentials".to_owned(),
        ));
    }
    if !valid_environment_name(&config.api_key_env) {
        return Err(CoreError::Validation(
            "api_key_env must be a valid environment variable name".to_owned(),
        ));
    }
    if let Some(name) = config
        .custom_headers
        .keys()
        .find(|name| secret_key_name(name))
    {
        return Err(CoreError::Validation(format!(
            "custom header {name:?} may contain credentials; use the write-only API key field"
        )));
    }
    if let Some(path) = find_secret_key(&config.extra_request_fields) {
        return Err(CoreError::Validation(format!(
            "extra request field {path:?} may contain secret material"
        )));
    }
    Ok(())
}

fn valid_environment_name(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn secret_key_name(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase().replace(['-', '_'], "");
    matches!(
        normalized.as_str(),
        "authorization"
            | "proxyauthorization"
            | "apikey"
            | "accesstoken"
            | "secrettoken"
            | "password"
    )
}

fn find_secret_key(fields: &BTreeMap<String, Value>) -> Option<String> {
    fn visit(value: &Value, path: &str) -> Option<String> {
        match value {
            Value::Object(object) => object.iter().find_map(|(key, value)| {
                let nested = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                secret_key_name(key)
                    .then_some(nested.clone())
                    .or_else(|| visit(value, &nested))
            }),
            Value::Array(values) => values
                .iter()
                .enumerate()
                .find_map(|(index, value)| visit(value, &format!("{path}[{index}]"))),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
        }
    }
    fields.iter().find_map(|(key, value)| {
        secret_key_name(key)
            .then_some(key.clone())
            .or_else(|| visit(value, key))
    })
}

fn message_json(message: &ModelMessage) -> Value {
    let role = match message.role {
        ModelRole::System => "system",
        ModelRole::User => "user",
        ModelRole::Assistant => "assistant",
        ModelRole::Tool => "tool",
    };
    let content = if message.role == ModelRole::Assistant
        && message.content.is_empty()
        && !message.tool_calls.is_empty()
    {
        Value::Null
    } else {
        json!(message.content)
    };
    let mut value = json!({"role": role, "content": content});
    if let Some(call_id) = &message.tool_call_id {
        value["tool_call_id"] = json!(call_id.as_str());
    }
    if !message.tool_calls.is_empty() {
        value["tool_calls"] = Value::Array(
            message
                .tool_calls
                .iter()
                .map(|call| {
                    json!({
                        "id": call.id.as_str(),
                        "type": "function",
                        "function": {
                            "name": call.name,
                            "arguments": call.arguments.to_string()
                        }
                    })
                })
                .collect(),
        );
    }
    value
}

#[async_trait]
impl VisionModelProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &str {
        "openai_compatible"
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            vision: true,
            tool_calls: self.config.supports_tool_calls,
            json_schema: self.config.supports_json_schema,
            usage_reporting: true,
            multi_image: true,
        }
    }

    async fn complete(
        &self,
        mut request: ModelRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<ModelResponse> {
        request.model.clone_from(&self.config.model);
        request.max_output_tokens = request.max_output_tokens.min(self.config.max_output_tokens);
        let key = self.temporary_api_key.clone().map_or_else(
            || {
                std::env::var(&self.config.api_key_env).map_err(|_| {
                    CoreError::Provider(format!(
                        "API key environment variable {:?} is not set",
                        self.config.api_key_env
                    ))
                })
            },
            Ok,
        )?;
        let body = self.request_body(&request);
        for attempt in 0..=self.config.max_retries {
            let mut builder = self
                .client
                .post(self.endpoint_url())
                .bearer_auth(&key)
                .json(&body);
            let mut headers = HeaderMap::new();
            for (name, value) in &self.config.custom_headers {
                let name =
                    reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                        CoreError::Provider(format!("invalid custom header name: {error}"))
                    })?;
                let value = reqwest::header::HeaderValue::from_str(value).map_err(|error| {
                    CoreError::Provider(format!("invalid custom header value: {error}"))
                })?;
                headers.insert(name, value);
            }
            builder = builder.headers(headers);
            let response = tokio::select! {
                () = cancellation.cancelled() => {
                    return Err(CoreError::Provider("model request cancelled".to_owned()));
                }
                result = builder.send() => result.map_err(|error| {
                    CoreError::Provider(format!("request to {} failed: {error}", self.endpoint_summary()))
                })?,
            };
            let status = response.status();
            let request_id = response
                .headers()
                .get("x-request-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            if is_retriable(status) && attempt < self.config.max_retries {
                let delay = Duration::from_millis(250_u64.saturating_mul(1_u64 << attempt));
                tokio::select! {
                    () = cancellation.cancelled() => {
                        return Err(CoreError::Provider("model request cancelled".to_owned()));
                    }
                    () = tokio::time::sleep(delay) => {}
                }
                continue;
            }
            let bytes = response.bytes().await.map_err(|error| {
                CoreError::Provider(format!("cannot read provider response: {error}"))
            })?;
            if !status.is_success() {
                let safe = String::from_utf8_lossy(&bytes).replace(&key, "[REDACTED]");
                return Err(CoreError::Provider(format!(
                    "provider returned {status}: {}",
                    truncate(&safe, 500)
                )));
            }
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|error| CoreError::Provider(format!("invalid provider JSON: {error}")))?;
            let mut parsed = parse_chat_response(&value, request_id)?;
            parsed
                .provider_metadata
                .insert("retry_count".to_owned(), attempt.to_string());
            if self.config.supports_tool_calls {
                try_promote_json_action(&mut parsed, &request.tools)?;
            } else {
                promote_json_action(&mut parsed, &request.tools)?;
            }
            return Ok(parsed);
        }
        Err(CoreError::Provider("provider retries exhausted".to_owned()))
    }
}

fn is_retriable(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn json_action_schema(tools: &[annotagent_core::ToolDefinition]) -> Value {
    if tools.is_empty() {
        return json!({"type": "object", "additionalProperties": false});
    }
    let actions = tools
        .iter()
        .map(|tool| {
            json!({
                "type": "object",
                "properties": {
                    "name": {"const": tool.name},
                    "arguments": tool.parameters,
                },
                "required": ["name", "arguments"],
                "additionalProperties": false,
            })
        })
        .collect::<Vec<_>>();
    json!({"oneOf": actions})
}

fn promote_json_action(
    response: &mut ModelResponse,
    tools: &[annotagent_core::ToolDefinition],
) -> CoreResult<()> {
    if !response.tool_calls.is_empty() || tools.is_empty() {
        return Ok(());
    }
    let content = response.content.as_deref().ok_or_else(|| {
        CoreError::Provider("JSON-only provider returned no constrained action".to_owned())
    })?;
    let action: Value = serde_json::from_str(content)
        .map_err(|error| CoreError::Provider(format!("invalid JSON-only action: {error}")))?;
    promote_action_value(response, tools, &action)
}

fn try_promote_json_action(
    response: &mut ModelResponse,
    tools: &[annotagent_core::ToolDefinition],
) -> CoreResult<bool> {
    if !response.tool_calls.is_empty() || tools.is_empty() {
        return Ok(false);
    }
    let Some(content) = response.content.as_deref() else {
        return Ok(false);
    };
    let Ok(action) = serde_json::from_str::<Value>(content) else {
        return Ok(false);
    };
    if action.get("name").is_none() || action.get("arguments").is_none() {
        return Ok(false);
    }
    promote_action_value(response, tools, &action)?;
    Ok(true)
}

fn promote_action_value(
    response: &mut ModelResponse,
    tools: &[annotagent_core::ToolDefinition],
    action: &Value,
) -> CoreResult<()> {
    let name = action.get("name").and_then(Value::as_str).ok_or_else(|| {
        CoreError::Provider("JSON-only action lacks string field `name`".to_owned())
    })?;
    if !tools.iter().any(|tool| tool.name == name) {
        return Err(CoreError::Provider(format!(
            "JSON-only action selected unregistered tool {name:?}"
        )));
    }
    let arguments = action
        .get("arguments")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| {
            CoreError::Provider("JSON-only action lacks object field `arguments`".to_owned())
        })?;
    response.tool_calls.push(ModelToolCall {
        id: ToolCallId::new(format!("json-action-{}", uuid::Uuid::new_v4())),
        name: name.to_owned(),
        arguments,
    });
    Ok(())
}

fn parse_chat_response(value: &Value, request_id: Option<String>) -> CoreResult<ModelResponse> {
    let message = value
        .pointer("/choices/0/message")
        .ok_or_else(|| CoreError::Provider("response lacks choices[0].message".to_owned()))?;
    let content = parse_message_content(message.get("content"));
    let tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .map(|call| {
                    let id = call
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("provider-call");
                    let function = call.get("function").unwrap_or(call);
                    let name = function
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or_else(|| CoreError::Provider("tool call lacks name".to_owned()))?;
                    let raw_arguments = function.get("arguments").cloned().unwrap_or(Value::Null);
                    let arguments = raw_arguments.as_str().map_or(raw_arguments.clone(), |raw| {
                        serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned()))
                    });
                    Ok(ModelToolCall {
                        id: ToolCallId::new(id),
                        name: name.to_owned(),
                        arguments,
                    })
                })
                .collect::<CoreResult<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();
    let input = value
        .pointer("/usage/prompt_tokens")
        .and_then(Value::as_u64);
    let output = value
        .pointer("/usage/completion_tokens")
        .and_then(Value::as_u64);
    let total = value.pointer("/usage/total_tokens").and_then(Value::as_u64);
    let source = match value.pointer("/usage/source").and_then(Value::as_str) {
        Some("estimated") => UsageSource::Estimated,
        Some("actual") => UsageSource::Actual,
        _ if input.is_some() || output.is_some() => UsageSource::Actual,
        _ => UsageSource::Unknown,
    };
    Ok(ModelResponse {
        content,
        tool_calls,
        usage: TokenUsage {
            input_tokens: input,
            output_tokens: output,
            total_tokens: total.or_else(|| Some(input?.saturating_add(output?))),
            source,
        },
        request_id,
        provider_metadata: BTreeMap::new(),
    })
}

fn parse_message_content(content: Option<&Value>) -> Option<String> {
    let content = content?;
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(Value::as_str)
                    .or_else(|| part.get("content").and_then(Value::as_str))
            })
            .collect::<Vec<_>>()
            .join(""),
        Value::Object(part) => part
            .get("text")
            .and_then(Value::as_str)
            .or_else(|| part.get("content").and_then(Value::as_str))
            .unwrap_or_default()
            .to_owned(),
        Value::Null | Value::Bool(_) | Value::Number(_) => String::new(),
    };
    (!text.trim().is_empty()).then_some(text)
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

/// Redacts common secret-bearing fields before structured values reach logs or traces.
#[must_use]
pub fn redact_secrets(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let lower = key.to_ascii_lowercase();
                    if lower.contains("authorization")
                        || lower.contains("api_key")
                        || lower.contains("apikey")
                    {
                        (key.clone(), Value::String("[REDACTED]".to_owned()))
                    } else if lower.contains("base64") || lower == "data_url" {
                        (key.clone(), Value::String("[BINARY OMITTED]".to_owned()))
                    } else {
                        (key.clone(), redact_secrets(value))
                    }
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(redact_secrets).collect()),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tool_calls_and_usage() {
        let response = parse_chat_response(
            &json!({
                "choices": [{"message": {"tool_calls": [{
                    "id": "c1", "function": {"name": "inspect", "arguments": "{\"id\":1}"}
                }]}}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
            }),
            Some("request-1".to_owned()),
        )
        .expect("valid response");
        assert_eq!(response.tool_calls[0].arguments["id"], 1);
        assert_eq!(response.usage.total_tokens, Some(15));
    }

    #[test]
    fn parses_openai_content_part_arrays() {
        let response = parse_chat_response(
            &json!({
                "choices": [{"message": {"content": [
                    {"type": "output_text", "text": "{\"detections\":"},
                    {"type": "output_text", "text": "[]} "}
                ]}}]
            }),
            None,
        )
        .expect("valid response");
        assert_eq!(response.content.as_deref(), Some("{\"detections\":[]} "));
    }

    #[test]
    fn serializes_assistant_tool_call_history_for_follow_up_turns() {
        let message = ModelMessage {
            role: ModelRole::Assistant,
            content: String::new(),
            tool_call_id: None,
            tool_calls: vec![ModelToolCall {
                id: ToolCallId::new("call-1"),
                name: "refine_line".to_owned(),
                arguments: json!({"points": [[0.1, 0.2], [0.8, 0.2]]}),
            }],
        };
        let value = message_json(&message);
        assert!(value["content"].is_null());
        assert_eq!(value["tool_calls"][0]["id"], "call-1");
        assert_eq!(value["tool_calls"][0]["function"]["name"], "refine_line");
        assert_eq!(
            value["tool_calls"][0]["function"]["arguments"],
            json!({"points": [[0.1, 0.2], [0.8, 0.2]]}).to_string()
        );
    }

    #[test]
    fn attaches_images_to_the_grounding_prompt_user_message() {
        let provider = OpenAiCompatibleProvider::new_with_api_key(
            OpenAiCompatibleConfig {
                endpoint: "https://provider.invalid/v1".to_owned(),
                api_key_env: "UNUSED_TEST_KEY".to_owned(),
                model: "vision".to_owned(),
                protocol: OpenAiProtocol::ChatCompletions,
                request_timeout_seconds: 1,
                max_output_tokens: 100,
                temperature: 0.0,
                reasoning_mode: None,
                supports_tool_calls: true,
                supports_json_schema: false,
                custom_headers: BTreeMap::new(),
                extra_request_fields: BTreeMap::new(),
                max_retries: 0,
            },
            Some("not-sent".to_owned()),
        )
        .expect("provider");
        let body = provider.request_body(&ModelRequest {
            model: "ignored".to_owned(),
            task_id: annotagent_core::TaskId::from("grounding"),
            messages: vec![
                ModelMessage {
                    role: ModelRole::System,
                    content: "system".to_owned(),
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                },
                ModelMessage {
                    role: ModelRole::User,
                    content: "locate the football".to_owned(),
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                },
            ],
            images: vec![annotagent_core::ModelImage {
                id: "image-1".to_owned(),
                mime_type: "image/png".to_owned(),
                data_base64: "aW1hZ2U=".to_owned(),
            }],
            tools: Vec::new(),
            max_output_tokens: 100,
            temperature: 0.0,
            extra: BTreeMap::new(),
        });
        assert_eq!(body["messages"].as_array().map(Vec::len), Some(2));
        assert_eq!(
            body["messages"][1]["content"][0]["text"],
            "locate the football"
        );
        assert_eq!(body["messages"][1]["content"][1]["type"], "image_url");
        assert_eq!(
            body["messages"][1]["content"][1]["image_url"]["url"],
            "data:image/png;base64,aW1hZ2U="
        );
    }

    #[test]
    fn request_level_thinking_switch_suppresses_conflicting_reasoning_effort() {
        let provider = OpenAiCompatibleProvider::new_with_api_key(
            OpenAiCompatibleConfig {
                endpoint: "https://provider.invalid/v1".to_owned(),
                api_key_env: "UNUSED_TEST_KEY".to_owned(),
                model: "vision".to_owned(),
                protocol: OpenAiProtocol::ChatCompletions,
                request_timeout_seconds: 1,
                max_output_tokens: 100,
                temperature: 0.0,
                reasoning_mode: Some("medium".to_owned()),
                supports_tool_calls: true,
                supports_json_schema: false,
                custom_headers: BTreeMap::new(),
                extra_request_fields: BTreeMap::new(),
                max_retries: 0,
            },
            Some("not-sent".to_owned()),
        )
        .expect("provider");
        let body = provider.request_body(&ModelRequest {
            model: "ignored".to_owned(),
            task_id: annotagent_core::TaskId::from("grounding"),
            messages: vec![ModelMessage {
                role: ModelRole::User,
                content: "return JSON".to_owned(),
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            images: Vec::new(),
            tools: Vec::new(),
            max_output_tokens: 100,
            temperature: 0.0,
            extra: BTreeMap::from([("enable_thinking".to_owned(), json!(false))]),
        });
        assert_eq!(body["enable_thinking"], false);
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn secrets_and_image_payloads_are_redacted() {
        let value =
            redact_secrets(&json!({"Authorization": "Bearer secret", "data_base64": "huge"}));
        assert_eq!(value["Authorization"], "[REDACTED]");
        assert_eq!(value["data_base64"], "[BINARY OMITTED]");
    }

    #[test]
    fn provider_configuration_rejects_credential_bearing_metadata_and_urls() {
        let base = OpenAiCompatibleConfig {
            endpoint: "https://provider.example/v1".to_owned(),
            api_key_env: "ANNOTAGENT_API_KEY".to_owned(),
            model: "vision".to_owned(),
            protocol: OpenAiProtocol::ChatCompletions,
            request_timeout_seconds: 1,
            max_output_tokens: 100,
            temperature: 0.0,
            reasoning_mode: None,
            supports_tool_calls: true,
            supports_json_schema: false,
            custom_headers: BTreeMap::new(),
            extra_request_fields: BTreeMap::new(),
            max_retries: 0,
        };
        let mut header = base.clone();
        header
            .custom_headers
            .insert("Authorization".to_owned(), "Bearer plaintext".to_owned());
        assert!(OpenAiCompatibleProvider::new(header).is_err());

        let mut extra = base.clone();
        extra
            .extra_request_fields
            .insert("vendor".to_owned(), json!({"access_token": "plaintext"}));
        assert!(OpenAiCompatibleProvider::new(extra).is_err());

        let mut embedded = base;
        embedded.endpoint = "https://user:pass@provider.example/v1".to_owned();
        assert!(OpenAiCompatibleProvider::new(embedded).is_err());
    }

    #[test]
    fn json_only_mode_emits_a_constrained_response_schema_without_tools() {
        let provider = OpenAiCompatibleProvider::new_with_api_key(
            OpenAiCompatibleConfig {
                endpoint: "https://provider.invalid/v1".to_owned(),
                api_key_env: "UNUSED_TEST_KEY".to_owned(),
                model: "json-vision".to_owned(),
                protocol: OpenAiProtocol::ChatCompletions,
                request_timeout_seconds: 1,
                max_output_tokens: 100,
                temperature: 0.0,
                reasoning_mode: None,
                supports_tool_calls: false,
                supports_json_schema: true,
                custom_headers: BTreeMap::new(),
                extra_request_fields: BTreeMap::new(),
                max_retries: 0,
            },
            Some("not-sent".to_owned()),
        )
        .expect("provider");
        let body = provider.request_body(&ModelRequest {
            model: "ignored".to_owned(),
            task_id: annotagent_core::TaskId::from("classification"),
            messages: vec![ModelMessage {
                role: ModelRole::User,
                content: "return one action".to_owned(),
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            images: Vec::new(),
            tools: vec![annotagent_core::ToolDefinition {
                name: "submit".to_owned(),
                description: "submit".to_owned(),
                parameters: json!({"type": "object"}),
                read_only: false,
            }],
            max_output_tokens: 100,
            temperature: 0.0,
            extra: BTreeMap::new(),
        });
        assert!(body.get("tools").is_none());
        assert_eq!(body["response_format"]["type"], "json_schema");
        assert_eq!(body["response_format"]["json_schema"]["strict"], true);
        assert_eq!(
            body["response_format"]["json_schema"]["schema"]["oneOf"][0]["properties"]["name"]["const"],
            "submit"
        );
        assert_eq!(
            body["response_format"]["json_schema"]["schema"]["oneOf"][0]["additionalProperties"],
            false
        );

        let mut response = ModelResponse {
            content: Some(json!({"name": "submit", "arguments": {}}).to_string()),
            tool_calls: Vec::new(),
            usage: TokenUsage {
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                source: UsageSource::Unknown,
            },
            request_id: Some("json-request".to_owned()),
            provider_metadata: BTreeMap::new(),
        };
        promote_json_action(
            &mut response,
            &[annotagent_core::ToolDefinition {
                name: "submit".to_owned(),
                description: "submit".to_owned(),
                parameters: json!({"type": "object"}),
                read_only: false,
            }],
        )
        .expect("constrained action");
        assert_eq!(response.tool_calls[0].name, "submit");
    }

    #[test]
    fn native_tool_mode_requires_a_tool_without_conflicting_json_schema() {
        let provider = OpenAiCompatibleProvider::new_with_api_key(
            OpenAiCompatibleConfig {
                endpoint: "https://provider.invalid/v1".to_owned(),
                api_key_env: "UNUSED_TEST_KEY".to_owned(),
                model: "tool-vision".to_owned(),
                protocol: OpenAiProtocol::ChatCompletions,
                request_timeout_seconds: 1,
                max_output_tokens: 100,
                temperature: 0.0,
                reasoning_mode: None,
                supports_tool_calls: true,
                supports_json_schema: true,
                custom_headers: BTreeMap::new(),
                extra_request_fields: BTreeMap::new(),
                max_retries: 0,
            },
            Some("not-sent".to_owned()),
        )
        .expect("provider");
        let tools = vec![annotagent_core::ToolDefinition {
            name: "submit".to_owned(),
            description: "submit".to_owned(),
            parameters: json!({"type": "object"}),
            read_only: false,
        }];
        let body = provider.request_body(&ModelRequest {
            model: "ignored".to_owned(),
            task_id: annotagent_core::TaskId::from("objects"),
            messages: vec![ModelMessage {
                role: ModelRole::User,
                content: "submit one action".to_owned(),
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            images: Vec::new(),
            tools: tools.clone(),
            max_output_tokens: 100,
            temperature: 0.0,
            extra: BTreeMap::new(),
        });
        assert!(body["tools"].is_array());
        assert!(body.get("tool_choice").is_none());
        assert!(body.get("response_format").is_none());

        let mut response = ModelResponse {
            content: Some(json!({"name": "submit", "arguments": {}}).to_string()),
            tool_calls: Vec::new(),
            usage: TokenUsage {
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                source: UsageSource::Unknown,
            },
            request_id: Some("content-fallback".to_owned()),
            provider_metadata: BTreeMap::new(),
        };
        assert!(try_promote_json_action(&mut response, &tools).expect("fallback action"));
        assert_eq!(response.tool_calls[0].name, "submit");
    }

    #[test]
    fn usage_source_preserves_provider_estimates() {
        let response = parse_chat_response(
            &json!({
                "choices": [{"message": {"content": "ok"}}],
                "usage": {"prompt_tokens": 8, "completion_tokens": 3, "source": "estimated"}
            }),
            None,
        )
        .expect("response");
        assert_eq!(response.usage.source, UsageSource::Estimated);
    }
}
