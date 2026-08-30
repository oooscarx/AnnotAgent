//! Versioned, bounded HTTP client and Detection adapter for untrusted Vision Workers.

use std::{collections::BTreeSet, time::Duration};

use annotagent_core::{
    ArtifactId, ArtifactKind, ArtifactProvenance, ArtifactRef, ArtifactRole,
    ArtifactValidationState, CoreError, CoreResult, DETECTION_ARTIFACT_SCHEMA_VERSION, Detection,
    DetectionScore, DetectionSetArtifact, DetectionSource, DetectionWorkerCancelRequest,
    DetectionWorkerCancelResponse, DetectionWorkerCapabilities, DetectionWorkerHealth,
    DetectionWorkerInferenceRequest, DetectionWorkerInferenceResponse, DetectionWorkerOptions,
    DetectionWorkerQuery, LabelId, NormalizedRect, PIPELINE_VISION_PROTOCOL_VERSION,
    PipelineArtifact, PipelineInferenceRequest, PipelineInferenceResponse, PipelineModelBackend,
    ScoreSemantics, StoredPayloadRef, VISION_WORKER_PROTOCOL_VERSION, VisionArtifact,
    VisionArtifactValue, VisionBackendKind, VisionBackendTimings, VisionBackendUsage,
    VisionCapability, VisionInferenceRequest, VisionInferenceResponse, VisionModelBackend,
};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::{Client, Url, header::HeaderValue};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use crate::http_transport::{
    bounded_response_body, build_worker_client, endpoint, validate_transport_limits,
    validate_worker_base_url,
};

const MAX_REQUEST_ID_BYTES: usize = 128;
const MAX_QUERY_COUNT: usize = 100;
const MAX_QUERY_TEXT_BYTES: usize = 2_000;
const MAX_WARNING_COUNT: usize = 100;
const MAX_WARNING_BYTES: usize = 1_000;
const MAX_INLINE_IMAGE_BYTES_UPPER_BOUND: usize = 64_000_000;

#[derive(Debug, Clone)]
pub struct HttpVisionWorkerConfig {
    pub id: String,
    pub base_url: String,
    pub expected_model_id: String,
    pub capabilities: Vec<VisionCapability>,
    pub expected_score_semantics: Option<ScoreSemantics>,
    pub request_timeout: Duration,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_retries: u32,
    pub allow_remote: bool,
    pub authorization: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectionWorkerResult {
    pub response: DetectionWorkerInferenceResponse,
    pub raw_output_ref: StoredPayloadRef,
}

pub struct HttpVisionWorkerClient {
    config: HttpVisionWorkerConfig,
    base_url: Url,
    client: Client,
    authorization: Option<HeaderValue>,
}

impl HttpVisionWorkerClient {
    pub fn new(config: HttpVisionWorkerConfig) -> CoreResult<Self> {
        if config.id.trim().is_empty() || config.expected_model_id.trim().is_empty() {
            return Err(CoreError::Validation(
                "HTTP Vision Worker requires non-empty worker and model ids".to_owned(),
            ));
        }
        validate_detection_capabilities(&config.capabilities)?;
        validate_transport_limits(config.max_response_bytes, config.max_retries)?;
        if config.max_request_bytes == 0
            || config.max_request_bytes > MAX_INLINE_IMAGE_BYTES_UPPER_BOUND
        {
            return Err(CoreError::Validation(format!(
                "HTTP Vision Worker max_request_bytes must be within 1..={MAX_INLINE_IMAGE_BYTES_UPPER_BOUND}"
            )));
        }
        let base_url = validate_worker_base_url(&config.base_url, config.allow_remote)?;
        let client = build_worker_client(config.request_timeout)?;
        let authorization = config
            .authorization
            .as_deref()
            .map(HeaderValue::from_str)
            .transpose()
            .map_err(|_| {
                CoreError::Validation(
                    "HTTP Vision Worker authorization header is invalid".to_owned(),
                )
            })?;
        Ok(Self {
            config,
            base_url,
            client,
            authorization,
        })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.config.id
    }

    #[must_use]
    pub fn capabilities(&self) -> &[VisionCapability] {
        &self.config.capabilities
    }

    pub async fn health(&self) -> CoreResult<DetectionWorkerHealth> {
        let response = self
            .authorized(self.client.get(endpoint(&self.base_url, "health")?))
            .send()
            .await
            .map_err(|error| worker_transport_error(&self.config.id, "health", &error))?;
        let (status, body) =
            bounded_response_body(response, self.config.max_response_bytes).await?;
        if !status.is_success() {
            return Err(worker_failure(
                &self.config.id,
                "health",
                &format!("http_{}", status.as_u16()),
            ));
        }
        let health: DetectionWorkerHealth = serde_json::from_slice(&body)
            .map_err(|_| worker_failure(&self.config.id, "health", "malformed_response"))?;
        validate_health(&self.config, &health)?;
        Ok(health)
    }

    pub async fn discover_capabilities(&self) -> CoreResult<DetectionWorkerCapabilities> {
        let response = self
            .authorized(
                self.client
                    .get(endpoint(&self.base_url, "v1/capabilities")?),
            )
            .send()
            .await
            .map_err(|error| worker_transport_error(&self.config.id, "capabilities", &error))?;
        let (status, body) =
            bounded_response_body(response, self.config.max_response_bytes).await?;
        if !status.is_success() {
            return Err(worker_failure(
                &self.config.id,
                "capabilities",
                &format!("http_{}", status.as_u16()),
            ));
        }
        let capabilities: DetectionWorkerCapabilities = serde_json::from_slice(&body)
            .map_err(|_| worker_failure(&self.config.id, "capabilities", "malformed_response"))?;
        validate_capabilities(&self.config, &capabilities)?;
        Ok(capabilities)
    }

