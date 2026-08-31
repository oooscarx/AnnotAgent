//! Bounded lifecycle requests for persistent Provider Profiles.

use std::time::{Duration, Instant};

use annotagent_core::{
    ProviderAdapterKind, ProviderErrorCode, ProviderErrorDetails, ProviderProfile, SecretValue,
};
use futures::StreamExt;
use reqwest::{Client, Response, StatusCode, header::HeaderValue, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const MAX_REGISTRY_RESPONSE_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredProviderModel {
    pub remote_model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPassiveCheck {
    pub reachable: bool,
    pub protocol_compatible: bool,
    pub discovered_model_count: usize,
    pub latency_ms: u64,
    pub safe_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderActiveProbe {
    pub request_id: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub latency_ms: u64,
    pub safe_message: String,
}

pub async fn discover_provider_models(
    profile: &ProviderProfile,
    credential: Option<&SecretValue>,
) -> Result<(Vec<DiscoveredProviderModel>, u64), ProviderErrorDetails> {
    profile.validate().map_err(|_| {
        provider_error(
            profile,
            ProviderErrorCode::InvalidEndpoint,
            "discover_models",
            false,
            "Provider connection settings are invalid.",
        )
    })?;
    if profile.adapter == ProviderAdapterKind::Mock {
        return Ok((Vec::new(), 0));
    }
    let credential = credential.ok_or_else(|| {
        provider_error(
            profile,
            ProviderErrorCode::MissingCredential,
            "discover_models",
            true,
            "Configure a credential before checking this Provider.",
        )
    })?;
    let client = registry_client(profile)?;
    let started = Instant::now();
    let response = authorized_request(
        client.get(endpoint_url(profile, "models")?),
        profile,
        credential,
    )
    .send()
    .await
    .map_err(|error| transport_error(profile, "discover_models", &error))?;
    let response = successful_response(profile, "discover_models", response)?;
    let body = read_limited(profile, "discover_models", response).await?;
    let value: Value = serde_json::from_slice(&body).map_err(|_| {
        provider_error(
            profile,
            ProviderErrorCode::InvalidResponse,
            "discover_models",
            true,
            "The Provider returned an invalid model-list response.",
        )
    })?;
    let data = value.get("data").and_then(Value::as_array).ok_or_else(|| {
        provider_error(
            profile,
            ProviderErrorCode::IncompatibleProtocol,
            "discover_models",
            true,
            "The Provider does not expose an OpenAI-compatible model list.",
        )
    })?;
    let mut models = data
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .filter(|id| !id.trim().is_empty() && id.len() <= 512 && !id.contains(['\r', '\n']))
        .map(|id| DiscoveredProviderModel {
            remote_model_id: id.to_owned(),
        })
        .collect::<Vec<_>>();
    models.sort_by(|left, right| left.remote_model_id.cmp(&right.remote_model_id));
    models.dedup_by(|left, right| left.remote_model_id == right.remote_model_id);
    Ok((models, duration_ms(started)))
}

pub async fn passive_provider_check(
    profile: &ProviderProfile,
    credential: Option<&SecretValue>,
) -> Result<ProviderPassiveCheck, ProviderErrorDetails> {
    let (models, latency_ms) = discover_provider_models(profile, credential).await?;
    Ok(ProviderPassiveCheck {
        reachable: true,
        protocol_compatible: true,
        discovered_model_count: models.len(),
        latency_ms,
        safe_message: if profile.adapter == ProviderAdapterKind::Mock {
            "Mock Provider is available offline.".to_owned()
        } else {
            "Connection succeeded without a billable generation request.".to_owned()
        },
    })
}

pub async fn active_provider_probe(
    profile: &ProviderProfile,
    credential: Option<&SecretValue>,
    remote_model_id: &str,
) -> Result<ProviderActiveProbe, ProviderErrorDetails> {
    profile.validate().map_err(|_| {
        provider_error(
            profile,
            ProviderErrorCode::InvalidEndpoint,
            "active_probe",
            false,
            "Provider connection settings are invalid.",
        )
    })?;
    if profile.adapter == ProviderAdapterKind::Mock {
        return Ok(ProviderActiveProbe {
            request_id: Some("mock-active-probe".to_owned()),
            input_tokens: Some(1),
            output_tokens: Some(1),
            total_tokens: Some(2),
            latency_ms: 0,
            safe_message: "Mock model probe completed offline.".to_owned(),
        });
    }
    let credential = credential.ok_or_else(|| {
        provider_error(
            profile,
            ProviderErrorCode::MissingCredential,
            "active_probe",
            true,
            "Configure a credential before running a billable model probe.",
        )
    })?;
    if remote_model_id.trim().is_empty() || remote_model_id.len() > 512 {
        return Err(provider_error(
            profile,
            ProviderErrorCode::ModelNotFound,
            "active_probe",
            false,
            "Choose a valid Model Profile before running a probe.",
        ));
    }
    let client = registry_client(profile)?;
    let started = Instant::now();
    let response = authorized_request(
        client.post(endpoint_url(profile, "chat/completions")?),
        profile,
        credential,
    )
    .json(&json!({
        "model": remote_model_id,
        "messages": [{"role": "user", "content": "Reply with OK."}],
        "max_tokens": 1,
        "temperature": 0
    }))
    .send()
    .await
    .map_err(|error| transport_error(profile, "active_probe", &error))?;
    let request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.len() <= 256)
        .map(str::to_owned);
    let response = successful_response(profile, "active_probe", response)?;
    let body = read_limited(profile, "active_probe", response).await?;
    let value: Value = serde_json::from_slice(&body).map_err(|_| {
        provider_error(
            profile,
            ProviderErrorCode::InvalidResponse,
            "active_probe",
            true,
            "The Provider returned an invalid generation response.",
        )
    })?;
    if value.get("choices").and_then(Value::as_array).is_none() {
        return Err(provider_error(
            profile,
            ProviderErrorCode::IncompatibleProtocol,
            "active_probe",
            true,
            "The Provider response is not OpenAI Chat Completions compatible.",
        ));
    }
    let usage = value.get("usage");
    let input_tokens = usage
        .and_then(|usage| usage.get("prompt_tokens"))
        .and_then(Value::as_u64);
    let output_tokens = usage
        .and_then(|usage| usage.get("completion_tokens"))
        .and_then(Value::as_u64);
    let total_tokens = usage
        .and_then(|usage| usage.get("total_tokens"))
        .and_then(Value::as_u64)
        .or_else(|| {
            input_tokens
                .zip(output_tokens)
                .map(|(input, output)| input + output)
        });
    Ok(ProviderActiveProbe {
        request_id,
        input_tokens,
        output_tokens,
        total_tokens,
        latency_ms: duration_ms(started),
        safe_message: "Billable model probe completed successfully.".to_owned(),
    })
}

fn registry_client(profile: &ProviderProfile) -> Result<Client, ProviderErrorDetails> {
    Client::builder()
        .timeout(Duration::from_secs(
            profile.connection_policy.request_timeout_seconds,
        ))
        // Redirects are rejected so authorization can never be forwarded to another origin.
        .redirect(Policy::none())
        .build()
        .map_err(|_| {
            provider_error(
                profile,
                ProviderErrorCode::Unreachable,
                "build_client",
                true,
                "The Provider HTTP client could not be initialized.",
            )
        })
}

fn endpoint_url(
    profile: &ProviderProfile,
    suffix: &str,
) -> Result<reqwest::Url, ProviderErrorDetails> {
    let mut base = profile.base_url.clone();
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    base.join(suffix).map_err(|_| {
        provider_error(
            profile,
            ProviderErrorCode::InvalidEndpoint,
            suffix,
            false,
            "The Provider endpoint could not be resolved.",
        )
    })
}

fn authorized_request(
    mut request: reqwest::RequestBuilder,
    profile: &ProviderProfile,
    credential: &SecretValue,
) -> reqwest::RequestBuilder {
    request = request.bearer_auth(credential.expose_secret());
    for (name, value) in &profile.safe_headers {
        request = request.header(name, value);
    }
    if let Some(organization) = &profile.organization
        && let Ok(value) = HeaderValue::from_str(organization)
    {
        request = request.header("openai-organization", value);
    }
    if let Some(workspace) = &profile.workspace
        && let Ok(value) = HeaderValue::from_str(workspace)
    {
        request = request.header("openai-project", value);
    }
    request
}

fn successful_response(
    profile: &ProviderProfile,
    operation: &str,
    response: Response,
) -> Result<Response, ProviderErrorDetails> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let (code, recoverable, message) = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => (
            ProviderErrorCode::InvalidCredential,
            true,
            "The Provider rejected the configured credential.",
        ),
        StatusCode::NOT_FOUND => (
            ProviderErrorCode::ModelNotFound,
            true,
            "The requested Provider endpoint or model was not found.",
        ),
        StatusCode::TOO_MANY_REQUESTS => (
            ProviderErrorCode::RateLimited,
            true,
            "The Provider rate-limited this request.",
        ),
        _ if status.is_server_error() => (
            ProviderErrorCode::Unreachable,
            true,
            "The Provider is temporarily unavailable.",
        ),
        _ => (
            ProviderErrorCode::IncompatibleProtocol,
            false,
            "The Provider rejected the OpenAI-compatible request.",
        ),
    };
    Err(provider_error(
        profile,
        code,
        operation,
        recoverable,
        message,
    ))
}

