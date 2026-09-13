use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, RwLock},
    time::Duration,
};

use annotagent_core::{
    CoreError, CoreResult, ModelCapabilities, ModelMessage, ModelRequest, ModelResponse, ModelRole,
    ModelToolCall, TokenUsage, ToolCallId, UsageSource, VisionModelProvider,
};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{Client, StatusCode, header::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiProtocol {
    ChatCompletions,
}

/// Wire-level constrained-output mode selected before an admitted request is sent.
/// `Automatic` preserves the legacy capability-driven behavior for non-Schema callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiResponseMode {
    #[default]
    Automatic,
    NativeTool,
    JsonObject,
    JsonSchema,
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
    /// Send and decode the OpenAI-compatible SSE transport. This is independent
    /// from the selected structured-output mode and must be declared by the
    /// frozen Model Profile.
    #[serde(default)]
    pub streaming: bool,
    #[serde(default)]
    pub response_mode: OpenAiResponseMode,
    #[serde(default)]
    pub custom_headers: BTreeMap<String, String>,
    #[serde(default)]
    pub extra_request_fields: BTreeMap<String, Value>,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    #[serde(default = "default_minimum_retry_delay_ms")]
    pub minimum_retry_delay_ms: u64,
    #[serde(default = "default_maximum_retry_delay_ms")]
    pub maximum_retry_delay_ms: u64,
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
const fn default_minimum_retry_delay_ms() -> u64 {
    250
}
const fn default_maximum_retry_delay_ms() -> u64 {
    5_000
}

pub struct OpenAiCompatibleProvider {
    config: OpenAiCompatibleConfig,
    client: Client,
    temporary_api_key: Option<String>,
    attempt_observer: Arc<RwLock<Option<Arc<dyn ModelAttemptObserver>>>>,
}

#[derive(Debug, Clone)]
pub struct ModelAttemptOutcome {
    pub status: ModelAttemptOutcomeStatus,
    pub request_id: Option<String>,
    pub usage: TokenUsage,
    pub cached_input_tokens: Option<u64>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
    pub failure: Option<annotagent_core::ModelFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelAttemptOutcomeStatus {
    Succeeded,
    Failed,
    InDoubt,
}

pub trait ModelAttemptObserver: Send + Sync {
    /// Must persist before transport begins. Returning an error prevents the HTTP request.
    fn begin(
        &self,
        call_id: Option<&str>,
        request: &ModelRequest,
        attempt_number: u32,
        started_at: chrono::DateTime<chrono::Utc>,
    ) -> CoreResult<String>;
    fn finish(&self, attempt_id: &str, outcome: &ModelAttemptOutcome) -> CoreResult<()>;
}

tokio::task_local! {
    static MODEL_CALL_ID: String;
}

/// Carries an already-admitted logical call identity to a concrete Provider transport.
/// The value is task-local, never serialized into the external request.
pub async fn within_model_call<T>(call_id: String, future: impl Future<Output = T>) -> T {
    MODEL_CALL_ID.scope(call_id, future).await
}

impl OpenAiCompatibleProvider {
    fn effective_response_mode(&self) -> OpenAiResponseMode {
        match self.config.response_mode {
            OpenAiResponseMode::Automatic if self.config.supports_tool_calls => {
                OpenAiResponseMode::NativeTool
            }
            OpenAiResponseMode::Automatic if self.config.supports_json_schema => {
                OpenAiResponseMode::JsonSchema
            }
            mode => mode,
        }
    }

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
            attempt_observer: Arc::new(RwLock::new(None)),
        })
    }

    #[must_use]
    pub fn with_attempt_observer(self, observer: Arc<dyn ModelAttemptObserver>) -> Self {
        self.set_attempt_observer(observer);
        self
    }

    pub fn set_attempt_observer(&self, observer: Arc<dyn ModelAttemptObserver>) {
        *self
            .attempt_observer
            .write()
            .expect("model attempt observer lock poisoned") = Some(observer);
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
        // Extras are extensions, never authority to replace the resolved model, limit,
        // messages, thinking control or constrained-output mode.
        let mut body = serde_json::Map::new();
        for (key, value) in &self.config.extra_request_fields {
            body.insert(key.clone(), value.clone());
        }
        for (key, value) in &request.extra {
            body.insert(key.clone(), value.clone());
        }
        // Transport is a typed capability. Arbitrary request extras cannot turn
        // it on for an endpoint whose frozen profile did not declare support.
        body.remove("stream");
        if self.config.streaming {
            body.insert("stream".to_owned(), Value::Bool(true));
        }
        body.insert("model".to_owned(), json!(request.model));
        body.insert("messages".to_owned(), Value::Array(messages));
        body.insert("max_tokens".to_owned(), json!(request.max_output_tokens));
        body.insert("temperature".to_owned(), json!(request.temperature));
        body.remove("tools");
        body.remove("tool_choice");
        body.remove("response_format");
        let response_mode = self.effective_response_mode();
        let native_tool_mode = response_mode == OpenAiResponseMode::NativeTool
            && self.config.supports_tool_calls
            && !request.tools.is_empty();
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
            // The common OpenAI-compatible auto-only subset accepts this value. We
            // never emit `required` or a named-tool selector.
            body.insert("tool_choice".to_owned(), json!("auto"));
        } else {
            body.remove("parallel_tool_calls");
        }
        if response_mode == OpenAiResponseMode::JsonObject {
            body.insert("response_format".to_owned(), json!({"type": "json_object"}));
        } else if response_mode == OpenAiResponseMode::JsonSchema
            && self.config.supports_json_schema
            && !native_tool_mode
        {
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
            && !self.config.extra_request_fields.contains_key("thinking")
        {
            body.insert("reasoning_effort".to_owned(), json!(mode));
        }
        // A typed provider/profile setting wins over request extras for thinking.
        if let Some(thinking) = self.config.extra_request_fields.get("thinking") {
            body.insert("thinking".to_owned(), thinking.clone());
            body.remove("reasoning_effort");
            body.remove("enable_thinking");
        } else if let Some(enable) = self.config.extra_request_fields.get("enable_thinking") {
            body.insert("enable_thinking".to_owned(), enable.clone());
            body.remove("reasoning_effort");
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
    if config.minimum_retry_delay_ms > config.maximum_retry_delay_ms
        || config.maximum_retry_delay_ms > 120_000
    {
        return Err(CoreError::Validation(
            "retry delays must be ordered and maximum_retry_delay_ms cannot exceed 120000"
                .to_owned(),
        ));
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
                    safe_failure(
                        annotagent_core::ModelFailureStage::PrepareRequest,
                        annotagent_core::ModelFailureCategory::Configuration,
                        None,
                    )
                })
            },
            Ok,
        )?;
        let body = self.request_body(&request);
        if body.get("stream").and_then(Value::as_bool) == Some(true) && !self.config.streaming {
            return Err(safe_failure(
                annotagent_core::ModelFailureStage::PrepareRequest,
                annotagent_core::ModelFailureCategory::Configuration,
                None,
            ));
        }
        for attempt in 0..=self.config.max_retries {
            let started_at = chrono::Utc::now();
            let observer = self
                .attempt_observer
                .read()
                .map_err(|_| CoreError::Provider("model attempt observer unavailable".into()))?
                .clone();
            let attempt_id = observer
                .as_ref()
                .map(|observer| {
                    let call_id = MODEL_CALL_ID.try_with(Clone::clone).ok();
                    observer.begin(call_id.as_deref(), &request, attempt + 1, started_at)
                })
                .transpose()?;
            let mut builder = self
                .client
                .post(self.endpoint_url())
                .bearer_auth(&key)
                .json(&body);
            let mut headers = HeaderMap::new();
            for (name, value) in &self.config.custom_headers {
                let name =
                    reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
                        safe_failure(
                            annotagent_core::ModelFailureStage::PrepareRequest,
                            annotagent_core::ModelFailureCategory::Configuration,
                            None,
                        )
                    })?;
                let value = reqwest::header::HeaderValue::from_str(value).map_err(|_| {
                    safe_failure(
                        annotagent_core::ModelFailureStage::PrepareRequest,
                        annotagent_core::ModelFailureCategory::Configuration,
                        None,
                    )
                })?;
                headers.insert(name, value);
            }
            builder = builder.headers(headers);
            let response = tokio::select! {
                () = cancellation.cancelled() => {
                    let failure=annotagent_core::ModelFailure{stage:annotagent_core::ModelFailureStage::ProviderRequest,category:annotagent_core::ModelFailureCategory::Cancelled,http_status:None};
                    self.finish_observed_attempt(attempt_id.as_deref(),ModelAttemptOutcomeStatus::InDoubt,None,TokenUsage{input_tokens:None,output_tokens:None,total_tokens:None,source:UsageSource::Unknown},None,Some(failure.clone()))?;
                    return Err(CoreError::ModelFailure(failure));
                }
                result = builder.send() => match result {
                    Ok(response)=>response,
                    Err(error)=>{
                        let failure=model_failure_for_transport(&error,annotagent_core::ModelFailureStage::ProviderRequest);
                        self.finish_observed_attempt(attempt_id.as_deref(),ModelAttemptOutcomeStatus::InDoubt,None,TokenUsage{input_tokens:None,output_tokens:None,total_tokens:None,source:UsageSource::Unknown},None,Some(failure.clone()))?;
                        return Err(CoreError::ModelFailure(failure));
                    }
                },
            };
            let status = response.status();
            let request_id = response
                .headers()
                .get("x-request-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            if is_retriable(status) && attempt < self.config.max_retries {
                let failure = annotagent_core::ModelFailure {
                    stage: annotagent_core::ModelFailureStage::ProviderRequest,
                    category: annotagent_core::ModelFailureCategory::HttpStatus,
                    http_status: Some(status.as_u16()),
                };
                self.finish_observed_attempt(
                    attempt_id.as_deref(),
                    ModelAttemptOutcomeStatus::Failed,
                    request_id.clone(),
                    TokenUsage {
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                        source: UsageSource::Unknown,
                    },
                    None,
                    Some(failure),
                )?;
                let delay = retry_delay(&self.config, response.headers(), attempt);
                tokio::select! {
                    () = cancellation.cancelled() => {
                        return Err(safe_failure(annotagent_core::ModelFailureStage::ProviderRequest, annotagent_core::ModelFailureCategory::Cancelled, None));
                    }
                    () = tokio::time::sleep(delay) => {}
                }
                continue;
            }
            if !status.is_success() {
                let failure = annotagent_core::ModelFailure {
                    stage: annotagent_core::ModelFailureStage::ProviderRequest,
                    category: annotagent_core::ModelFailureCategory::HttpStatus,
                    http_status: Some(status.as_u16()),
                };
                self.finish_observed_attempt(
                    attempt_id.as_deref(),
                    ModelAttemptOutcomeStatus::Failed,
                    request_id,
                    TokenUsage {
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                        source: UsageSource::Unknown,
                    },
                    None,
                    Some(failure.clone()),
                )?;
                return Err(CoreError::ModelFailure(failure));
            }
            let value = if self.config.streaming {
                match read_streaming_response(response, cancellation.clone()).await {
                    Ok(value) => value,
                    Err(error) => {
                        let failure = match &error {
                            CoreError::ModelFailure(failure) => failure.clone(),
                            _ => annotagent_core::ModelFailure {
                                stage: annotagent_core::ModelFailureStage::ResponseDecode,
                                category: annotagent_core::ModelFailureCategory::InvalidResponse,
                                http_status: None,
                            },
                        };
                        let outcome_status = if failure.stage
                            == annotagent_core::ModelFailureStage::ResponseDecode
                        {
                            ModelAttemptOutcomeStatus::Failed
                        } else {
                            ModelAttemptOutcomeStatus::InDoubt
                        };
                        self.finish_observed_attempt(
                            attempt_id.as_deref(),
                            outcome_status,
                            request_id.clone(),
                            unknown_usage(),
                            None,
                            Some(failure),
                        )?;
                        return Err(error);
                    }
                }
            } else {
                let bytes = tokio::select! {
                () = cancellation.cancelled() => {
                    let failure=annotagent_core::ModelFailure{stage:annotagent_core::ModelFailureStage::ResponseBody,category:annotagent_core::ModelFailureCategory::Cancelled,http_status:None};
                    self.finish_observed_attempt(attempt_id.as_deref(),ModelAttemptOutcomeStatus::InDoubt,request_id.clone(),unknown_usage(),None,Some(failure.clone()))?;
                    return Err(CoreError::ModelFailure(failure));
                },
                result = response.bytes() => match result {
                    Ok(bytes)=>bytes,
                    Err(error)=>{
                        let failure=model_failure_for_transport(&error,annotagent_core::ModelFailureStage::ResponseBody);
                        self.finish_observed_attempt(attempt_id.as_deref(),ModelAttemptOutcomeStatus::InDoubt,request_id.clone(),TokenUsage{input_tokens:None,output_tokens:None,total_tokens:None,source:UsageSource::Unknown},None,Some(failure.clone()))?;
                        return Err(CoreError::ModelFailure(failure));
                    }
                },
                };
                let Ok(value): Result<Value, _> = serde_json::from_slice(&bytes) else {
                    let failure = annotagent_core::ModelFailure {
                        stage: annotagent_core::ModelFailureStage::ResponseDecode,
                        category: annotagent_core::ModelFailureCategory::InvalidResponse,
                        http_status: None,
                    };
                    self.finish_observed_attempt(
                        attempt_id.as_deref(),
                        ModelAttemptOutcomeStatus::Failed,
                        request_id.clone(),
                        TokenUsage {
                            input_tokens: None,
                            output_tokens: None,
                            total_tokens: None,
                            source: UsageSource::Unknown,
                        },
                        None,
                        Some(failure.clone()),
                    )?;
                    return Err(CoreError::ModelFailure(failure));
                };
                value
            };
            let Ok(mut parsed) = parse_chat_response(&value, request_id.clone()) else {
                let failure = annotagent_core::ModelFailure {
                    stage: annotagent_core::ModelFailureStage::ResponseDecode,
                    category: annotagent_core::ModelFailureCategory::InvalidResponse,
                    http_status: None,
                };
                self.finish_observed_attempt(
                    attempt_id.as_deref(),
                    ModelAttemptOutcomeStatus::Failed,
                    request_id,
                    TokenUsage {
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                        source: UsageSource::Unknown,
                    },
                    None,
                    Some(failure.clone()),
                )?;
                return Err(CoreError::ModelFailure(failure));
            };
            parsed
                .provider_metadata
                .insert("retry_count".to_owned(), attempt.to_string());
            parsed
                .provider_metadata
                .insert("streaming".to_owned(), self.config.streaming.to_string());
            let response_mode = self.effective_response_mode();
            parsed.provider_metadata.insert(
                "response_mode".to_owned(),
                serde_json::to_value(response_mode)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_else(|| "automatic".to_owned()),
            );
            let before = parsed.tool_calls.len();
            let promotion = match response_mode {
                OpenAiResponseMode::NativeTool => {
                    try_promote_json_action(&mut parsed, &request.tools).map(|_| ())
                }
                OpenAiResponseMode::JsonObject | OpenAiResponseMode::JsonSchema => {
                    promote_json_action(&mut parsed, &request.tools)
                }
                OpenAiResponseMode::Automatic => Ok(()),
            };
            if parsed.tool_calls.len() > before {
                parsed
                    .provider_metadata
                    .insert("action_source".to_owned(), "json_adapter".to_owned());
                // The validated/promoted action is the durable structured value.
                // Keep only safe content presence/length diagnostics, not a second
                // raw copy of the Provider envelope.
                parsed.content = None;
            } else if !parsed.tool_calls.is_empty() {
                parsed
                    .provider_metadata
                    .insert("action_source".to_owned(), "native_tool".to_owned());
            }
            if let Err(error) = promotion {
                let failure = annotagent_core::ModelFailure {
                    stage: annotagent_core::ModelFailureStage::StructuredOutput,
                    category: annotagent_core::ModelFailureCategory::InvalidStructuredOutput,
                    http_status: None,
                };
                parsed.provider_metadata.insert(
                    "structured_output_error".to_owned(),
                    safe_structured_output_error(&error),
                );
                // Invalid/partial final text is not an executable artifact and may
                // echo private input. Its safe length metadata and usage remain.
                parsed.content = None;
                let cached = cached_input_tokens(&parsed);
                self.finish_observed_attempt(
                    attempt_id.as_deref(),
                    ModelAttemptOutcomeStatus::Failed,
                    parsed.request_id.clone(),
                    parsed.usage.clone(),
                    cached,
                    Some(failure.clone()),
                )?;
                // The HTTP request completed and usage is known. Preserve the response
                // for the phase-specific strict validator instead of converting it to
                // an indeterminate transport result or retrying.
                return Ok(parsed);
            }
            let cached = cached_input_tokens(&parsed);
            self.finish_observed_attempt(
                attempt_id.as_deref(),
                ModelAttemptOutcomeStatus::Succeeded,
                parsed.request_id.clone(),
                parsed.usage.clone(),
                cached,
                None,
            )?;
            return Ok(parsed);
        }
        Err(CoreError::Provider("provider retries exhausted".to_owned()))
    }
}

