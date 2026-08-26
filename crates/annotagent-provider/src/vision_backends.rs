use std::{collections::BTreeMap, sync::Arc, time::Duration};

use annotagent_core::{
    ArtifactId, CoreError, CoreResult, ModelMessage, ModelRequest, ModelRole, ToolDefinition,
    VisionArtifact, VisionBackendKind, VisionCapability, VisionInferenceRequest,
    VisionInferenceResponse, VisionModelBackend, VisionModelProvider,
};
use async_trait::async_trait;
use reqwest::Client;
use tokio_util::sync::CancellationToken;

pub struct MockVisionBackend {
    id: String,
    capabilities: Vec<VisionCapability>,
    artifacts: Vec<VisionArtifact>,
}

impl MockVisionBackend {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        capabilities: Vec<VisionCapability>,
        artifacts: Vec<VisionArtifact>,
    ) -> Self {
        Self {
            id: id.into(),
            capabilities,
            artifacts,
        }
    }
}

#[async_trait]
impl VisionModelBackend for MockVisionBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> VisionBackendKind {
        VisionBackendKind::Mock
    }

    fn capabilities(&self) -> Vec<VisionCapability> {
        self.capabilities.clone()
    }

    async fn infer(
        &self,
        request: VisionInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<VisionInferenceResponse> {
        if cancellation.is_cancelled() {
            return Err(CoreError::Provider("vision inference cancelled".to_owned()));
        }
        let mut artifacts = self.artifacts.clone();
        for artifact in &mut artifacts {
            artifact.id = ArtifactId::new();
            artifact.image_id = request.image_id;
            artifact.task_id = Some(request.task_id.clone());
            if artifact.source_node.trim().is_empty() {
                artifact.source_node.clone_from(&request.node_id);
            }
            artifact.validate()?;
        }
        Ok(VisionInferenceResponse {
            artifacts,
            request_id: Some(format!("mock-{}", request.run_id)),
            metadata: BTreeMap::from([("backend".to_owned(), serde_json::json!(self.id))]),
        })
    }
}

#[derive(Debug, Clone)]
pub struct HttpJsonVisionBackendConfig {
    pub id: String,
    pub endpoint: String,
    pub capabilities: Vec<VisionCapability>,
    pub request_timeout: Duration,
    pub authorization: Option<String>,
}

pub struct HttpJsonVisionBackend {
    config: HttpJsonVisionBackendConfig,
    client: Client,
}

impl HttpJsonVisionBackend {
    pub fn new(config: HttpJsonVisionBackendConfig) -> CoreResult<Self> {
        let client = Client::builder()
            .timeout(config.request_timeout)
            .build()
            .map_err(|error| CoreError::Provider(format!("cannot build HTTP client: {error}")))?;
        Ok(Self { config, client })
    }
}

#[async_trait]
impl VisionModelBackend for HttpJsonVisionBackend {
    fn id(&self) -> &str {
        &self.config.id
    }

    fn kind(&self) -> VisionBackendKind {
        VisionBackendKind::HttpJson
    }

    fn capabilities(&self) -> Vec<VisionCapability> {
        self.config.capabilities.clone()
    }

    async fn infer(
        &self,
        request: VisionInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<VisionInferenceResponse> {
        let mut builder = self.client.post(&self.config.endpoint).json(&request);
        if let Some(authorization) = &self.config.authorization {
            builder = builder.header(reqwest::header::AUTHORIZATION, authorization);
        }
        let response = tokio::select! {
            () = cancellation.cancelled() => {
                return Err(CoreError::Provider("vision inference cancelled".to_owned()))
            },
            response = builder.send() => response,
        }
        .map_err(|error| CoreError::Provider(format!("HTTP vision backend failed: {error}")))?;
        let status = response.status();
        if !status.is_success() {
            let detail = response.text().await.unwrap_or_default();
            return Err(CoreError::Provider(format!(
                "HTTP vision backend returned {status}: {}",
                truncate(&detail, 500)
            )));
        }
        let response = response
            .json::<VisionInferenceResponse>()
            .await
            .map_err(|error| {
                CoreError::Provider(format!("invalid HTTP vision response schema: {error}"))
            })?;
        validate_backend_response(&request, &response)?;
        Ok(response)
    }
}

pub struct OpenAiVisionBackend {
    id: String,
    model: String,
    provider: Arc<dyn VisionModelProvider>,
    max_output_tokens: u32,
    temperature: f32,
}

impl OpenAiVisionBackend {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        model: impl Into<String>,
        provider: Arc<dyn VisionModelProvider>,
        max_output_tokens: u32,
        temperature: f32,
    ) -> Self {
        Self {
            id: id.into(),
            model: model.into(),
            provider,
            max_output_tokens,
            temperature,
        }
    }
}