async fn read_limited(
    profile: &ProviderProfile,
    operation: &str,
    response: Response,
) -> Result<Vec<u8>, ProviderErrorDetails> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_REGISTRY_RESPONSE_BYTES as u64)
    {
        return Err(provider_error(
            profile,
            ProviderErrorCode::ResponseTooLarge,
            operation,
            false,
            "The Provider response exceeded the safe size limit.",
        ));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| transport_error(profile, operation, &error))?;
        if bytes.len().saturating_add(chunk.len()) > MAX_REGISTRY_RESPONSE_BYTES {
            return Err(provider_error(
                profile,
                ProviderErrorCode::ResponseTooLarge,
                operation,
                false,
                "The Provider response exceeded the safe size limit.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn transport_error(
    profile: &ProviderProfile,
    operation: &str,
    error: &reqwest::Error,
) -> ProviderErrorDetails {
    let (code, message) = if error.is_timeout() {
        (
            ProviderErrorCode::Timeout,
            "The Provider request timed out.",
        )
    } else if error.is_redirect() {
        (
            ProviderErrorCode::IncompatibleProtocol,
            "The Provider attempted a redirect, which AnnotAgent rejects for credential safety.",
        )
    } else {
        (
            ProviderErrorCode::Unreachable,
            "AnnotAgent could not reach the Provider endpoint.",
        )
    };
    provider_error(profile, code, operation, true, message)
}

fn provider_error(
    profile: &ProviderProfile,
    code: ProviderErrorCode,
    operation: &str,
    recoverable: bool,
    safe_message: &str,
) -> ProviderErrorDetails {
    ProviderErrorDetails {
        code,
        provider_id: profile.id,
        model_profile_id: None,
        operation: operation.to_owned(),
        recoverable,
        retry_after_ms: None,
        safe_message: safe_message.to_owned(),
    }
}

fn duration_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use annotagent_core::{
        ProviderConnectionPolicy, ProviderHealthSnapshot, ProviderId, ProviderProfile,
    };
    use axum::{
        Json, Router,
        http::HeaderMap,
        routing::{get, post},
    };
    use chrono::Utc;
    use serde_json::json;

    use super::*;

    async fn spawn_fixture() -> String {
        async fn models(headers: HeaderMap) -> Json<Value> {
            assert_eq!(
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer fixture-secret")
            );
            Json(json!({"data": [{"id": "vision-b"}, {"id": "vision-a"}]}))
        }
        async fn completion() -> (HeaderMap, Json<Value>) {
            let mut headers = HeaderMap::new();
            headers.insert("x-request-id", HeaderValue::from_static("probe-1"));
            (
                headers,
                Json(json!({
                    "choices": [{"message": {"role": "assistant", "content": "OK"}}],
                    "usage": {"prompt_tokens": 4, "completion_tokens": 1, "total_tokens": 5}
                })),
            )
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/v1/models", get(models))
                    .route("/v1/chat/completions", post(completion)),
            )
            .await
            .expect("fixture server");
        });
        format!("http://{address}/v1")
    }

    fn profile(base_url: &str) -> ProviderProfile {
        let now = Utc::now();
        ProviderProfile {
            id: ProviderId::new(),
            display_name: "Fixture".to_owned(),
            preset_id: None,
            adapter: ProviderAdapterKind::OpenAiCompatible,
            base_url: base_url.parse().expect("URL"),
            organization: None,
            workspace: None,
            credential_ref: None,
            safe_headers: BTreeMap::new(),
            connection_policy: ProviderConnectionPolicy::default(),
            enabled: true,
            health: ProviderHealthSnapshot::default(),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn discovery_and_active_probe_are_bounded_protocol_requests() {
        let profile = profile(&spawn_fixture().await);
        let secret = SecretValue::new("fixture-secret").expect("secret");
        let (models, _) = discover_provider_models(&profile, Some(&secret))
            .await
            .expect("discovery");
        assert_eq!(
            models
                .iter()
                .map(|model| model.remote_model_id.as_str())
                .collect::<Vec<_>>(),
            vec!["vision-a", "vision-b"]
        );
        let probe = active_provider_probe(&profile, Some(&secret), "vision-a")
            .await
            .expect("probe");
        assert_eq!(probe.request_id.as_deref(), Some("probe-1"));
        assert_eq!(probe.total_tokens, Some(5));
    }
}