const MAX_STREAM_EVENT_BYTES: usize = 1024 * 1024;
const MAX_STREAM_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_STREAM_TOOL_CALLS: usize = 16;

fn unknown_usage() -> TokenUsage {
    TokenUsage {
        input_tokens: None,
        output_tokens: None,
        total_tokens: None,
        source: UsageSource::Unknown,
    }
}

#[derive(Default)]
struct StreamToolCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Default)]
struct StreamAccumulator {
    id: Option<String>,
    content: String,
    tools: BTreeMap<usize, StreamToolCall>,
    finish_reason: Option<String>,
    usage: Option<Value>,
    received_bytes: usize,
    saw_data: bool,
    saw_done: bool,
}

impl StreamAccumulator {
    fn push(&mut self, data: &[u8]) -> CoreResult<()> {
        if data == b"[DONE]" {
            self.saw_done = true;
            return Ok(());
        }
        self.received_bytes = self.received_bytes.saturating_add(data.len());
        if data.len() > MAX_STREAM_EVENT_BYTES || self.received_bytes > MAX_STREAM_RESPONSE_BYTES {
            return Err(safe_failure(
                annotagent_core::ModelFailureStage::ResponseDecode,
                annotagent_core::ModelFailureCategory::InvalidResponse,
                None,
            ));
        }
        let value: Value = serde_json::from_slice(data).map_err(|_| {
            safe_failure(
                annotagent_core::ModelFailureStage::ResponseDecode,
                annotagent_core::ModelFailureCategory::InvalidResponse,
                None,
            )
        })?;
        self.saw_data = true;
        if self.id.is_none() {
            self.id = value.get("id").and_then(Value::as_str).map(str::to_owned);
        }
        if let Some(usage) = value.get("usage").filter(|usage| usage.is_object()) {
            self.usage = Some(usage.clone());
        }
        for choice in value
            .get("choices")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if choice.get("index").and_then(Value::as_u64).unwrap_or(0) != 0 {
                continue;
            }
            if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                self.finish_reason = Some(reason.to_owned());
            }
            let Some(delta) = choice.get("delta") else {
                continue;
            };
            if let Some(content) = delta.get("content").and_then(Value::as_str) {
                self.content.push_str(content);
            }
            for fragment in delta
                .get("tool_calls")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let index = fragment
                    .get("index")
                    .and_then(Value::as_u64)
                    .and_then(|index| usize::try_from(index).ok())
                    .ok_or_else(|| {
                        safe_failure(
                            annotagent_core::ModelFailureStage::ResponseDecode,
                            annotagent_core::ModelFailureCategory::InvalidResponse,
                            None,
                        )
                    })?;
                if index >= MAX_STREAM_TOOL_CALLS {
                    return Err(safe_failure(
                        annotagent_core::ModelFailureStage::ResponseDecode,
                        annotagent_core::ModelFailureCategory::InvalidResponse,
                        None,
                    ));
                }
                let tool = self.tools.entry(index).or_default();
                if let Some(id) = fragment.get("id").and_then(Value::as_str) {
                    if !tool.id.is_empty() && tool.id != id {
                        return Err(safe_failure(
                            annotagent_core::ModelFailureStage::ResponseDecode,
                            annotagent_core::ModelFailureCategory::InvalidResponse,
                            None,
                        ));
                    }
                    id.clone_into(&mut tool.id);
                }
                if let Some(function) = fragment.get("function") {
                    if let Some(name) = function.get("name").and_then(Value::as_str) {
                        tool.name.push_str(name);
                    }
                    if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                        tool.arguments.push_str(arguments);
                    }
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> CoreResult<Value> {
        if !self.saw_data || !self.saw_done || self.finish_reason.is_none() {
            return Err(safe_failure(
                annotagent_core::ModelFailureStage::ResponseBody,
                annotagent_core::ModelFailureCategory::InvalidResponse,
                None,
            ));
        }
        let tool_calls = self
            .tools
            .into_iter()
            .map(|(index, tool)| {
                json!({
                    "index":index,"id":tool.id,"type":"function",
                    "function":{"name":tool.name,"arguments":tool.arguments}
                })
            })
            .collect::<Vec<_>>();
        let mut message = json!({
            "role":"assistant",
            "content":if self.content.is_empty(){Value::Null}else{Value::String(self.content)},
        });
        if !tool_calls.is_empty() {
            message["tool_calls"] = Value::Array(tool_calls);
        }
        let mut value = json!({
            "id":self.id,
            "choices":[{"index":0,"message":message,"finish_reason":self.finish_reason}],
        });
        if let Some(usage) = self.usage {
            value["usage"] = usage;
        }
        Ok(value)
    }
}