    pub async fn infer(
        &self,
        request: DetectionWorkerInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<DetectionWorkerResult> {
        validate_inference_request(&self.config, &request)?;
        let request_body = serde_json::to_vec(&request).map_err(|error| {
            CoreError::Validation(format!(
                "cannot serialize HTTP Vision Worker request: {error}"
            ))
        })?;
        if request_body.len() > self.config.max_request_bytes {
            return Err(CoreError::Validation(format!(
                "HTTP Vision Worker request exceeds {} bytes",
                self.config.max_request_bytes
            )));
        }
        let attempts = self.config.max_retries.saturating_add(1);
        for attempt in 0..attempts {
            let send = self.send_inference(&request, &request_body);
            let result = tokio::select! {
                () = cancellation.cancelled() => {
                    let _ = tokio::time::timeout(
                        Duration::from_millis(500),
                        self.cancel(&request.request_id),
                    )
                    .await;
                    return Err(worker_failure(&self.config.id, &request.request_id, "cancelled"));
                }
                result = send => result,
            };
            let (status, body) = match result {
                Ok(result) => result,
                Err(error) if attempt + 1 < attempts => {
                    let _ = error;
                    continue;
                }
                Err(error) => return Err(error),
            };
            let parsed = serde_json::from_slice::<DetectionWorkerInferenceResponse>(&body);
            let response = match parsed {
                Ok(response) => response,
                Err(_) if status.is_server_error() && attempt + 1 < attempts => continue,
                Err(_) => {
                    return Err(worker_failure(
                        &self.config.id,
                        &request.request_id,
                        "malformed_response",
                    ));
                }
            };
            validate_response_scope(&self.config, &request, &response)?;
            if let Some(error) = &response.error {
                if error.retryable && attempt + 1 < attempts {
                    continue;
                }
                return Err(worker_failure(
                    &self.config.id,
                    &request.request_id,
                    &error.code,
                ));
            }
            if !status.is_success() {
                return Err(worker_failure(
                    &self.config.id,
                    &request.request_id,
                    &format!("http_{}", status.as_u16()),
                ));
            }
            validate_inference_response(&self.config, &request, &response)?;
            let raw_output_ref = StoredPayloadRef {
                id: format!("worker-response:{}", request.request_id),
                media_type: "application/json".to_owned(),
                sha256: format!("{:x}", Sha256::digest(&body)),
                size_bytes: u64::try_from(body.len()).unwrap_or(u64::MAX),
            };
            return Ok(DetectionWorkerResult {
                response,
                raw_output_ref,
            });
        }
        Err(worker_failure(
            &self.config.id,
            &request.request_id,
            "retries_exhausted",
        ))
    }

    pub async fn cancel(&self, request_id: &str) -> CoreResult<DetectionWorkerCancelResponse> {
        validate_request_id(request_id)?;
        let request = DetectionWorkerCancelRequest {
            protocol_version: VISION_WORKER_PROTOCOL_VERSION,
            request_id: request_id.to_owned(),
            model_id: self.config.expected_model_id.clone(),
        };
        let response = self
            .authorized(
                self.client
                    .post(endpoint(&self.base_url, "v1/cancel")?)
                    .json(&request),
            )
            .timeout(Duration::from_millis(500))
            .send()
            .await
            .map_err(|error| worker_transport_error(&self.config.id, request_id, &error))?;
        let (status, body) =
            bounded_response_body(response, self.config.max_response_bytes).await?;
        if !status.is_success() {
            return Err(worker_failure(
                &self.config.id,
                request_id,
                &format!("cancel_http_{}", status.as_u16()),
            ));
        }
        let response: DetectionWorkerCancelResponse =
            serde_json::from_slice(&body).map_err(|_| {
                worker_failure(&self.config.id, request_id, "malformed_cancel_response")
            })?;
        if response.protocol_version != VISION_WORKER_PROTOCOL_VERSION
            || response.request_id != request_id
        {
            return Err(worker_failure(
                &self.config.id,
                request_id,
                "cancel_scope_mismatch",
            ));
        }
        Ok(response)
    }

    async fn send_inference(
        &self,
        request: &DetectionWorkerInferenceRequest,
        body: &[u8],
    ) -> CoreResult<(reqwest::StatusCode, Vec<u8>)> {
        let mut builder = self.authorized(
            self.client
                .post(endpoint(&self.base_url, "v1/infer")?)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_vec()),
        );
        if let Some(timeout_ms) = request.timeout_ms {
            builder =
                builder.timeout(Duration::from_millis(timeout_ms).min(self.config.request_timeout));
        }
        let response = builder.send().await.map_err(|error| {
            worker_transport_error(&self.config.id, &request.request_id, &error)
        })?;
        bounded_response_body(response, self.config.max_response_bytes).await
    }

    fn authorized(&self, mut request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(value) = &self.authorization {
            request = request.header(reqwest::header::AUTHORIZATION, value);
        }
        request
    }
}

pub struct HttpVisionDetectionBackend {
    client: HttpVisionWorkerClient,
    capability: VisionCapability,
}

/// Registry-facing adapter for a Worker that advertises more than one detection capability.
/// Pipeline execution still binds one capability per node through `HttpVisionDetectionBackend`.
pub struct HttpVisionWorkerRegistryBackend {
    client: HttpVisionWorkerClient,
}

impl HttpVisionWorkerRegistryBackend {
    pub fn new(config: HttpVisionWorkerConfig) -> CoreResult<Self> {
        Ok(Self {
            client: HttpVisionWorkerClient::new(config)?,
        })
    }

    pub async fn health(&self) -> CoreResult<DetectionWorkerHealth> {
        self.client.health().await
    }

    pub async fn discover_capabilities(&self) -> CoreResult<DetectionWorkerCapabilities> {
        self.client.discover_capabilities().await
    }
}

#[async_trait]
impl VisionModelBackend for HttpVisionWorkerRegistryBackend {
    fn id(&self) -> &str {
        self.client.id()
    }

    fn kind(&self) -> VisionBackendKind {
        VisionBackendKind::HttpVision
    }

    fn capabilities(&self) -> Vec<VisionCapability> {
        self.client.capabilities().to_vec()
    }

