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
            let mut content =
                vec![json!({"type": "text", "text": "Attached controlled image inputs."})];
            content.extend(request.images.iter().map(|image| {
                json!({
                    "type": "image_url",
                    "image_url": {"url": format!("data:{};base64,{}", image.mime_type, image.data_base64)}
                })
            }));
            messages.push(json!({"role": "user", "content": content}));
        }
        let mut body = serde_json::Map::new();
        body.insert("model".to_owned(), json!(request.model));
        body.insert("messages".to_owned(), Value::Array(messages));
        body.insert("max_tokens".to_owned(), json!(request.max_output_tokens));
        body.insert("temperature".to_owned(), json!(request.temperature));
        if self.config.supports_tool_calls && !request.tools.is_empty() {
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
        if let Some(mode) = &self.config.reasoning_mode {
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
                let safe = String::from_utf8_lossy(&bytes);
                return Err(CoreError::Provider(format!(
                    "provider returned {status}: {}",
                    truncate(&safe, 500)
                )));
            }
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|error| CoreError::Provider(format!("invalid provider JSON: {error}")))?;
            return parse_chat_response(&value, request_id);
        }
        Err(CoreError::Provider("provider retries exhausted".to_owned()))
    }
}

fn is_retriable(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn parse_chat_response(value: &Value, request_id: Option<String>) -> CoreResult<ModelResponse> {
    let message = value
        .pointer("/choices/0/message")
        .ok_or_else(|| CoreError::Provider("response lacks choices[0].message".to_owned()))?;
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .map(str::to_owned);
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
    let source = if input.is_some() || output.is_some() {
        UsageSource::Actual
    } else {
        UsageSource::Unknown
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
    fn secrets_and_image_payloads_are_redacted() {
        let value =
            redact_secrets(&json!({"Authorization": "Bearer secret", "data_base64": "huge"}));
        assert_eq!(value["Authorization"], "[REDACTED]");
        assert_eq!(value["data_base64"], "[BINARY OMITTED]");
    }
}