async fn read_streaming_response(
    response: reqwest::Response,
    cancellation: CancellationToken,
) -> CoreResult<Value> {
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut event_data = Vec::<Vec<u8>>::new();
    let mut accumulator = StreamAccumulator::default();
    loop {
        let next = tokio::select! {
            () = cancellation.cancelled() => {
                return Err(safe_failure(
                    annotagent_core::ModelFailureStage::ResponseBody,
                    annotagent_core::ModelFailureCategory::Cancelled,
                    None,
                ));
            }
            next = stream.next() => next,
        };
        let Some(chunk) = next else { break };
        let chunk = chunk.map_err(|error| {
            CoreError::ModelFailure(model_failure_for_transport(
                &error,
                annotagent_core::ModelFailureStage::ResponseBody,
            ))
        })?;
        buffer.extend_from_slice(&chunk);
        if buffer.len() > MAX_STREAM_EVENT_BYTES {
            return Err(safe_failure(
                annotagent_core::ModelFailureStage::ResponseDecode,
                annotagent_core::ModelFailureCategory::InvalidResponse,
                None,
            ));
        }
        while let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = buffer.drain(..=newline).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                if !event_data.is_empty() {
                    let data = event_data
                        .drain(..)
                        .reduce(|mut joined, line| {
                            joined.push(b'\n');
                            joined.extend(line);
                            joined
                        })
                        .unwrap_or_default();
                    accumulator.push(&data)?;
                }
                continue;
            }
            if line.starts_with(b":") {
                continue;
            }
            if let Some(data) = line.strip_prefix(b"data:") {
                event_data.push(data.strip_prefix(b" ").unwrap_or(data).to_vec());
            }
        }
    }
    if !buffer.is_empty() || !event_data.is_empty() {
        return Err(safe_failure(
            annotagent_core::ModelFailureStage::ResponseBody,
            annotagent_core::ModelFailureCategory::InvalidResponse,
            None,
        ));
    }
    accumulator.finish()
}