    async fn infer(
        &self,
        request: VisionInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<VisionInferenceResponse> {
        if request.protocol_version != VISION_WORKER_PROTOCOL_VERSION
            || !self.client.capabilities().contains(&request.operation)
            || request.model_id != self.client.config.expected_model_id
        {
            return Err(CoreError::Validation(
                "Registry Detection request protocol, model, or capability mismatch".to_owned(),
            ));
        }
        let worker_request = DetectionWorkerInferenceRequest {
            protocol_version: VISION_WORKER_PROTOCOL_VERSION,
            request_id: request.request_id.clone(),
            operation: request.operation,
            model_id: request.model_id.clone(),
            image: request.image.ok_or_else(|| {
                CoreError::Validation("Registry Detection requires inline image input".to_owned())
            })?,
            queries: parse_queries(&request.parameters)?,
            target_labels: parse_target_labels(&request.parameters)?,
            options: parse_options(&request.parameters)?,
            timeout_ms: request.timeout_ms,
        };
        let result = self.client.infer(worker_request, cancellation).await?;
        let mut artifacts = Vec::with_capacity(result.response.detections.len());
        for detection in result.response.detections {
            let score = DetectionScore::new(detection.score, detection.score_semantics)
                .map_err(CoreError::Validation)?;
            let mut metadata = std::collections::BTreeMap::from([
                (
                    "score_semantics".to_owned(),
                    serde_json::json!(score.semantics),
                ),
                (
                    "raw_output_ref".to_owned(),
                    serde_json::to_value(&result.raw_output_ref).map_err(|error| {
                        CoreError::Validation(format!(
                            "cannot serialize controlled Worker response reference: {error}"
                        ))
                    })?,
                ),
            ]);
            if let Some(query_id) = detection.query_id {
                metadata.insert("query_id".to_owned(), serde_json::json!(query_id));
            }
            if let Some(model_label) = detection.model_label {
                metadata.insert("model_label".to_owned(), serde_json::json!(model_label));
            }
            let artifact = VisionArtifact {
                id: ArtifactId::new(),
                image_id: request.image_id,
                task_id: Some(request.task_id.clone()),
                label: detection.target_label,
                role: ArtifactRole::Candidate,
                value: VisionArtifactValue::BoundingBox {
                    rect: normalized_xyxy(detection.bbox_xyxy_normalized)?,
                },
                source_node: request.node_id.clone(),
                confidence: score.comparable_confidence(),
                metadata,
                validation_state: ArtifactValidationState::Unvalidated,
                provenance: ArtifactProvenance {
                    provider: Some(self.client.id().to_owned()),
                    model: Some(request.model_id.clone()),
                    request_id: Some(request.request_id.clone()),
                    ..ArtifactProvenance::default()
                },
                revision: 1,
                replaces_artifact_id: None,
                created_at: chrono::Utc::now(),
            };
            artifact.validate()?;
            artifacts.push(artifact);
        }
        Ok(VisionInferenceResponse {
            request_id: Some(request.request_id),
            model_identity: Some(result.response.model_id),
            artifacts,
            usage: VisionBackendUsage {
                source: Some("worker_reported".to_owned()),
                compute_milliseconds: result.response.usage.duration_ms,
                input_megapixels: None,
            },
            timings: VisionBackendTimings {
                total_ms: result.response.usage.duration_ms,
                ..VisionBackendTimings::default()
            },
            warnings: result.response.warnings,
            ..VisionInferenceResponse::default()
        })
    }
}

impl HttpVisionDetectionBackend {
    pub fn new(config: HttpVisionWorkerConfig, capability: VisionCapability) -> CoreResult<Self> {
        if !is_detection_capability(capability) {
            return Err(CoreError::Validation(
                "HTTP Vision Detection Backend requires a detection capability".to_owned(),
            ));
        }
        if !config.capabilities.contains(&capability) {
            return Err(CoreError::Validation(
                "HTTP Vision Detection Backend capability is not configured for the Worker"
                    .to_owned(),
            ));
        }
        Ok(Self {
            client: HttpVisionWorkerClient::new(config)?,
            capability,
        })
    }

    pub async fn health(&self) -> CoreResult<DetectionWorkerHealth> {
        self.client.health().await
    }

    pub async fn discover_capabilities(&self) -> CoreResult<DetectionWorkerCapabilities> {
        self.client.discover_capabilities().await
    }
}

#[async_trait]
impl PipelineModelBackend for HttpVisionDetectionBackend {
    fn id(&self) -> &str {
        self.client.id()
    }

    fn capability(&self) -> VisionCapability {
        self.capability
    }