#[async_trait]
impl VisionModelBackend for OpenAiVisionBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> VisionBackendKind {
        VisionBackendKind::OpenAiCompatible
    }

    fn capabilities(&self) -> Vec<VisionCapability> {
        vec![VisionCapability::VisionLanguage]
    }

    async fn infer(
        &self,
        request: VisionInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<VisionInferenceResponse> {
        let system = "Return only JSON matching VisionInferenceResponse. Artifacts must use the supplied image_id/task_id and a registered typed artifact kind.";
        let user = request.prompt.clone().unwrap_or_else(|| {
            format!(
                "Execute vision node {:?} for task {:?}.",
                request.node_id, request.task_id
            )
        });
        let response = self
            .provider
            .complete(
                ModelRequest {
                    model: self.model.clone(),
                    task_id: request.task_id.clone(),
                    messages: vec![
                        ModelMessage {
                            role: ModelRole::System,
                            content: system.to_owned(),
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                        },
                        ModelMessage {
                            role: ModelRole::User,
                            content: user,
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                        },
                    ],
                    images: request.image.clone().into_iter().collect(),
                    tools: Vec::<ToolDefinition>::new(),
                    max_output_tokens: self.max_output_tokens,
                    temperature: self.temperature,
                    extra: BTreeMap::new(),
                },
                cancellation,
            )
            .await?;
        if !response.tool_calls.is_empty() {
            return Err(CoreError::Provider(
                "OpenAI vision backend expected JSON content, not tool calls".to_owned(),
            ));
        }
        let content = response.content.ok_or_else(|| {
            CoreError::Provider("OpenAI vision backend returned empty content".to_owned())
        })?;
        let mut decoded: VisionInferenceResponse =
            serde_json::from_str(&content).map_err(|error| {
                CoreError::Provider(format!("invalid OpenAI vision response schema: {error}"))
            })?;
        decoded.request_id = decoded.request_id.or(response.request_id);
        validate_backend_response(&request, &decoded)?;
        Ok(decoded)
    }
}

fn validate_backend_response(
    request: &VisionInferenceRequest,
    response: &VisionInferenceResponse,
) -> CoreResult<()> {
    for artifact in &response.artifacts {
        if artifact.image_id != request.image_id
            || artifact.task_id.as_ref() != Some(&request.task_id)
        {
            return Err(CoreError::Provider(format!(
                "artifact {} is outside the requested image/task scope",
                artifact.id
            )));
        }
        artifact.validate()?;
    }
    Ok(())
}

fn truncate(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use annotagent_core::{
        ArtifactProvenance, ArtifactRole, ArtifactValidationState, ImageId, LabelId, RunId, TaskId,
        VisionArtifactValue,
    };
    use axum::{Json, Router, routing::post};

    use crate::{MockResponseSpec, MockScript, MockStep, MockUsage, MockVisionProvider};

    use super::*;

    fn artifact(image_id: ImageId, task_id: &TaskId) -> VisionArtifact {
        VisionArtifact {
            id: ArtifactId::new(),
            image_id,
            task_id: Some(task_id.clone()),
            label: Some(LabelId::from("target")),
            role: ArtifactRole::Candidate,
            value: VisionArtifactValue::Classification {
                labels: vec![LabelId::from("target")],
            },
            source_node: "test.classifier".to_owned(),
            confidence: Some(0.9),
            metadata: BTreeMap::new(),
            validation_state: ArtifactValidationState::Unvalidated,
            provenance: ArtifactProvenance::default(),
            revision: 1,
            replaces_artifact_id: None,
            created_at: chrono::Utc::now(),
        }
    }

    fn request() -> VisionInferenceRequest {
        VisionInferenceRequest {
            run_id: RunId::new(),
            image_id: ImageId::new(),
            task_id: TaskId::from("classification"),
            node_id: "classifier".to_owned(),
            model_id: "model".to_owned(),
            image: None,
            input_artifacts: Vec::new(),
            prompt: Some("classify".to_owned()),
            parameters: BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn mock_backend_scopes_fresh_typed_artifacts() {
        let request = request();
        let template = artifact(ImageId::new(), &TaskId::from("old"));
        let old_id = template.id;
        let backend = MockVisionBackend::new(
            "mock",
            vec![VisionCapability::Classification],
            vec![template],
        );
        let response = backend
            .infer(request.clone(), CancellationToken::new())
            .await
            .expect("mock inference");
        assert_ne!(response.artifacts[0].id, old_id);
        assert_eq!(response.artifacts[0].image_id, request.image_id);
        assert_eq!(response.artifacts[0].task_id, Some(request.task_id));
    }

    #[tokio::test]
    async fn http_json_backend_uses_the_shared_wire_schema() {
        async fn infer(
            Json(request): Json<VisionInferenceRequest>,
        ) -> Json<VisionInferenceResponse> {
            Json(VisionInferenceResponse {
                artifacts: vec![artifact(request.image_id, &request.task_id)],
                request_id: Some("worker-request".to_owned()),
                metadata: BTreeMap::new(),
            })
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test worker");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/infer", post(infer)))
                .await
                .expect("test worker");
        });
        let backend = HttpJsonVisionBackend::new(HttpJsonVisionBackendConfig {
            id: "python-worker".to_owned(),
            endpoint: format!("http://{address}/infer"),
            capabilities: vec![VisionCapability::Classification],
            request_timeout: Duration::from_secs(2),
            authorization: None,
        })
        .expect("backend");
        let response = backend
            .infer(request(), CancellationToken::new())
            .await
            .expect("HTTP inference");
        assert_eq!(response.request_id.as_deref(), Some("worker-request"));
        assert_eq!(response.artifacts.len(), 1);
    }

    #[tokio::test]
    async fn openai_adapter_requires_and_parses_typed_response_json() {
        let request = request();
        let expected = VisionInferenceResponse {
            artifacts: vec![artifact(request.image_id, &request.task_id)],
            request_id: None,
            metadata: BTreeMap::new(),
        };
        let provider = Arc::new(MockVisionProvider::new(MockScript {
            steps: vec![MockStep {
                expect_task: Some(request.task_id.to_string()),
                expect_message_contains: Some("classify".to_owned()),
                response: MockResponseSpec::Content {
                    content: serde_json::to_string(&expected).expect("response JSON"),
                },
                usage: MockUsage {
                    input_tokens: 10,
                    output_tokens: 10,
                },
            }],
        }));
        let backend = OpenAiVisionBackend::new("openai", "vision", provider, 500, 0.0);
        let response = backend
            .infer(request, CancellationToken::new())
            .await
            .expect("OpenAI adapter");
        assert_eq!(response.artifacts, expected.artifacts);
    }
}