fn safe_structured_output_error(error: &CoreError) -> String {
    let text = error.to_string();
    if text.contains("invalid JSON-only action") {
        "invalid_json".to_owned()
    } else if text.contains("unregistered tool") {
        "wrong_action_name".to_owned()
    } else if text.contains("lacks object field `arguments`") {
        "arguments_not_object".to_owned()
    } else if text.contains("lacks string field `name`") {
        "action_name_missing".to_owned()
    } else if text.contains("no constrained action") {
        "no_action".to_owned()
    } else {
        "invalid_action_envelope".to_owned()
    }
}

impl OpenAiCompatibleProvider {
    fn finish_observed_attempt(
        &self,
        attempt_id: Option<&str>,
        status: ModelAttemptOutcomeStatus,
        request_id: Option<String>,
        usage: TokenUsage,
        cached_input_tokens: Option<u64>,
        failure: Option<annotagent_core::ModelFailure>,
    ) -> CoreResult<()> {
        let observer = self
            .attempt_observer
            .read()
            .map_err(|_| CoreError::Provider("model attempt observer unavailable".into()))?
            .clone();
        if let (Some(observer), Some(attempt_id)) = (observer, attempt_id) {
            observer.finish(
                attempt_id,
                &ModelAttemptOutcome {
                    status,
                    request_id,
                    usage,
                    cached_input_tokens,
                    completed_at: chrono::Utc::now(),
                    failure,
                },
            )?;
        }
        Ok(())
    }
}

fn cached_input_tokens(response: &ModelResponse) -> Option<u64> {
    response
        .provider_metadata
        .get("cached_input_tokens")
        .and_then(|value| value.parse().ok())
}

fn is_retriable(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn retry_delay(config: &OpenAiCompatibleConfig, headers: &HeaderMap, attempt: u32) -> Duration {
    let configured = config
        .minimum_retry_delay_ms
        .saturating_mul(1_u64.checked_shl(attempt.min(31)).unwrap_or(u64::MAX));
    let retry_after = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1_000));
    Duration::from_millis(
        retry_after
            .unwrap_or(configured)
            .clamp(config.minimum_retry_delay_ms, config.maximum_retry_delay_ms),
    )
}