    async fn infer_pipeline(
        &self,
        request: PipelineInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<PipelineInferenceResponse> {
        if request.protocol_version != PIPELINE_VISION_PROTOCOL_VERSION
            || request.operation != self.capability
        {
            return Err(CoreError::Validation(
                "Pipeline request protocol or capability does not match HTTP Vision Backend"
                    .to_owned(),
            ));
        }
        let image = request.image.clone().ok_or_else(|| {
            CoreError::Validation("HTTP Vision Detection requires inline image input".to_owned())
        })?;
        let queries = parse_queries(&request.parameters)?;
        let target_labels = parse_target_labels(&request.parameters)?;
        let options = parse_options(&request.parameters)?;
        let worker_request = DetectionWorkerInferenceRequest {
            protocol_version: VISION_WORKER_PROTOCOL_VERSION,
            request_id: request.request_id.clone(),
            operation: request.operation,
            model_id: request.model_id.clone(),
            image,
            queries,
            target_labels,
            options,
            timeout_ms: request.timeout_ms,
        };
        let result = self.client.infer(worker_request, cancellation).await?;
        let artifact_id = format!("detection-set:{}", request.request_id);
        let mut detections = Vec::with_capacity(result.response.detections.len());
        for item in result.response.detections {
            let bbox = normalized_xyxy(item.bbox_xyxy_normalized)?;
            let score = DetectionScore::new(item.score, item.score_semantics)
                .map_err(CoreError::Validation)?;
            let mut detection = Detection::from_source(
                item.detection_id,
                item.query_id,
                item.model_label,
                item.target_label,
                bbox,
                score,
                DetectionSource {
                    model_id: request.model_id.clone(),
                    capability: request.operation,
                    artifact_id: artifact_id.clone(),
                },
            )
            .map_err(CoreError::Validation)?;
            detection.evidence[0].raw_output_ref = Some(result.raw_output_ref.clone());
            detections.push(detection);
        }
        let artifact = DetectionSetArtifact {
            schema_version: DETECTION_ARTIFACT_SCHEMA_VERSION,
            reference: ArtifactRef {
                artifact_id,
                source_node: request.node_id,
                port: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
                item_id: None,
            },
            image_id: request.image_id,
            model_binding: request.model_id,
            validation_state: ArtifactValidationState::Unvalidated,
            detections,
            metadata: std::collections::BTreeMap::from([
                ("worker_id".to_owned(), serde_json::json!(self.client.id())),
                (
                    "device".to_owned(),
                    serde_json::json!(result.response.usage.device),
                ),
            ]),
        };
        artifact.validate().map_err(CoreError::Validation)?;
        Ok(PipelineInferenceResponse {
            request_id: Some(request.request_id),
            model_identity: Some(result.response.model_id),
            artifacts: vec![PipelineArtifact::DetectionSet(artifact)],
            usage: VisionBackendUsage {
                source: Some("worker_reported".to_owned()),
                compute_milliseconds: result.response.usage.duration_ms,
                input_megapixels: None,
            },
            timings: VisionBackendTimings {
                total_ms: result.response.usage.duration_ms,
                ..VisionBackendTimings::default()
            },
            warnings: result.response.warnings,
            ..PipelineInferenceResponse::default()
        })
    }
}

fn validate_detection_capabilities(capabilities: &[VisionCapability]) -> CoreResult<()> {
    if capabilities.is_empty()
        || capabilities
            .iter()
            .any(|capability| !is_detection_capability(*capability))
        || capabilities.iter().collect::<BTreeSet<_>>().len() != capabilities.len()
    {
        return Err(CoreError::Validation(
            "HTTP Vision Worker capabilities must be unique detection capabilities".to_owned(),
        ));
    }
    Ok(())
}

const fn is_detection_capability(capability: VisionCapability) -> bool {
    matches!(
        capability,
        VisionCapability::ObjectDetection
            | VisionCapability::OpenVocabularyDetection
            | VisionCapability::PhraseGrounding
    )
}

fn validate_health(
    config: &HttpVisionWorkerConfig,
    health: &DetectionWorkerHealth,
) -> CoreResult<()> {
    if health.protocol_version != VISION_WORKER_PROTOCOL_VERSION
        || health.worker_id != config.id
        || health.model_id != config.expected_model_id
        || health
            .detail
            .as_deref()
            .is_some_and(|detail| detail.len() > MAX_WARNING_BYTES)
    {
        return Err(worker_failure(&config.id, "health", "identity_mismatch"));
    }
    Ok(())
}

fn validate_capabilities(
    config: &HttpVisionWorkerConfig,
    capabilities: &DetectionWorkerCapabilities,
) -> CoreResult<()> {
    if capabilities.protocol_version != VISION_WORKER_PROTOCOL_VERSION
        || capabilities.worker_id != config.id
        || capabilities.model_id != config.expected_model_id
    {
        return Err(worker_failure(
            &config.id,
            "capabilities",
            "identity_mismatch",
        ));
    }
    validate_detection_capabilities(&capabilities.capabilities)?;
    if !config
        .capabilities
        .iter()
        .all(|capability| capabilities.capabilities.contains(capability))
    {
        return Err(worker_failure(
            &config.id,
            "capabilities",
            "capability_mismatch",
        ));
    }
    if config
        .expected_score_semantics
        .is_some_and(|expected| expected != capabilities.score_semantics)
    {
        return Err(worker_failure(
            &config.id,
            "capabilities",
            "score_semantics_mismatch",
        ));
    }
    let labels = capabilities
        .label_space
        .iter()
        .map(|label| label.trim())
        .collect::<BTreeSet<_>>();
    if labels.len() != capabilities.label_space.len() || labels.contains("") {
        return Err(worker_failure(
            &config.id,
            "capabilities",
            "invalid_label_space",
        ));
    }
    if capabilities.label_space.len() > 10_000
        || capabilities
            .label_space
            .iter()
            .any(|label| label.len() > MAX_QUERY_TEXT_BYTES)
        || capabilities.limits.max_images == Some(0)
        || capabilities.limits.max_request_bytes == Some(0)
        || capabilities.limits.timeout_seconds == Some(0)
    {
        return Err(worker_failure(
            &config.id,
            "capabilities",
            "invalid_worker_limits",
        ));
    }
    Ok(())
}

fn validate_inference_request(
    config: &HttpVisionWorkerConfig,
    request: &DetectionWorkerInferenceRequest,
) -> CoreResult<()> {
    if request.protocol_version != VISION_WORKER_PROTOCOL_VERSION
        || request.model_id != config.expected_model_id
        || !config.capabilities.contains(&request.operation)
    {
        return Err(CoreError::Validation(
            "HTTP Vision Worker request protocol, model, or capability mismatch".to_owned(),
        ));
    }
    validate_request_id(&request.request_id)?;
    if request.timeout_ms == Some(0) {
        return Err(CoreError::Validation(
            "HTTP Vision Worker request timeout must be greater than zero".to_owned(),
        ));
    }
    if request.image.id.trim().is_empty()
        || !matches!(request.image.mime_type.as_str(), "image/jpeg" | "image/png")
        || request.image.data_base64.len()
            > config.max_request_bytes.saturating_mul(4).saturating_div(3) + 8
    {
        return Err(CoreError::Validation(
            "HTTP Vision Worker image metadata or encoded size is invalid".to_owned(),
        ));
    }
    let decoded = STANDARD.decode(&request.image.data_base64).map_err(|_| {
        CoreError::Validation("HTTP Vision Worker image base64 is invalid".to_owned())
    })?;
    if decoded.is_empty() || decoded.len() > config.max_request_bytes {
        return Err(CoreError::Validation(
            "HTTP Vision Worker decoded image exceeds the request limit".to_owned(),
        ));
    }
    if request.queries.len() > MAX_QUERY_COUNT {
        return Err(CoreError::Validation(
            "HTTP Vision Worker query count exceeds its bound".to_owned(),
        ));
    }
    let query_ids = request
        .queries
        .iter()
        .map(|query| query.id.trim())
        .collect::<BTreeSet<_>>();
    if query_ids.len() != request.queries.len()
        || query_ids.contains("")
        || request.queries.iter().any(|query| {
            query.text.trim().is_empty()
                || query.text.len() > MAX_QUERY_TEXT_BYTES
                || query
                    .target_label
                    .as_ref()
                    .is_some_and(|label| label.as_str().trim().is_empty())
        })
    {
        return Err(CoreError::Validation(
            "HTTP Vision Worker queries must have unique ids and bounded text".to_owned(),
        ));
    }
    if matches!(
        request.operation,
        VisionCapability::OpenVocabularyDetection | VisionCapability::PhraseGrounding
    ) && request.queries.is_empty()
    {
        return Err(CoreError::Validation(
            "open-vocabulary Worker requests require at least one query".to_owned(),
        ));
    }
    let labels = request
        .target_labels
        .iter()
        .map(LabelId::as_str)
        .collect::<BTreeSet<_>>();
    if labels.len() != request.target_labels.len() || labels.contains("") {
        return Err(CoreError::Validation(
            "HTTP Vision Worker target labels must be non-empty and unique".to_owned(),
        ));
    }
    validate_options(&request.options)
}

fn validate_options(options: &DetectionWorkerOptions) -> CoreResult<()> {
    for (name, value) in [
        ("confidence_threshold", options.confidence_threshold),
        ("iou_threshold", options.iou_threshold),
    ] {
        if value.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
            return Err(CoreError::Validation(format!(
                "HTTP Vision Worker {name} must be finite and within [0,1]"
            )));
        }
    }
    if options.max_detections == Some(0)
        || options.max_detections.is_some_and(|value| value > 10_000)
    {
        return Err(CoreError::Validation(
            "HTTP Vision Worker max_detections must be within 1..=10000".to_owned(),
        ));
    }
    if options
        .generation_mode
        .as_deref()
        .is_some_and(|mode| mode.trim().is_empty() || mode.len() > 100)
    {
        return Err(CoreError::Validation(
            "HTTP Vision Worker generation_mode is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_inference_response(
    config: &HttpVisionWorkerConfig,
    request: &DetectionWorkerInferenceRequest,
    response: &DetectionWorkerInferenceResponse,
) -> CoreResult<()> {
    validate_response_scope(config, request, response)?;
    if response.error.is_some() {
        return Err(worker_failure(
            &config.id,
            &request.request_id,
            "error_with_success_response",
        ));
    }

    if response.warnings.len() > MAX_WARNING_COUNT
        || response
            .warnings
            .iter()
            .any(|warning| warning.len() > MAX_WARNING_BYTES)
    {
        return Err(worker_failure(
            &config.id,
            &request.request_id,
            "warnings_exceed_limit",
        ));
    }
    let configured_limit = request.options.max_detections.unwrap_or(10_000) as usize;
    if response.detections.len() > configured_limit {
        return Err(worker_failure(
            &config.id,
            &request.request_id,
            "detection_count_exceeds_limit",
        ));
    }
    let mut ids = BTreeSet::new();
    for detection in &response.detections {
        if detection.detection_id.trim().is_empty()
            || detection.detection_id.len() > MAX_REQUEST_ID_BYTES
            || !ids.insert(detection.detection_id.as_str())
            || detection
                .query_id
                .as_deref()
                .is_none_or(|query| query.trim().is_empty())
                && detection
                    .model_label
                    .as_deref()
                    .is_none_or(|label| label.trim().is_empty())
        {
            return Err(worker_failure(
                &config.id,
                &request.request_id,
                "invalid_or_duplicate_detection_identity",
            ));
        }
        if let Some(query_id) = &detection.query_id
            && !request.queries.iter().any(|query| query.id == *query_id)
        {
            return Err(worker_failure(
                &config.id,
                &request.request_id,
                "undeclared_query_id",
            ));
        }
        if detection
            .target_label
            .as_ref()
            .is_some_and(|label| label.as_str().trim().is_empty())
        {
            return Err(worker_failure(
                &config.id,
                &request.request_id,
                "invalid_target_label",
            ));
        }
        if let Some(target_label) = &detection.target_label {
            let requested_directly = request.target_labels.contains(target_label);
            let requested_by_query = request.queries.iter().any(|query| {
                query.id == detection.query_id.as_deref().unwrap_or_default()
                    && query.target_label.as_ref() == Some(target_label)
            });
            if !requested_directly && !requested_by_query {
                return Err(worker_failure(
                    &config.id,
                    &request.request_id,
                    "undeclared_target_label",
                ));
            }
        }
        normalized_xyxy(detection.bbox_xyxy_normalized)?;
        let score = DetectionScore::new(detection.score, detection.score_semantics)
            .map_err(CoreError::Validation)?;
        if config
            .expected_score_semantics
            .is_some_and(|expected| score.semantics != expected)
        {
            return Err(worker_failure(
                &config.id,
                &request.request_id,
                "score_semantics_mismatch",
            ));
        }
    }
    Ok(())
}

fn validate_response_scope(
    config: &HttpVisionWorkerConfig,
    request: &DetectionWorkerInferenceRequest,
    response: &DetectionWorkerInferenceResponse,
) -> CoreResult<()> {
    if response.protocol_version != VISION_WORKER_PROTOCOL_VERSION {
        return Err(worker_failure(
            &config.id,
            &request.request_id,
            "protocol_version_mismatch",
        ));
    }
    if response.request_id != request.request_id || response.model_id != config.expected_model_id {
        return Err(worker_failure(
            &config.id,
            &request.request_id,
            "response_scope_mismatch",
        ));
    }
    Ok(())
}

fn normalized_xyxy(values: [f32; 4]) -> CoreResult<NormalizedRect> {
    if values
        .iter()
        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        || values[2] <= values[0]
        || values[3] <= values[1]
    {
        return Err(CoreError::Validation(
            "HTTP Vision Worker bbox_xyxy_normalized is invalid".to_owned(),
        ));
    }
    NormalizedRect::new(
        values[0],
        values[1],
        values[2] - values[0],
        values[3] - values[1],
    )
}

fn validate_request_id(request_id: &str) -> CoreResult<()> {
    if request_id.trim().is_empty() || request_id.len() > MAX_REQUEST_ID_BYTES {
        return Err(CoreError::Validation(
            "HTTP Vision Worker request_id is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn parse_queries(
    parameters: &std::collections::BTreeMap<String, serde_json::Value>,
) -> CoreResult<Vec<DetectionWorkerQuery>> {
    parameters.get("queries").map_or_else(
        || Ok(Vec::new()),
        |value| {
            serde_json::from_value(value.clone())
                .map_err(|error| CoreError::Validation(format!("invalid Worker queries: {error}")))
        },
    )
}

fn parse_target_labels(
    parameters: &std::collections::BTreeMap<String, serde_json::Value>,
) -> CoreResult<Vec<LabelId>> {
    let value = parameters
        .get("target_labels")
        .or_else(|| parameters.get("labels"));
    value.map_or_else(
        || Ok(Vec::new()),
        |value| {
            serde_json::from_value(value.clone()).map_err(|error| {
                CoreError::Validation(format!("invalid Worker target_labels: {error}"))
            })
        },
    )
}

fn parse_options(
    parameters: &std::collections::BTreeMap<String, serde_json::Value>,
) -> CoreResult<DetectionWorkerOptions> {
    let options = DetectionWorkerOptions {
        confidence_threshold: parameter_f32(parameters, "confidence_threshold")?,
        iou_threshold: parameter_f32(parameters, "iou_threshold")?,
        max_detections: parameters.get("max_detections").map_or(Ok(None), |value| {
            value
                .as_u64()
                .map(|value| Some(u32::try_from(value).unwrap_or(u32::MAX)))
                .ok_or_else(|| {
                    CoreError::Validation("max_detections must be an unsigned integer".to_owned())
                })
        })?,
        generation_mode: parameters
            .get("generation_mode")
            .map_or(Ok(None), |value| {
                value
                    .as_str()
                    .map(|value| Some(value.to_owned()))
                    .ok_or_else(|| {
                        CoreError::Validation("generation_mode must be a string".to_owned())
                    })
            })?,
    };
    validate_options(&options)?;
    Ok(options)
}

fn parameter_f32(
    parameters: &std::collections::BTreeMap<String, serde_json::Value>,
    name: &str,
) -> CoreResult<Option<f32>> {
    parameters.get(name).map_or(Ok(None), |value| {
        value
            .as_f64()
            .map(|value| Some(value as f32))
            .ok_or_else(|| CoreError::Validation(format!("{name} must be a number")))
    })
}

fn worker_transport_error(worker_id: &str, request_id: &str, error: &reqwest::Error) -> CoreError {
    worker_failure(
        worker_id,
        request_id,
        if error.is_timeout() {
            "timeout"
        } else if error.is_redirect() {
            "redirect_rejected"
        } else {
            "transport_error"
        },
    )
}

fn worker_failure(worker_id: &str, request_id: &str, code: &str) -> CoreError {
    CoreError::Provider(format!(
        "worker={worker_id} request={request_id} code={code}"
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use annotagent_core::{
        ArtifactValidationState, DetectionWorkerUsage, ImageId, ModelImage, RunId,
        VisionModelHealthStatus,
    };
    use axum::{
        Json, Router,
        extract::State,
        response::Redirect,
        routing::{get, post},
    };
    use serde_json::json;

    use super::*;

    fn config(base_url: String) -> HttpVisionWorkerConfig {
        HttpVisionWorkerConfig {
            id: "fixture-worker".to_owned(),
            base_url,
            expected_model_id: "fixture-model".to_owned(),
            capabilities: vec![VisionCapability::ObjectDetection],
            expected_score_semantics: Some(ScoreSemantics::RelativeConfidence),
            request_timeout: Duration::from_secs(2),
            max_request_bytes: 1_000_000,
            max_response_bytes: 100_000,
            max_retries: 0,
            allow_remote: false,
            authorization: None,
        }
    }

    fn worker_request() -> DetectionWorkerInferenceRequest {
        DetectionWorkerInferenceRequest {
            protocol_version: VISION_WORKER_PROTOCOL_VERSION,
            request_id: "request-1".to_owned(),
            operation: VisionCapability::ObjectDetection,
            model_id: "fixture-model".to_owned(),
            image: ModelImage {
                id: "image-1".to_owned(),
                mime_type: "image/png".to_owned(),
                data_base64: STANDARD.encode(b"bounded fixture"),
            },
            queries: Vec::new(),
            target_labels: vec![LabelId::from("target")],
            options: DetectionWorkerOptions {
                max_detections: Some(10),
                ..DetectionWorkerOptions::default()
            },
            timeout_ms: Some(1_000),
        }
    }

    fn worker_response(
        request: &DetectionWorkerInferenceRequest,
    ) -> DetectionWorkerInferenceResponse {
        DetectionWorkerInferenceResponse {
            protocol_version: VISION_WORKER_PROTOCOL_VERSION,
            request_id: request.request_id.clone(),
            model_id: request.model_id.clone(),
            detections: vec![annotagent_core::DetectionWorkerDetection {
                detection_id: "detection-1".to_owned(),
                query_id: None,
                model_label: Some("model-target".to_owned()),
                target_label: Some(LabelId::from("target")),
                bbox_xyxy_normalized: [0.2, 0.3, 0.5, 0.7],
                score: Some(0.87),
                score_semantics: ScoreSemantics::RelativeConfidence,
            }],
            usage: annotagent_core::DetectionWorkerUsage {
                duration_ms: Some(24),
                device: Some("fixture".to_owned()),
            },
            warnings: Vec::new(),
            error: None,
        }
    }

    async fn spawn(router: Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind Worker fixture");
        let address = listener.local_addr().expect("Worker fixture address");
        tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("serve Worker fixture");
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn versioned_health_capabilities_and_infer_form_one_detection_contract() {
        async fn health() -> Json<DetectionWorkerHealth> {
            Json(DetectionWorkerHealth {
                protocol_version: VISION_WORKER_PROTOCOL_VERSION,
                worker_id: "fixture-worker".to_owned(),
                model_id: "fixture-model".to_owned(),
                status: VisionModelHealthStatus::Healthy,
                detail: Some("ready".to_owned()),
            })
        }
        async fn capabilities() -> Json<DetectionWorkerCapabilities> {
            Json(DetectionWorkerCapabilities {
                protocol_version: VISION_WORKER_PROTOCOL_VERSION,
                worker_id: "fixture-worker".to_owned(),
                model_id: "fixture-model".to_owned(),
                capabilities: vec![VisionCapability::ObjectDetection],
                score_semantics: ScoreSemantics::RelativeConfidence,
                supports_visual_prompt: false,
                supports_batch: true,
                label_space: vec!["model-target".to_owned()],
                limits: annotagent_core::VisionModelLimits {
                    max_images: Some(1),
                    max_input_artifacts: Some(0),
                    max_request_bytes: Some(1_000_000),
                    timeout_seconds: Some(2),
                },
            })
        }
        async fn infer(
            Json(request): Json<DetectionWorkerInferenceRequest>,
        ) -> Json<DetectionWorkerInferenceResponse> {
            Json(worker_response(&request))
        }
        async fn cancel(
            Json(request): Json<DetectionWorkerCancelRequest>,
        ) -> Json<DetectionWorkerCancelResponse> {
            Json(DetectionWorkerCancelResponse {
                protocol_version: request.protocol_version,
                request_id: request.request_id,
                cancelled: true,
            })
        }
        let base_url = spawn(
            Router::new()
                .route("/health", get(health))
                .route("/v1/capabilities", get(capabilities))
                .route("/v1/infer", post(infer))
                .route("/v1/cancel", post(cancel)),
        )
        .await;
        let backend =
            HttpVisionDetectionBackend::new(config(base_url), VisionCapability::ObjectDetection)
                .expect("Detection Backend");
        assert_eq!(
            backend.health().await.expect("health").status,
            VisionModelHealthStatus::Healthy
        );
        let discovered = backend.discover_capabilities().await.expect("capabilities");
        assert!(discovered.supports_batch);
        assert!(!discovered.supports_visual_prompt);

        let worker_request = worker_request();
        let image_id = ImageId::new();
        let response = backend
            .infer_pipeline(
                PipelineInferenceRequest {
                    protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
                    request_id: worker_request.request_id,
                    run_id: RunId::new(),
                    image_id,
                    node_id: "detector".to_owned(),
                    model_id: worker_request.model_id,
                    operation: VisionCapability::ObjectDetection,
                    image: Some(worker_request.image),
                    input_artifacts: Vec::new(),
                    parameters: std::collections::BTreeMap::from([
                        ("target_labels".to_owned(), json!(["target"])),
                        ("max_detections".to_owned(), json!(10)),
                    ]),
                    timeout_ms: Some(1_000),
                },
                CancellationToken::new(),
            )
            .await
            .expect("Detection inference");
        let PipelineArtifact::DetectionSet(set) = &response.artifacts[0] else {
            panic!("DetectionSet")
        };
        assert_eq!(set.image_id, image_id);
        assert_eq!(set.validation_state, ArtifactValidationState::Unvalidated);
        assert_eq!(set.detections.len(), 1);
        assert!((set.detections[0].bbox.width() - 0.3).abs() < f32::EPSILON);
        assert!((set.detections[0].bbox.height() - 0.4).abs() < f32::EPSILON);
        assert_eq!(set.detections[0].score.value, Some(0.87));
        let raw = set.detections[0].evidence[0]
            .raw_output_ref
            .as_ref()
            .expect("controlled raw payload ref");
        assert_eq!(raw.sha256.len(), 64);
        assert!(raw.size_bytes > 0);
    }

    #[tokio::test]
    async fn valid_empty_detection_response_is_not_a_worker_failure() {
        async fn infer(
            Json(request): Json<DetectionWorkerInferenceRequest>,
        ) -> Json<DetectionWorkerInferenceResponse> {
            Json(DetectionWorkerInferenceResponse {
                detections: Vec::new(),
                ..worker_response(&request)
            })
        }
        let client = HttpVisionWorkerClient::new(config(
            spawn(Router::new().route("/v1/infer", post(infer))).await,
        ))
        .expect("client");
        let result = client
            .infer(worker_request(), CancellationToken::new())
            .await
            .expect("empty Detection response");
        assert!(result.response.detections.is_empty());
    }

    #[tokio::test]
    async fn open_vocabulary_worker_preserves_queries_optional_score_and_empty_results() {
        async fn capabilities() -> Json<DetectionWorkerCapabilities> {
            Json(DetectionWorkerCapabilities {
                protocol_version: VISION_WORKER_PROTOCOL_VERSION,
                worker_id: "grounding-worker".to_owned(),
                model_id: "grounding-model".to_owned(),
                capabilities: vec![
                    VisionCapability::OpenVocabularyDetection,
                    VisionCapability::PhraseGrounding,
                ],
                score_semantics: ScoreSemantics::NotProvided,
                supports_visual_prompt: false,
                supports_batch: false,
                label_space: Vec::new(),
                limits: annotagent_core::VisionModelLimits {
                    max_images: Some(1),
                    max_input_artifacts: Some(0),
                    max_request_bytes: Some(1_000_000),
                    timeout_seconds: Some(2),
                },
            })
        }
        async fn infer(
            Json(request): Json<DetectionWorkerInferenceRequest>,
        ) -> Json<DetectionWorkerInferenceResponse> {
            let detections = request
                .queries
                .iter()
                .filter(|query| query.text != "absent")
                .enumerate()
                .map(|(index, query)| annotagent_core::DetectionWorkerDetection {
                    detection_id: format!("detection-{index}"),
                    query_id: Some(query.id.clone()),
                    model_label: None,
                    target_label: query.target_label.clone(),
                    bbox_xyxy_normalized: [0.2, 0.3, 0.5, 0.7],
                    score: None,
                    score_semantics: ScoreSemantics::NotProvided,
                })
                .collect();
            Json(DetectionWorkerInferenceResponse {
                protocol_version: VISION_WORKER_PROTOCOL_VERSION,
                request_id: request.request_id,
                model_id: request.model_id,
                detections,
                usage: DetectionWorkerUsage::default(),
                warnings: Vec::new(),
                error: None,
            })
        }
        let base_url = spawn(
            Router::new()
                .route("/v1/capabilities", get(capabilities))
                .route("/v1/infer", post(infer)),
        )
        .await;
        let worker_config = HttpVisionWorkerConfig {
            id: "grounding-worker".to_owned(),
            base_url,
            expected_model_id: "grounding-model".to_owned(),
            capabilities: vec![
                VisionCapability::OpenVocabularyDetection,
                VisionCapability::PhraseGrounding,
            ],
            expected_score_semantics: Some(ScoreSemantics::NotProvided),
            request_timeout: Duration::from_secs(2),
            max_request_bytes: 1_000_000,
            max_response_bytes: 100_000,
            max_retries: 0,
            allow_remote: false,
            authorization: None,
        };
        let backend = HttpVisionDetectionBackend::new(
            worker_config.clone(),
            VisionCapability::OpenVocabularyDetection,
        )
        .expect("Grounding Backend");
        let discovered = backend.discover_capabilities().await.expect("discovery");
        assert!(!discovered.supports_visual_prompt);
        assert_eq!(discovered.score_semantics, ScoreSemantics::NotProvided);

        for (text, expected_count) in [("a football", 1), ("absent", 0)] {
            let image_id = ImageId::new();
            let response = backend
                .infer_pipeline(
                    PipelineInferenceRequest {
                        protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
                        request_id: uuid::Uuid::new_v4().to_string(),
                        run_id: RunId::new(),
                        image_id,
                        node_id: "grounding".to_owned(),
                        model_id: "grounding-model".to_owned(),
                        operation: VisionCapability::OpenVocabularyDetection,
                        image: Some(ModelImage {
                            id: "image".to_owned(),
                            mime_type: "image/png".to_owned(),
                            data_base64: STANDARD.encode(b"png"),
                        }),
                        input_artifacts: Vec::new(),
                        parameters: std::collections::BTreeMap::from([(
                            "queries".to_owned(),
                            json!([{"id":"football","text":text,"target_label":"football"}]),
                        )]),
                        timeout_ms: Some(1_000),
                    },
                    CancellationToken::new(),
                )
                .await
                .expect("Grounding inference");
            let PipelineArtifact::DetectionSet(set) = &response.artifacts[0] else {
                panic!("DetectionSet")
            };
            assert_eq!(set.detections.len(), expected_count);
            if let Some(detection) = set.detections.first() {
                assert_eq!(detection.query_id.as_deref(), Some("football"));
                assert_eq!(detection.score, DetectionScore::not_provided());
                assert!((detection.bbox.width() - 0.3).abs() < f32::EPSILON);
                assert!((detection.bbox.height() - 0.4).abs() < f32::EPSILON);
            }
        }

        let registry = HttpVisionWorkerRegistryBackend::new(worker_config)
            .expect("registry-facing Grounding Backend");
        assert_eq!(registry.kind(), VisionBackendKind::HttpVision);
        assert_eq!(registry.capabilities().len(), 2);
    }

    #[test]
    fn endpoint_policy_is_loopback_by_default_and_remote_is_explicit_https_only() {
        for endpoint in [
            "file:///tmp/worker.sock",
            "http://worker.example/v1",
            "https://user:secret@worker.example/v1",
            "http://127.0.0.1:8790?v=1",
        ] {
            assert!(
                HttpVisionWorkerClient::new(config(endpoint.to_owned())).is_err(),
                "endpoint {endpoint:?} must be rejected"
            );
        }
        let mut remote = config("https://worker.example/service".to_owned());
        remote.allow_remote = true;
        assert!(HttpVisionWorkerClient::new(remote).is_ok());
        let mut insecure_remote = config("http://worker.example/service".to_owned());
        insecure_remote.allow_remote = true;
        assert!(HttpVisionWorkerClient::new(insecure_remote).is_err());
    }

    #[test]
    fn response_validation_rejects_spoofed_scope_geometry_ids_and_scores() {
        let config = config("http://127.0.0.1:8790".to_owned());
        let request = worker_request();
        let baseline = worker_response(&request);
        let mut cases = Vec::new();
        let mut wrong_version = baseline.clone();
        wrong_version.protocol_version += 1;
        cases.push(wrong_version);
        let mut wrong_model = baseline.clone();
        wrong_model.model_id = "spoofed-model".to_owned();
        cases.push(wrong_model);
        let mut duplicate = baseline.clone();
        duplicate.detections.push(duplicate.detections[0].clone());
        cases.push(duplicate);
        let mut out_of_bounds = baseline.clone();
        out_of_bounds.detections[0].bbox_xyxy_normalized = [0.1, 0.1, 1.1, 0.2];
        cases.push(out_of_bounds);
        let mut reversed = baseline.clone();
        reversed.detections[0].bbox_xyxy_normalized = [0.7, 0.1, 0.2, 0.3];
        cases.push(reversed);
        let mut fake_score = baseline.clone();
        fake_score.detections[0].score = None;
        fake_score.detections[0].score_semantics = ScoreSemantics::RelativeConfidence;
        cases.push(fake_score);
        let mut undeclared_label = baseline.clone();
        undeclared_label.detections[0].target_label = Some(LabelId::from("undeclared"));
        cases.push(undeclared_label);
        for response in cases {
            assert!(validate_inference_response(&config, &request, &response).is_err());
        }
    }

    #[test]
    fn capability_discovery_rejects_worker_identity_and_capability_spoofing() {
        let config = config("http://127.0.0.1:8790".to_owned());
        let baseline = DetectionWorkerCapabilities {
            protocol_version: VISION_WORKER_PROTOCOL_VERSION,
            worker_id: config.id.clone(),
            model_id: config.expected_model_id.clone(),
            capabilities: vec![VisionCapability::ObjectDetection],
            score_semantics: ScoreSemantics::RelativeConfidence,
            supports_visual_prompt: false,
            supports_batch: false,
            label_space: vec!["target".to_owned()],
            limits: annotagent_core::VisionModelLimits::default(),
        };
        validate_capabilities(&config, &baseline).expect("baseline capabilities");
        let mut wrong_worker = baseline.clone();
        wrong_worker.worker_id = "spoofed-worker".to_owned();
        assert!(validate_capabilities(&config, &wrong_worker).is_err());
        let mut missing_capability = baseline.clone();
        missing_capability.capabilities = vec![VisionCapability::OpenVocabularyDetection];
        assert!(validate_capabilities(&config, &missing_capability).is_err());
        let mut wrong_semantics = baseline;
        wrong_semantics.score_semantics = ScoreSemantics::NotProvided;
        assert!(validate_capabilities(&config, &wrong_semantics).is_err());
    }

    #[tokio::test]
    async fn unreachable_worker_does_not_block_client_construction() {
        let mut worker_config = config("http://127.0.0.1:1".to_owned());
        worker_config.request_timeout = Duration::from_millis(50);
        let client = HttpVisionWorkerClient::new(worker_config)
            .expect("an offline Worker remains a configured client");
        assert!(client.health().await.is_err());
    }

    #[tokio::test]
    async fn malformed_unknown_fields_and_oversized_responses_are_rejected() {
        async fn malicious(
            Json(request): Json<DetectionWorkerInferenceRequest>,
        ) -> Json<serde_json::Value> {
            Json(json!({
                "protocol_version": VISION_WORKER_PROTOCOL_VERSION,
                "request_id": request.request_id,
                "model_id": request.model_id,
                "detections": [{
                    "detection_id": "detection-1",
                    "query_id": null,
                    "model_label": "target",
                    "target_label": "target",
                    "bbox_xyxy_normalized": [0.1, 0.1, 0.2, 0.2],
                    "score": 0.9,
                    "score_semantics": "relative_confidence",
                    "local_path": "/private/worker-output.png"
                }],
                "usage": {},
                "warnings": [],
                "error": null
            }))
        }
        async fn oversized() -> Vec<u8> {
            vec![b'x'; 1_024]
        }
        let client = HttpVisionWorkerClient::new(config(
            spawn(Router::new().route("/v1/infer", post(malicious))).await,
        ))
        .expect("client");
        let error = client
            .infer(worker_request(), CancellationToken::new())
            .await
            .expect_err("unknown path field must fail");
        assert!(error.to_string().contains("malformed_response"));

        let mut small = config(spawn(Router::new().route("/v1/infer", post(oversized))).await);
        small.max_response_bytes = 128;
        let client = HttpVisionWorkerClient::new(small).expect("small response client");
        let error = client
            .infer(worker_request(), CancellationToken::new())
            .await
            .expect_err("oversized response");
        assert!(error.to_string().contains("exceeds 128 bytes"));
    }

    #[tokio::test]
    async fn timeout_and_runtime_cancellation_are_distinct_and_cancel_is_forwarded() {
        async fn slow_infer(
            Json(request): Json<DetectionWorkerInferenceRequest>,
        ) -> Json<DetectionWorkerInferenceResponse> {
            tokio::time::sleep(Duration::from_millis(250)).await;
            Json(worker_response(&request))
        }
        async fn cancel(
            State(count): State<Arc<AtomicUsize>>,
            Json(request): Json<DetectionWorkerCancelRequest>,
        ) -> Json<DetectionWorkerCancelResponse> {
            count.fetch_add(1, Ordering::SeqCst);
            Json(DetectionWorkerCancelResponse {
                protocol_version: VISION_WORKER_PROTOCOL_VERSION,
                request_id: request.request_id,
                cancelled: true,
            })
        }
        let cancel_count = Arc::new(AtomicUsize::new(0));
        let base_url = spawn(
            Router::new()
                .route("/v1/infer", post(slow_infer))
                .route("/v1/cancel", post(cancel))
                .with_state(cancel_count.clone()),
        )
        .await;

        let mut timeout_config = config(base_url.clone());
        timeout_config.request_timeout = Duration::from_millis(30);
        let timeout_client = HttpVisionWorkerClient::new(timeout_config).expect("timeout client");
        let mut timed_request = worker_request();
        timed_request.timeout_ms = Some(30);
        let error = timeout_client
            .infer(timed_request, CancellationToken::new())
            .await
            .expect_err("timeout");
        assert!(error.to_string().contains("code=timeout"));

        let client = HttpVisionWorkerClient::new(config(base_url)).expect("cancel client");
        let cancellation = CancellationToken::new();
        let trigger = cancellation.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            trigger.cancel();
        });
        let error = client
            .infer(worker_request(), cancellation)
            .await
            .expect_err("cancelled");
        assert!(error.to_string().contains("code=cancelled"));
        assert_eq!(cancel_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn redirects_are_not_followed_and_authorization_cannot_leak_to_target() {
        async fn target(State(hits): State<Arc<AtomicUsize>>) -> Json<serde_json::Value> {
            hits.fetch_add(1, Ordering::SeqCst);
            Json(json!({"unexpected": true}))
        }
        async fn redirect(State(target_url): State<String>) -> Redirect {
            Redirect::temporary(&format!("{target_url}/capture"))
        }
        let hits = Arc::new(AtomicUsize::new(0));
        let target_url = spawn(
            Router::new()
                .route("/capture", post(target))
                .with_state(hits.clone()),
        )
        .await;
        let redirect_url = spawn(
            Router::new()
                .route("/v1/infer", post(redirect))
                .with_state(target_url),
        )
        .await;
        let mut worker_config = config(redirect_url);
        worker_config.authorization = Some("Bearer fixture-secret".to_owned());
        let client = HttpVisionWorkerClient::new(worker_config).expect("redirect client");
        client
            .infer(worker_request(), CancellationToken::new())
            .await
            .expect_err("redirect must fail");
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}