fn safe_failure(
    stage: annotagent_core::ModelFailureStage,
    category: annotagent_core::ModelFailureCategory,
    http_status: Option<u16>,
) -> CoreError {
    CoreError::ModelFailure(annotagent_core::ModelFailure {
        stage,
        category,
        http_status,
    })
}
fn model_failure_for_transport(
    error: &reqwest::Error,
    stage: annotagent_core::ModelFailureStage,
) -> annotagent_core::ModelFailure {
    use annotagent_core::ModelFailureCategory as C;
    annotagent_core::ModelFailure {
        stage,
        category: if error.is_timeout() {
            C::Timeout
        } else if error.is_connect() {
            C::Connection
        } else {
            C::Transport
        },
        http_status: None,
    }
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
    let mut invalid_tool_arguments = 0usize;
    let mut missing_tool_names = 0usize;
    let mut tool_calls = Vec::new();
    if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
        for call in calls {
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("provider-call");
            let function = call.get("function").unwrap_or(call);
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_else(|| {
                    missing_tool_names += 1;
                    ""
                });
            let arguments = match function.get("arguments") {
                Some(Value::String(raw)) => serde_json::from_str(raw).unwrap_or_else(|_| {
                    invalid_tool_arguments += 1;
                    // Preserve the completed response and usage without persisting
                    // a partial argument payload that may contain private input.
                    Value::Null
                }),
                Some(value) if value.is_object() => value.clone(),
                _ => {
                    invalid_tool_arguments += 1;
                    Value::Null
                }
            };
            tool_calls.push(ModelToolCall {
                id: ToolCallId::new(id),
                name: name.to_owned(),
                arguments,
            });
        }
    }
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
    let mut provider_metadata = BTreeMap::new();
    if let Some(finish_reason) = value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
    {
        provider_metadata.insert("finish_reason".to_owned(), finish_reason.to_owned());
    }
    provider_metadata.insert("content_present".to_owned(), content.is_some().to_string());
    provider_metadata.insert(
        "content_length".to_owned(),
        content.as_ref().map_or(0, String::len).to_string(),
    );
    provider_metadata.insert("tool_call_count".to_owned(), tool_calls.len().to_string());
    provider_metadata.insert(
        "invalid_tool_arguments".to_owned(),
        invalid_tool_arguments.to_string(),
    );
    provider_metadata.insert(
        "missing_tool_names".to_owned(),
        missing_tool_names.to_string(),
    );
    let reasoning_content = message
        .get("reasoning_content")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    provider_metadata.insert(
        "reasoning_content_present".to_owned(),
        reasoning_content.is_some().to_string(),
    );
    provider_metadata.insert(
        "reasoning_content_length".to_owned(),
        reasoning_content.map_or(0, str::len).to_string(),
    );
    if let Some(reasoning_tokens) = value
        .pointer("/usage/completion_tokens_details/reasoning_tokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            value
                .pointer("/usage/reasoning_tokens")
                .and_then(Value::as_u64)
        })
    {
        provider_metadata.insert("reasoning_tokens".to_owned(), reasoning_tokens.to_string());
    }
    if let Some(cached) = value
        .pointer("/usage/prompt_tokens_details/cached_tokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            value
                .pointer("/usage/cached_tokens")
                .and_then(Value::as_u64)
        })
    {
        provider_metadata.insert("cached_input_tokens".to_owned(), cached.to_string());
    }
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
        provider_metadata,
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
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{
        Json, Router,
        body::Body,
        extract::State,
        http::header,
        response::{Html, IntoResponse, Response},
        routing::post,
    };

    use super::*;

    #[derive(Clone)]
    struct RetryFixture {
        calls: Arc<AtomicUsize>,
        failures: usize,
    }

    #[derive(Clone)]
    struct CaptureFixture {
        bodies: Arc<Mutex<Vec<Value>>>,
        response: Value,
    }

    async fn capture_completion(
        State(state): State<CaptureFixture>,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        state.bodies.lock().expect("bodies").push(body);
        Json(state.response)
    }

    async fn spawn_capture_fixture(response: Value) -> (String, Arc<Mutex<Vec<Value>>>) {
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let state = CaptureFixture {
            bodies: Arc::clone(&bodies),
            response,
        };
        tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/v1/chat/completions", post(capture_completion))
                    .with_state(state),
            )
            .await
            .expect("fixture server");
        });
        (format!("http://{address}/v1"), bodies)
    }

    async fn retrying_completion(State(state): State<RetryFixture>) -> Response {
        let call = state.calls.fetch_add(1, Ordering::SeqCst);
        if call < state.failures {
            return (
                StatusCode::BAD_GATEWAY,
                Html("<html><body><h1>502 Bad Gateway</h1><p>nginx</p></body></html>"),
            )
                .into_response();
        }
        Json(json!({
            "choices": [{"message": {"role": "assistant", "content": "OK"}}],
            "usage": {"prompt_tokens": 2, "completion_tokens": 1, "total_tokens": 3}
        }))
        .into_response()
    }

    async fn spawn_retry_fixture(failures: usize) -> (String, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let state = RetryFixture {
            calls: Arc::clone(&calls),
            failures,
        };
        tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/v1/chat/completions", post(retrying_completion))
                    .with_state(state),
            )
            .await
            .expect("fixture server");
        });
        (format!("http://{address}/v1"), calls)
    }

    #[derive(Clone)]
    struct StreamingFixture {
        bodies: Arc<Mutex<Vec<Value>>>,
        chunks: Arc<Vec<Vec<u8>>>,
    }

    async fn streaming_completion(
        State(state): State<StreamingFixture>,
        Json(body): Json<Value>,
    ) -> Response {
        state.bodies.lock().expect("bodies").push(body);
        let chunks = Arc::clone(&state.chunks);
        let stream = futures::stream::unfold(0usize, move |index| {
            let chunks = Arc::clone(&chunks);
            async move {
                let chunk = chunks.get(index)?.clone();
                tokio::time::sleep(Duration::from_millis(40)).await;
                Some((
                    Ok::<_, std::io::Error>(axum::body::Bytes::from(chunk)),
                    index + 1,
                ))
            }
        });
        Response::builder()
            .header(header::CONTENT_TYPE, "text/event-stream")
            .body(Body::from_stream(stream))
            .expect("stream response")
    }

    async fn spawn_streaming_fixture(chunks: Vec<Vec<u8>>) -> (String, Arc<Mutex<Vec<Value>>>) {
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let state = StreamingFixture {
            bodies: Arc::clone(&bodies),
            chunks: Arc::new(chunks),
        };
        tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/v1/chat/completions", post(streaming_completion))
                    .with_state(state),
            )
            .await
            .expect("stream fixture server");
        });
        (format!("http://{address}/v1"), bodies)
    }

    fn retry_test_config(endpoint: String, max_retries: u32) -> OpenAiCompatibleConfig {
        OpenAiCompatibleConfig {
            endpoint,
            api_key_env: "UNUSED_TEST_KEY".to_owned(),
            model: "fixture-model".to_owned(),
            protocol: OpenAiProtocol::ChatCompletions,
            request_timeout_seconds: 5,
            max_output_tokens: 100,
            temperature: 0.0,
            reasoning_mode: None,
            supports_tool_calls: true,
            supports_json_schema: false,
            streaming: false,
            response_mode: OpenAiResponseMode::Automatic,
            custom_headers: BTreeMap::new(),
            extra_request_fields: BTreeMap::new(),
            max_retries,
            minimum_retry_delay_ms: 0,
            maximum_retry_delay_ms: 0,
        }
    }

    fn retry_test_request() -> ModelRequest {
        ModelRequest {
            model: "ignored".to_owned(),
            task_id: annotagent_core::TaskId::from("pipeline_builder"),
            messages: vec![ModelMessage {
                role: ModelRole::User,
                content: "Reply with OK.".to_owned(),
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            images: Vec::new(),
            tools: Vec::new(),
            max_output_tokens: 100,
            temperature: 0.0,
            extra: BTreeMap::new(),
        }
    }

    #[derive(Default)]
    struct RecordingAttemptObserver {
        begins: Mutex<Vec<(Option<String>, u32)>>,
        finishes: Mutex<Vec<ModelAttemptOutcome>>,
    }
    impl ModelAttemptObserver for RecordingAttemptObserver {
        fn begin(
            &self,
            call_id: Option<&str>,
            _request: &ModelRequest,
            attempt_number: u32,
            _started_at: chrono::DateTime<chrono::Utc>,
        ) -> CoreResult<String> {
            self.begins
                .lock()
                .expect("begins")
                .push((call_id.map(str::to_owned), attempt_number));
            Ok(format!("attempt-{attempt_number}"))
        }
        fn finish(&self, _attempt_id: &str, outcome: &ModelAttemptOutcome) -> CoreResult<()> {
            self.finishes
                .lock()
                .expect("finishes")
                .push(outcome.clone());
            Ok(())
        }
    }

    #[tokio::test]
    async fn retries_transient_gateway_failures_and_records_attempt_count() {
        let (endpoint, calls) = spawn_retry_fixture(2).await;
        let provider = OpenAiCompatibleProvider::new_with_api_key(
            retry_test_config(endpoint, 2),
            Some("fixture-secret".to_owned()),
        )
        .expect("provider");

        let response = provider
            .complete(retry_test_request(), CancellationToken::new())
            .await
            .expect("third attempt succeeds");

        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert_eq!(response.provider_metadata["retry_count"], "2");
    }

    #[tokio::test]
    async fn reports_each_physical_retry_under_the_admitted_call_identity() {
        let (endpoint, calls) = spawn_retry_fixture(2).await;
        let observer = Arc::new(RecordingAttemptObserver::default());
        let provider = OpenAiCompatibleProvider::new_with_api_key(
            retry_test_config(endpoint, 2),
            Some("fixture-secret".to_owned()),
        )
        .expect("provider")
        .with_attempt_observer(observer.clone());

        let response = within_model_call(
            "00000000-0000-0000-0000-000000000123".to_owned(),
            provider.complete(retry_test_request(), CancellationToken::new()),
        )
        .await
        .expect("third attempt succeeds");

        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert_eq!(response.usage.input_tokens, Some(2));
        assert_eq!(
            *observer.begins.lock().expect("begins"),
            vec![
                (Some("00000000-0000-0000-0000-000000000123".into()), 1),
                (Some("00000000-0000-0000-0000-000000000123".into()), 2),
                (Some("00000000-0000-0000-0000-000000000123".into()), 3),
            ]
        );
        let statuses = observer
            .finishes
            .lock()
            .expect("finishes")
            .iter()
            .map(|outcome| outcome.status)
            .collect::<Vec<_>>();
        assert_eq!(
            statuses,
            vec![
                ModelAttemptOutcomeStatus::Failed,
                ModelAttemptOutcomeStatus::Failed,
                ModelAttemptOutcomeStatus::Succeeded,
            ]
        );
    }

    #[tokio::test]
    async fn exhausted_gateway_failure_is_actionable_and_hides_html() {
        let (endpoint, calls) = spawn_retry_fixture(usize::MAX).await;
        let provider = OpenAiCompatibleProvider::new_with_api_key(
            retry_test_config(endpoint, 1),
            Some("fixture-secret".to_owned()),
        )
        .expect("provider");

        let error = provider
            .complete(retry_test_request(), CancellationToken::new())
            .await
            .expect_err("both attempts fail")
            .to_string();

        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(error.contains("HttpStatus"));
        assert!(error.contains("502"));
        assert!(!error.contains("<html>"));
        assert!(!error.contains("nginx"));
    }

    #[tokio::test]
    async fn safe_failure_preserves_http_and_decode_categories_without_remote_text() {
        use annotagent_core::{ModelFailureCategory as C, ModelFailureStage as S};
        for (status, body, category, stage) in [
            (
                StatusCode::UNAUTHORIZED,
                "secret fixture-key raw server cause",
                C::HttpStatus,
                S::ProviderRequest,
            ),
            (
                StatusCode::OK,
                "secret fixture-key broken JSON",
                C::InvalidResponse,
                S::ResponseDecode,
            ),
            (StatusCode::OK, "{}", C::InvalidResponse, S::ResponseDecode),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                axum::serve(
                    listener,
                    Router::new().route(
                        "/v1/chat/completions",
                        post(move || async move { (status, body) }),
                    ),
                )
                .await
                .unwrap();
            });
            let provider = OpenAiCompatibleProvider::new_with_api_key(
                retry_test_config(format!("http://{addr}/v1"), 0),
                Some("fixture-key".into()),
            )
            .unwrap();
            let error = provider
                .complete(retry_test_request(), CancellationToken::new())
                .await
                .unwrap_err();
            assert!(!error.to_string().contains("fixture-key"));
            assert!(!error.to_string().contains("raw server"));
            let CoreError::ModelFailure(failure) = error else {
                panic!("expected typed failure")
            };
            assert_eq!(failure.category, category);
            assert_eq!(failure.stage, stage);
            assert_eq!(
                failure.http_status,
                (!status.is_success()).then_some(status.as_u16())
            );
            server.abort();
        }
    }

    #[tokio::test]
    async fn stalled_response_body_has_safe_timeout_and_cancellation_causes() {
        use futures::StreamExt;
        for cancel in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                axum::serve(
                    listener,
                    Router::new().route(
                        "/v1/chat/completions",
                        post(|| async {
                            axum::body::Body::from_stream(
                                futures::stream::once(async {
                                    Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"{"))
                                })
                                .chain(futures::stream::pending()),
                            )
                        }),
                    ),
                )
                .await
                .unwrap();
            });
            let mut config = retry_test_config(format!("http://{addr}/v1"), 0);
            config.request_timeout_seconds = 1;
            let provider =
                OpenAiCompatibleProvider::new_with_api_key(config, Some("TEST-only".into()))
                    .unwrap();
            let cancellation = CancellationToken::new();
            let token = cancellation.clone();
            let trigger = async move {
                if cancel {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    token.cancel();
                }
            };
            let (result, ()) = tokio::join!(
                provider.complete(retry_test_request(), cancellation),
                trigger
            );
            let CoreError::ModelFailure(failure) = result.unwrap_err() else {
                panic!("typed failure required")
            };
            assert_eq!(
                failure.stage,
                annotagent_core::ModelFailureStage::ResponseBody
            );
            assert_eq!(
                failure.category,
                if cancel {
                    annotagent_core::ModelFailureCategory::Cancelled
                } else {
                    annotagent_core::ModelFailureCategory::Timeout
                }
            );
            server.abort();
        }
    }

    #[tokio::test]
    async fn untyped_stream_extra_cannot_enable_streaming() {
        let (endpoint, bodies) = spawn_capture_fixture(json!({
            "choices":[{"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}]
        }))
        .await;
        let mut config = retry_test_config(endpoint, 0);
        config
            .extra_request_fields
            .insert("stream".into(), json!(true));
        let provider =
            OpenAiCompatibleProvider::new_with_api_key(config, Some("TEST-only".into())).unwrap();
        provider
            .complete(retry_test_request(), CancellationToken::new())
            .await
            .unwrap();
        assert!(bodies.lock().unwrap()[0].get("stream").is_none());
    }

    #[tokio::test]
    async fn timed_sse_stream_reassembles_split_utf8_content_and_usage() {
        let wire = concat!(
            "data: {\"id\":\"stream-1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"你\"},\"finish_reason\":null}]}\r\n\r\n",
            ": heartbeat\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"好\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":2,\"total_tokens\":9}}\n\n",
            "data: [DONE]\n\n"
        )
        .as_bytes()
        .to_vec();
        let split = wire
            .windows("你".len())
            .position(|window| window == "你".as_bytes())
            .expect("Chinese bytes")
            + 1;
        let chunks = vec![wire[..split].to_vec(), wire[split..].to_vec()];
        let (endpoint, bodies) = spawn_streaming_fixture(chunks).await;
        let mut config = retry_test_config(endpoint, 0);
        config.streaming = true;
        config.supports_tool_calls = false;
        let provider =
            OpenAiCompatibleProvider::new_with_api_key(config, Some("fixture-secret".to_owned()))
                .unwrap();

        let started = std::time::Instant::now();
        let response = provider
            .complete(retry_test_request(), CancellationToken::new())
            .await
            .expect("valid SSE response");
        assert!(started.elapsed() >= Duration::from_millis(80));
        assert_eq!(response.content.as_deref(), Some("你好"));
        assert_eq!(response.usage.input_tokens, Some(7));
        assert_eq!(response.usage.output_tokens, Some(2));
        assert_eq!(response.provider_metadata["finish_reason"], "stop");
        assert_eq!(response.provider_metadata["streaming"], "true");
        assert_eq!(bodies.lock().unwrap()[0]["stream"], true);
    }

    #[test]
    fn stream_tool_fragments_are_joined_by_index_only_after_done() {
        let mut stream = StreamAccumulator::default();
        stream
            .push(br#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"id":"b","function":{"name":"second","arguments":"{\"b\":"}},{"index":0,"id":"a","function":{"name":"first","arguments":"{\"a\":"}}]},"finish_reason":null}]}"#)
            .unwrap();
        assert!(stream.finish_reason.is_none());
        stream
            .push(br#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"1}"}},{"index":1,"function":{"arguments":"2}"}}]},"finish_reason":"tool_calls"}]}"#)
            .unwrap();
        stream.push(b"[DONE]").unwrap();
        let value = stream.finish().unwrap();
        let response = parse_chat_response(&value, Some("request".into())).unwrap();
        assert_eq!(response.tool_calls.len(), 2);
        assert_eq!(response.tool_calls[0].name, "first");
        assert_eq!(response.tool_calls[0].arguments, json!({"a":1}));
        assert_eq!(response.tool_calls[1].name, "second");
        assert_eq!(response.tool_calls[1].arguments, json!({"b":2}));
    }

    #[tokio::test]
    async fn truncated_stream_is_in_doubt_and_never_retried() {
        let event = b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n".to_vec();
        let (endpoint, bodies) = spawn_streaming_fixture(vec![event]).await;
        let mut config = retry_test_config(endpoint, 3);
        config.streaming = true;
        config.supports_tool_calls = false;
        let observer = Arc::new(RecordingAttemptObserver::default());
        let provider =
            OpenAiCompatibleProvider::new_with_api_key(config, Some("fixture-secret".to_owned()))
                .unwrap()
                .with_attempt_observer(observer.clone());
        let error = provider
            .complete(retry_test_request(), CancellationToken::new())
            .await
            .expect_err("EOF without finish and DONE is not success");
        assert!(matches!(error, CoreError::ModelFailure(_)));
        assert_eq!(bodies.lock().unwrap().len(), 1);
        assert_eq!(observer.finishes.lock().unwrap().len(), 1);
        assert_eq!(
            observer.finishes.lock().unwrap()[0].status,
            ModelAttemptOutcomeStatus::InDoubt
        );
    }

    #[test]
    fn retry_after_and_connection_policy_bound_retry_delay() {
        let mut config = retry_test_config("https://provider.example/v1".to_owned(), 2);
        config.minimum_retry_delay_ms = 250;
        config.maximum_retry_delay_ms = 5_000;
        let mut headers = HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "3".parse().expect("header"));
        assert_eq!(retry_delay(&config, &headers, 0), Duration::from_secs(3));
        assert_eq!(
            retry_delay(&config, &HeaderMap::new(), 8),
            Duration::from_secs(5)
        );
    }

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
    fn preserves_safe_completion_diagnostics_without_reasoning_text() {
        let response = parse_chat_response(
            &json!({
                "choices": [{
                    "finish_reason":"length",
                    "message": {"content":null,"reasoning_content":"private chain"}
                }],
                "usage": {
                    "prompt_tokens":1639,"completion_tokens":2048,"total_tokens":3687,
                    "completion_tokens_details":{"reasoning_tokens":1900}
                }
            }),
            Some("TEST-length".to_owned()),
        )
        .expect("response");
        assert_eq!(response.provider_metadata["finish_reason"], "length");
        assert_eq!(response.provider_metadata["content_present"], "false");
        assert_eq!(response.provider_metadata["content_length"], "0");
        assert_eq!(response.provider_metadata["tool_call_count"], "0");
        assert_eq!(
            response.provider_metadata["reasoning_content_present"],
            "true"
        );
        assert_eq!(response.provider_metadata["reasoning_content_length"], "13");
        assert_eq!(response.provider_metadata["reasoning_tokens"], "1900");
        assert!(!format!("{:?}", response.provider_metadata).contains("private chain"));
        assert_eq!(response.usage.output_tokens, Some(2048));
    }

    #[test]
    fn malformed_native_arguments_keep_usage_without_persisting_partial_payload() {
        let response = parse_chat_response(
            &json!({
                "choices":[{"finish_reason":"length","message":{"content":null,"tool_calls":[{
                    "id":"TEST-partial","function":{
                        "name":"propose_annotation_schema",
                        "arguments":"{\"decision\":\"draft\",\"private\":\"do not retain"
                    }
                }]}}],
                "usage":{"prompt_tokens":1639,"completion_tokens":2048,"total_tokens":3687}
            }),
            Some("TEST-partial".into()),
        )
        .unwrap();
        assert_eq!(response.usage.total_tokens, Some(3_687));
        assert_eq!(response.tool_calls.len(), 1);
        assert!(response.tool_calls[0].arguments.is_null());
        assert_eq!(response.provider_metadata["invalid_tool_arguments"], "1");
        assert!(
            !serde_json::to_string(&response)
                .unwrap()
                .contains("do not retain")
        );
    }

    #[tokio::test]
    async fn json_object_http_request_freezes_mode_limit_and_thinking_after_extra_merge() {
        let (endpoint, bodies) = spawn_capture_fixture(json!({
            "id":"TEST-json-object",
            "choices":[{"finish_reason":"stop","message":{"content":
                "{\"name\":\"submit\",\"arguments\":{}}"}}],
            "usage":{"prompt_tokens":12,"completion_tokens":8,"total_tokens":20}
        }))
        .await;
        let provider = OpenAiCompatibleProvider::new_with_api_key(
            OpenAiCompatibleConfig {
                endpoint,
                api_key_env: "UNUSED_TEST_KEY".to_owned(),
                model: "TEST-structured".to_owned(),
                protocol: OpenAiProtocol::ChatCompletions,
                request_timeout_seconds: 5,
                max_output_tokens: 4_096,
                temperature: 0.0,
                reasoning_mode: Some("high".to_owned()),
                supports_tool_calls: true,
                supports_json_schema: true,
                streaming: false,
                response_mode: OpenAiResponseMode::JsonObject,
                custom_headers: BTreeMap::new(),
                extra_request_fields: BTreeMap::from([
                    ("max_tokens".to_owned(), json!(99_999)),
                    ("response_format".to_owned(), json!({"type":"json_schema"})),
                    ("tool_choice".to_owned(), json!("required")),
                    ("thinking".to_owned(), json!({"type":"disabled"})),
                ]),
                max_retries: 0,
                minimum_retry_delay_ms: 0,
                maximum_retry_delay_ms: 0,
            },
            Some("TEST-only".to_owned()),
        )
        .expect("provider");
        let request = ModelRequest {
            model: "ignored".to_owned(),
            task_id: annotagent_core::TaskId::from("schema"),
            messages: vec![ModelMessage {
                role: ModelRole::User,
                content: "return JSON".to_owned(),
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            images: Vec::new(),
            tools: vec![annotagent_core::ToolDefinition {
                name: "submit".to_owned(),
                description: "submit".to_owned(),
                parameters: json!({"type":"object","additionalProperties":false}),
                read_only: false,
            }],
            max_output_tokens: 4_096,
            temperature: 0.0,
            extra: BTreeMap::from([
                ("max_tokens".to_owned(), json!(1)),
                ("thinking".to_owned(), json!({"type":"enabled"})),
                ("tools".to_owned(), json!([{"unsafe":true}])),
            ]),
        };
        let response = provider
            .complete(request, CancellationToken::new())
            .await
            .expect("completed response");
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.provider_metadata["action_source"], "json_adapter");
        let body = &bodies.lock().expect("bodies")[0];
        assert_eq!(body["model"], "TEST-structured");
        assert_eq!(body["max_tokens"], 4_096);
        assert_eq!(body["thinking"], json!({"type":"disabled"}));
        assert_eq!(body["response_format"], json!({"type":"json_object"}));
        assert!(body.get("tools").is_none());
        assert!(body.get("tool_choice").is_none());
        assert!(body.get("parallel_tool_calls").is_none());
        assert!(body.get("reasoning_effort").is_none());
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
                streaming: false,
                response_mode: OpenAiResponseMode::Automatic,
                custom_headers: BTreeMap::new(),
                extra_request_fields: BTreeMap::new(),
                max_retries: 0,
                minimum_retry_delay_ms: 0,
                maximum_retry_delay_ms: 0,
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
                streaming: false,
                response_mode: OpenAiResponseMode::Automatic,
                custom_headers: BTreeMap::new(),
                extra_request_fields: BTreeMap::new(),
                max_retries: 0,
                minimum_retry_delay_ms: 0,
                maximum_retry_delay_ms: 0,
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
            streaming: false,
            response_mode: OpenAiResponseMode::Automatic,
            custom_headers: BTreeMap::new(),
            extra_request_fields: BTreeMap::new(),
            max_retries: 0,
            minimum_retry_delay_ms: 0,
            maximum_retry_delay_ms: 0,
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

        let mut unbounded_retry = retry_test_config("https://provider.example/v1".to_owned(), 1);
        unbounded_retry.maximum_retry_delay_ms = 120_001;
        assert!(OpenAiCompatibleProvider::new(unbounded_retry).is_err());
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
                streaming: false,
                response_mode: OpenAiResponseMode::Automatic,
                custom_headers: BTreeMap::new(),
                extra_request_fields: BTreeMap::new(),
                max_retries: 0,
                minimum_retry_delay_ms: 0,
                maximum_retry_delay_ms: 0,
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
                streaming: false,
                response_mode: OpenAiResponseMode::Automatic,
                custom_headers: BTreeMap::new(),
                extra_request_fields: BTreeMap::new(),
                max_retries: 0,
                minimum_retry_delay_ms: 0,
                maximum_retry_delay_ms: 0,
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
        assert_eq!(body["tool_choice"], "auto");
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
