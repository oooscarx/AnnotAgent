//! Domain-neutral registries and contracts for hybrid vision workflow nodes.

use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::{CoreError, CoreResult, ImageId, LabelId, ModelImage, RunId, TaskId, VisionArtifact};

pub const VISION_WORKER_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisionCapability {
    VisionLanguage,
    OpenVocabularyDetection,
    PhraseGrounding,
    ObjectDetection,
    SemanticSegmentation,
    InstanceSegmentation,
    PromptedSegmentation,
    Classification,
    KeypointDetection,
    Embedding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisionBackendKind {
    OpenAiCompatible,
    #[serde(alias = "http_json")]
    HttpVision,
    Onnx,
    DeterministicCv,
    Mock,
}

/// Public architecture name used by Model Registry APIs and documentation.
pub type BackendKind = VisionBackendKind;
/// Model IDs are opaque Registry identities, never product or scheduling concepts.
pub type ModelId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScoreSemantics {
    SemanticConfidence,
    DetectionConfidence,
    CalibratedProbability,
    RelativeConfidence,
    RankingScore,
    NotProvided,
    #[default]
    Unknown,
}

impl ScoreSemantics {
    #[must_use]
    pub const fn is_semantic(self) -> bool {
        matches!(self, Self::SemanticConfidence)
    }

    #[must_use]
    pub const fn is_detection_score(self) -> bool {
        matches!(self, Self::DetectionConfidence)
    }

    #[must_use]
    pub const fn is_calibrated_probability(self) -> bool {
        matches!(self, Self::CalibratedProbability)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelVersionMetadata {
    pub architecture: Option<String>,
    pub model_version: String,
    pub checkpoint_sha256: Option<String>,
    pub training_dataset_version: Option<String>,
    pub backend_protocol_version: String,
}

impl Default for ModelVersionMetadata {
    fn default() -> Self {
        Self {
            architecture: None,
            model_version: "unversioned".to_owned(),
            checkpoint_sha256: None,
            training_dataset_version: None,
            backend_protocol_version: VISION_WORKER_PROTOCOL_VERSION.to_string(),
        }
    }
}

/// Backend metadata is frozen with the model while the executable adapter remains Registry-owned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BackendDescriptor {
    /// `None` is accepted only while migrating an older descriptor and is filled by registration.
    pub kind: Option<VisionBackendKind>,
    pub protocol_version: Option<String>,
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModelInputContract {
    #[serde(default)]
    pub input_types: Vec<VisionInputType>,
    #[serde(default)]
    pub supports_multiple_queries: bool,
    /// Whether a model accepts an exemplar image/box in addition to text queries.
    #[serde(default)]
    pub supports_visual_prompt: bool,
    pub max_queries: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModelOutputContract {
    #[serde(default)]
    pub output_types: Vec<ArtifactKind>,
    #[serde(default)]
    pub normalized_coordinates: bool,
    #[serde(default)]
    pub allows_empty: bool,
    #[serde(default)]
    pub label_space: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeRequirements {
    #[serde(default)]
    pub devices: Vec<String>,
    pub minimum_gpu_memory_mb: Option<u64>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub supports_batch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LicensePermission {
    Allowed,
    Restricted,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LicenseMetadata {
    pub code_license: Option<String>,
    pub weight_license: Option<String>,
    pub source_url: Option<String>,
    #[serde(default)]
    pub commercial_use: LicensePermission,
    #[serde(default)]
    pub redistribution: LicensePermission,
    #[serde(default)]
    pub usage_notes: Vec<String>,
    #[serde(default)]
    pub verified_from_official_source: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelAvailabilityStatus {
    Available,
    Unreachable,
    Misconfigured,
    IncompatibleProtocol,
    MissingWeights,
    Disabled,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Image,
    DetectionSet,
    BoxPromptSet,
    PointPromptSet,
    MaskSet,
    PolygonSet,
    CandidateClusterSet,
    CropSet,
    ClassificationSet,
    AnnotationCandidateSet,
    Classification,
    BoundingBox,
    Keypoints,
    Polyline,
    Polygon,
    SemanticMask,
    InstanceMask,
    Attributes,
    Relations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisionInputType {
    Image,
    Text,
    Artifact(ArtifactKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VisionModelHealthStatus {
    Healthy,
    Degraded,
    Unavailable,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VisionModelHealth {
    pub status: VisionModelHealthStatus,
    pub detail: Option<String>,
    pub checked_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VisionModelLimits {
    pub max_images: Option<u32>,
    pub max_input_artifacts: Option<u32>,
    pub max_request_bytes: Option<u64>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VisionModelPricing {
    pub per_request: Option<rust_decimal::Decimal>,
    pub per_input_megapixel: Option<rust_decimal::Decimal>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VisionModelDescriptor {
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    pub backend_id: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub backend: BackendDescriptor,
    pub capabilities: Vec<VisionCapability>,
    #[serde(default)]
    pub input_types: Vec<VisionInputType>,
    #[serde(default)]
    pub output_types: Vec<ArtifactKind>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub model_version: String,
    #[serde(default)]
    pub version: ModelVersionMetadata,
    pub endpoint_or_path: Option<String>,
    #[serde(default)]
    pub input_contract: ModelInputContract,
    #[serde(default)]
    pub output_contract: ModelOutputContract,
    #[serde(default)]
    pub score_semantics: ScoreSemantics,
    #[serde(default)]
    pub runtime_requirements: RuntimeRequirements,
    #[serde(default)]
    pub license: LicenseMetadata,
    #[serde(default)]
    pub status: ModelAvailabilityStatus,
    #[serde(default)]
    pub pricing: VisionModelPricing,
    #[serde(default)]
    pub health: VisionModelHealth,
    #[serde(default)]
    pub limits: VisionModelLimits,
    pub secret_reference: Option<String>,
    #[serde(default)]
    pub configuration: BTreeMap<String, serde_json::Value>,
}

/// Preferred domain-neutral name. `VisionModelDescriptor` remains source-compatible for existing
/// callers and serialized Workflow snapshots.
pub type ModelDescriptor = VisionModelDescriptor;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisionNodeDescriptor {
    pub id: String,
    pub display_name: String,
    pub required_capabilities: Vec<VisionCapability>,
    #[serde(default)]
    pub accepts: Vec<ArtifactKind>,
    pub produces: Vec<ArtifactKind>,
    #[serde(default)]
    pub deterministic: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisionInferenceRequest {
    #[serde(default = "default_protocol_version")]
    pub protocol_version: u32,
    pub request_id: String,
    pub operation: VisionCapability,
    pub run_id: RunId,
    pub image_id: ImageId,
    pub task_id: TaskId,
    pub node_id: String,
    pub model_id: String,
    pub image: Option<ModelImage>,
    #[serde(default)]
    pub input_artifacts: Vec<VisionArtifact>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub cancellation_requested: bool,
}

const fn default_protocol_version() -> u32 {
    VISION_WORKER_PROTOCOL_VERSION
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VisionBackendUsage {
    pub source: Option<String>,
    pub compute_milliseconds: Option<u64>,
    pub input_megapixels: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VisionBackendTimings {
    pub queue_ms: Option<u64>,
    pub preprocess_ms: Option<u64>,
    pub inference_ms: Option<u64>,
    pub postprocess_ms: Option<u64>,
    pub total_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisionBackendError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisionInferenceResponse {
    #[serde(default = "default_protocol_version")]
    pub protocol_version: u32,
    pub model_identity: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<VisionArtifact>,
    pub request_id: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub usage: VisionBackendUsage,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub timings: VisionBackendTimings,
    pub error: Option<VisionBackendError>,
}

impl Default for VisionInferenceResponse {
    fn default() -> Self {
        Self {
            protocol_version: VISION_WORKER_PROTOCOL_VERSION,
            model_identity: None,
            artifacts: Vec::new(),
            request_id: None,
            metadata: BTreeMap::new(),
            usage: VisionBackendUsage::default(),
            warnings: Vec::new(),
            timings: VisionBackendTimings::default(),
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisionWorkerCapabilities {
    pub protocol_version: u32,
    pub worker_id: String,
    pub model_identity: String,
    pub capabilities: Vec<VisionCapability>,
    pub input_types: Vec<VisionInputType>,
    pub output_types: Vec<ArtifactKind>,
    #[serde(default)]
    pub limits: VisionModelLimits,
    /// Multi-model discovery extension. Older single-model Workers may omit this field.
    #[serde(default)]
    pub models: Vec<VisionWorkerModelSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisionWorkerModelSummary {
    pub model_id: String,
    pub display_name: String,
    pub architecture: Option<String>,
    pub model_version: String,
    pub checkpoint_sha256: Option<String>,
    pub capabilities: Vec<VisionCapability>,
    pub availability: crate::ModelAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisionWorkerModelsResponse {
    pub protocol_version: u32,
    pub worker_id: String,
    pub models: Vec<VisionWorkerModelSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisionWorkerContractsResponse {
    pub protocol_version: u32,
    pub worker_id: String,
    pub models: Vec<crate::ExpertModelManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisionWorkerWarmupRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisionWorkerWarmupResponse {
    pub protocol_version: u32,
    pub request_id: String,
    pub model_id: String,
    pub ready: bool,
    pub duration_ms: Option<u64>,
    pub error: Option<VisionBackendError>,
}

/// Health response for the versioned detection-worker protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionWorkerHealth {
    pub protocol_version: u32,
    pub worker_id: String,
    pub model_id: ModelId,
    pub status: VisionModelHealthStatus,
    pub detail: Option<String>,
}

/// Runtime-discovered facts reported by a detection Worker, never inferred by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionWorkerCapabilities {
    pub protocol_version: u32,
    pub worker_id: String,
    pub model_id: ModelId,
    pub capabilities: Vec<VisionCapability>,
    pub score_semantics: ScoreSemantics,
    #[serde(default)]
    pub supports_visual_prompt: bool,
    #[serde(default)]
    pub supports_batch: bool,
    #[serde(default)]
    pub label_space: Vec<String>,
    #[serde(default)]
    pub limits: VisionModelLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionWorkerQuery {
    pub id: String,
    pub text: String,
    pub target_label: Option<LabelId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DetectionWorkerOptions {
    pub confidence_threshold: Option<f32>,
    pub iou_threshold: Option<f32>,
    pub max_detections: Option<u32>,
    pub generation_mode: Option<String>,
}

/// The only image-bearing request accepted by detection Workers. It has no filesystem path field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionWorkerInferenceRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub operation: VisionCapability,
    pub model_id: ModelId,
    pub image: ModelImage,
    #[serde(default)]
    pub queries: Vec<DetectionWorkerQuery>,
    #[serde(default)]
    pub target_labels: Vec<LabelId>,
    #[serde(default)]
    pub options: DetectionWorkerOptions,
    pub timeout_ms: Option<u64>,
}

/// Worker-native Detection before the Provider adapter converts xyxy into Core xywh geometry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionWorkerDetection {
    pub detection_id: String,
    pub query_id: Option<String>,
    pub model_label: Option<String>,
    pub target_label: Option<LabelId>,
    pub bbox_xyxy_normalized: [f32; 4],
    pub score: Option<f32>,
    pub score_semantics: ScoreSemantics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DetectionWorkerUsage {
    pub duration_ms: Option<u64>,
    pub device: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionWorkerInferenceResponse {
    pub protocol_version: u32,
    pub request_id: String,
    pub model_id: ModelId,
    #[serde(default)]
    pub detections: Vec<DetectionWorkerDetection>,
    #[serde(default)]
    pub usage: DetectionWorkerUsage,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub error: Option<VisionBackendError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionWorkerCancelRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub model_id: ModelId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionWorkerCancelResponse {
    pub protocol_version: u32,
    pub request_id: String,
    pub cancelled: bool,
}

#[async_trait]
pub trait VisionModelBackend: Send + Sync {
    fn id(&self) -> &str;
    fn kind(&self) -> VisionBackendKind;
    fn capabilities(&self) -> Vec<VisionCapability>;

    async fn infer(
        &self,
        request: VisionInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<VisionInferenceResponse>;
}

#[derive(Default)]
pub struct ModelRegistry {
    models: BTreeMap<String, VisionModelDescriptor>,
    expert_manifests: BTreeMap<String, crate::ExpertModelManifest>,
    backends: BTreeMap<String, Arc<dyn VisionModelBackend>>,
}

impl ModelRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_backend(&mut self, backend: Arc<dyn VisionModelBackend>) -> CoreResult<()> {
        let id = backend.id().to_owned();
        if self.backends.insert(id.clone(), backend).is_some() {
            return Err(CoreError::Validation(format!(
                "vision backend {id:?} is already registered"
            )));
        }
        Ok(())
    }

    pub fn register_model(&mut self, mut model: VisionModelDescriptor) -> CoreResult<()> {
        if self.models.contains_key(&model.id) {
            return Err(CoreError::Validation(
                "vision model id is already registered".to_owned(),
            ));
        }
        if model.id.trim().is_empty() || model.backend_id.trim().is_empty() {
            return Err(CoreError::Validation(
                "model id and backend_id cannot be empty".to_owned(),
            ));
        }
        let backend = self.backends.get(&model.backend_id).ok_or_else(|| {
            CoreError::Validation(format!(
                "model {:?} references unknown backend {:?}",
                model.id, model.backend_id
            ))
        })?;
        if model.capabilities.is_empty() {
            return Err(CoreError::Validation(
                "model capabilities cannot be empty".to_owned(),
            ));
        }
        let unique_capabilities = model
            .capabilities
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if unique_capabilities.len() != model.capabilities.len() {
            return Err(CoreError::Validation(
                "model capabilities must be unique".to_owned(),
            ));
        }
        if model.display_name.trim().is_empty() {
            model.display_name.clone_from(&model.id);
        }
        if model.provider.trim().is_empty() {
            model.provider.clone_from(&model.backend_id);
        }
        if model.model.trim().is_empty() {
            model.model.clone_from(&model.id);
        }
        if model.model_version.trim().is_empty() {
            "unversioned".clone_into(&mut model.model_version);
        }
        if model.input_types.is_empty() {
            if model.input_contract.input_types.is_empty() {
                model.input_types.push(VisionInputType::Image);
            } else {
                model
                    .input_types
                    .clone_from(&model.input_contract.input_types);
            }
        }
        if model.input_contract.input_types.is_empty() {
            model
                .input_contract
                .input_types
                .clone_from(&model.input_types);
        } else if model.input_types != model.input_contract.input_types {
            return Err(CoreError::Validation(
                "legacy input_types and input_contract must agree".to_owned(),
            ));
        }
        if model.output_types.is_empty() {
            if model.output_contract.output_types.is_empty() {
                model.output_types = model
                    .capabilities
                    .iter()
                    .copied()
                    .filter_map(capability_output_type)
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
            } else {
                model
                    .output_types
                    .clone_from(&model.output_contract.output_types);
            }
        }
        if model.output_contract.output_types.is_empty() {
            model
                .output_contract
                .output_types
                .clone_from(&model.output_types);
        } else if model.output_types != model.output_contract.output_types {
            return Err(CoreError::Validation(
                "legacy output_types and output_contract must agree".to_owned(),
            ));
        }
        normalize_model_version(&mut model)?;
        let backend_kind = backend.kind();
        match model.backend.kind {
            Some(kind) if kind != backend_kind => {
                return Err(CoreError::Validation(format!(
                    "model backend kind {kind:?} does not match registered backend {backend_kind:?}"
                )));
            }
            Some(_) => {}
            None => model.backend.kind = Some(backend_kind),
        }
        if model.backend.protocol_version.is_none() {
            model.backend.protocol_version = Some(model.version.backend_protocol_version.clone());
        }
        if model.backend.endpoint.is_none() {
            model.backend.endpoint.clone_from(&model.endpoint_or_path);
        }
        validate_model_contract(&model)?;
        if model.health.status == VisionModelHealthStatus::Unknown
            && matches!(
                backend.kind(),
                VisionBackendKind::Mock | VisionBackendKind::DeterministicCv
            )
        {
            model.health = VisionModelHealth {
                status: VisionModelHealthStatus::Healthy,
                detail: Some("in-process backend registered".to_owned()),
                checked_at: Some(chrono::Utc::now()),
            };
        }
        if model.status == ModelAvailabilityStatus::Unknown {
            model.status = match model.health.status {
                VisionModelHealthStatus::Healthy => ModelAvailabilityStatus::Available,
                VisionModelHealthStatus::Unavailable => ModelAvailabilityStatus::Unreachable,
                VisionModelHealthStatus::Degraded | VisionModelHealthStatus::Unknown => {
                    ModelAvailabilityStatus::Unknown
                }
            };
        }
        if let Some(secret_reference) = &model.secret_reference
            && !secret_reference.starts_with("env:")
            && !secret_reference.starts_with("keychain:")
        {
            return Err(CoreError::Validation(
                "secret_reference must be an env: or keychain: reference, never secret material"
                    .to_owned(),
            ));
        }
        if let Some(path) = secret_configuration_path(&model.configuration) {
            return Err(CoreError::Validation(format!(
                "model configuration {path:?} may contain secret material; use secret_reference"
            )));
        }
        let supported = backend.capabilities();
        if let Some(capability) = model
            .capabilities
            .iter()
            .find(|capability| !supported.contains(capability))
        {
            return Err(CoreError::Validation(format!(
                "backend {:?} does not support {capability:?}",
                model.backend_id
            )));
        }
        let has_expert_capability = model.capabilities.iter().any(|capability| {
            !matches!(
                capability,
                VisionCapability::VisionLanguage | VisionCapability::Embedding
            )
        });
        let manifest =
            if backend_kind == VisionBackendKind::OpenAiCompatible || !has_expert_capability {
                None
            } else {
                Some(crate::ExpertModelManifest::from_vision_descriptor(&model)?)
            };
        if let Some(manifest) = manifest {
            self.expert_manifests
                .insert(manifest.model_id.clone(), manifest);
        }
        self.models.insert(model.id.clone(), model);
        Ok(())
    }

    /// Registers a Worker-backed or Mock model entirely from a capability manifest. The
    /// executable backend is still supplied separately by the Worker Registry.
    pub fn register_expert_manifest(
        &mut self,
        manifest: crate::ExpertModelManifest,
    ) -> CoreResult<()> {
        manifest.validate()?;
        if self.models.contains_key(&manifest.model_id) {
            return Err(CoreError::Validation(
                "vision model id is already registered".to_owned(),
            ));
        }
        let backend_id = match &manifest.connection {
            crate::ModelConnection::VisionWorkerModel { worker_id, .. } => worker_id.clone(),
            crate::ModelConnection::Mock { fixture_id } => fixture_id.clone(),
            crate::ModelConnection::ProviderModel { .. } => {
                return Err(CoreError::Validation(
                    "Provider-backed models must be registered through Model Profiles".to_owned(),
                ));
            }
        };
        let backend_kind = self
            .backends
            .get(&backend_id)
            .ok_or_else(|| {
                CoreError::Validation(format!(
                    "Expert Model {:?} references unknown backend {backend_id:?}",
                    manifest.model_id
                ))
            })?
            .kind();
        let input_types = manifest
            .input_contracts
            .iter()
            .map(|contract| match contract.data_type {
                crate::ContractDataType::Text => VisionInputType::Text,
                crate::ContractDataType::Artifact(ArtifactKind::Image) => VisionInputType::Image,
                crate::ContractDataType::Artifact(kind) => VisionInputType::Artifact(kind),
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let output_types = manifest
            .output_contracts
            .iter()
            .filter_map(|contract| match contract.data_type {
                crate::ContractDataType::Text => None,
                crate::ContractDataType::Artifact(kind) => Some(kind),
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let capabilities = manifest
            .capabilities
            .iter()
            .copied()
            .map(crate::vision_capability)
            .collect::<Vec<_>>();
        let checkpoint_sha256 = manifest
            .checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.sha256.clone());
        let training_dataset_version = manifest
            .checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.training_dataset_version.clone());
        let health = VisionModelHealth {
            status: if manifest.availability_evidence.health_passed {
                VisionModelHealthStatus::Healthy
            } else if manifest.availability == crate::ModelAvailability::Unreachable {
                VisionModelHealthStatus::Unavailable
            } else {
                VisionModelHealthStatus::Unknown
            },
            detail: manifest.availability_evidence.detail.clone(),
            checked_at: manifest.availability_evidence.checked_at,
        };
        let descriptor = VisionModelDescriptor {
            id: manifest.model_id.clone(),
            display_name: manifest.display_name.clone(),
            backend_id,
            provider: match backend_kind {
                VisionBackendKind::Mock => "mock",
                VisionBackendKind::HttpVision => "vision_worker",
                VisionBackendKind::Onnx => "onnx_worker",
                VisionBackendKind::DeterministicCv => "deterministic_worker",
                VisionBackendKind::OpenAiCompatible => "provider",
            }
            .to_owned(),
            backend: BackendDescriptor {
                kind: Some(backend_kind),
                protocol_version: Some(manifest.schema_version.clone()),
                endpoint: None,
            },
            capabilities,
            input_types: input_types.clone(),
            output_types: output_types.clone(),
            model: manifest.model_id.clone(),
            model_version: manifest.model_version.clone(),
            version: ModelVersionMetadata {
                architecture: manifest.architecture.clone(),
                model_version: manifest.model_version.clone(),
                checkpoint_sha256,
                training_dataset_version,
                backend_protocol_version: manifest.schema_version.clone(),
            },
            input_contract: ModelInputContract {
                input_types,
                supports_multiple_queries: manifest
                    .prompt_contracts
                    .iter()
                    .any(|contract| contract.multiple),
                supports_visual_prompt: manifest.prompt_contracts.iter().any(|contract| {
                    matches!(
                        contract.kind,
                        crate::PromptKind::Box
                            | crate::PromptKind::Point
                            | crate::PromptKind::ExistingAnnotation
                    )
                }),
                max_queries: None,
            },
            output_contract: ModelOutputContract {
                output_types,
                normalized_coordinates: manifest.geometry_semantics
                    != crate::GeometrySemantics::NotApplicable,
                allows_empty: true,
                label_space: manifest.label_space.clone().unwrap_or_default(),
            },
            score_semantics: manifest.score_semantics,
            runtime_requirements: manifest.runtime_requirements.clone(),
            license: manifest.license.clone(),
            status: manifest.availability.legacy_status(),
            health,
            configuration: manifest.metadata.clone(),
            ..VisionModelDescriptor::default()
        };
        self.register_model(descriptor)?;
        self.expert_manifests
            .insert(manifest.model_id.clone(), manifest);
        Ok(())
    }

    pub fn set_health(&mut self, model_id: &str, health: VisionModelHealth) -> CoreResult<()> {
        let model = self
            .models
            .get_mut(model_id)
            .ok_or_else(|| CoreError::Validation(format!("unknown vision model {model_id:?}")))?;
        model.health = health;
        Ok(())
    }

    #[must_use]
    pub fn models(&self) -> Vec<VisionModelDescriptor> {
        self.models.values().cloned().collect()
    }

    #[must_use]
    pub fn expert_manifests(&self) -> Vec<crate::ExpertModelManifest> {
        self.expert_manifests.values().cloned().collect()
    }

    #[must_use]
    pub fn expert_manifest(&self, model_id: &str) -> Option<&crate::ExpertModelManifest> {
        self.expert_manifests.get(model_id)
    }

    pub fn resolve(
        &self,
        model_id: &str,
    ) -> CoreResult<(&VisionModelDescriptor, Arc<dyn VisionModelBackend>)> {
        let model = self
            .models
            .get(model_id)
            .ok_or_else(|| CoreError::Validation(format!("unknown vision model {model_id:?}")))?;
        if matches!(
            model.status,
            ModelAvailabilityStatus::Disabled
                | ModelAvailabilityStatus::Misconfigured
                | ModelAvailabilityStatus::IncompatibleProtocol
                | ModelAvailabilityStatus::MissingWeights
        ) {
            return Err(CoreError::Validation(format!(
                "vision model {model_id:?} is not executable: {:?}",
                model.status
            )));
        }
        let backend = self.backends.get(&model.backend_id).ok_or_else(|| {
            CoreError::Validation(format!("unknown vision backend {:?}", model.backend_id))
        })?;
        Ok((model, backend.clone()))
    }
}

fn normalize_model_version(model: &mut VisionModelDescriptor) -> CoreResult<()> {
    let legacy = model.model_version.trim();
    let structured = model.version.model_version.trim();
    if structured.is_empty() || structured == "unversioned" {
        model.version.model_version = if legacy.is_empty() {
            "unversioned".to_owned()
        } else {
            legacy.to_owned()
        };
    } else if !legacy.is_empty() && legacy != "unversioned" && legacy != structured {
        return Err(CoreError::Validation(
            "legacy model_version and version.model_version must agree".to_owned(),
        ));
    }
    model.model_version.clone_from(&model.version.model_version);
    if model.version.backend_protocol_version.trim().is_empty() {
        return Err(CoreError::Validation(
            "backend protocol version cannot be empty".to_owned(),
        ));
    }
    if let Some(checkpoint) = &model.version.checkpoint_sha256
        && (checkpoint.len() != 64 || !checkpoint.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(CoreError::Validation(
            "checkpoint_sha256 must contain exactly 64 hexadecimal characters".to_owned(),
        ));
    }
    Ok(())
}

fn validate_model_contract(model: &VisionModelDescriptor) -> CoreResult<()> {
    if model.input_contract.max_queries == Some(0) {
        return Err(CoreError::Validation(
            "model max_queries must be greater than zero".to_owned(),
        ));
    }
    if model.runtime_requirements.minimum_gpu_memory_mb == Some(0) {
        return Err(CoreError::Validation(
            "minimum_gpu_memory_mb must be greater than zero".to_owned(),
        ));
    }
    validate_unique_nonempty(&model.output_contract.label_space, "model label_space")?;
    validate_unique_nonempty(&model.runtime_requirements.devices, "runtime devices")?;
    validate_unique_nonempty(
        &model.runtime_requirements.dependencies,
        "runtime dependencies",
    )?;
    validate_unique_nonempty(&model.license.usage_notes, "license usage_notes")?;
    if let Some(source_url) = &model.license.source_url
        && !(source_url.starts_with("https://") || source_url.starts_with("http://"))
    {
        return Err(CoreError::Validation(
            "license source_url must be an http(s) URL".to_owned(),
        ));
    }
    if model.license.verified_from_official_source && model.license.source_url.is_none() {
        return Err(CoreError::Validation(
            "verified license metadata requires an official source_url".to_owned(),
        ));
    }
    if model
        .capabilities
        .iter()
        .any(|capability| is_detection_capability(*capability))
        && !model
            .output_contract
            .output_types
            .iter()
            .any(|kind| matches!(kind, ArtifactKind::BoundingBox | ArtifactKind::DetectionSet))
    {
        return Err(CoreError::Validation(
            "detection capabilities require BoundingBox or DetectionSet output".to_owned(),
        ));
    }
    Ok(())
}

fn validate_unique_nonempty(values: &[String], field: &str) -> CoreResult<()> {
    let mut unique = std::collections::BTreeSet::new();
    for value in values {
        if value.trim().is_empty() || !unique.insert(value.as_str()) {
            return Err(CoreError::Validation(format!(
                "{field} values must be non-empty and unique"
            )));
        }
    }
    Ok(())
}

const fn is_detection_capability(capability: VisionCapability) -> bool {
    matches!(
        capability,
        VisionCapability::OpenVocabularyDetection
            | VisionCapability::PhraseGrounding
            | VisionCapability::ObjectDetection
    )
}

fn secret_configuration_path(fields: &BTreeMap<String, serde_json::Value>) -> Option<String> {
    fn is_secret_name(value: &str) -> bool {
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
    fn visit(value: &serde_json::Value, path: &str) -> Option<String> {
        match value {
            serde_json::Value::Object(object) => object.iter().find_map(|(key, value)| {
                let nested = format!("{path}.{key}");
                is_secret_name(key)
                    .then_some(nested.clone())
                    .or_else(|| visit(value, &nested))
            }),
            serde_json::Value::Array(values) => values
                .iter()
                .enumerate()
                .find_map(|(index, value)| visit(value, &format!("{path}[{index}]"))),
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => None,
        }
    }
    fields.iter().find_map(|(key, value)| {
        is_secret_name(key)
            .then_some(key.clone())
            .or_else(|| visit(value, key))
    })
}

const fn capability_output_type(capability: VisionCapability) -> Option<ArtifactKind> {
    match capability {
        VisionCapability::OpenVocabularyDetection
        | VisionCapability::PhraseGrounding
        | VisionCapability::ObjectDetection => Some(ArtifactKind::BoundingBox),
        VisionCapability::SemanticSegmentation => Some(ArtifactKind::SemanticMask),
        VisionCapability::InstanceSegmentation => Some(ArtifactKind::InstanceMask),
        VisionCapability::PromptedSegmentation => Some(ArtifactKind::MaskSet),
        VisionCapability::Classification => Some(ArtifactKind::Classification),
        VisionCapability::KeypointDetection => Some(ArtifactKind::Keypoints),
        VisionCapability::VisionLanguage | VisionCapability::Embedding => None,
    }
}

#[derive(Debug, Default, Clone)]
pub struct NodeRegistry {
    nodes: BTreeMap<String, VisionNodeDescriptor>,
    definitions: BTreeMap<String, crate::NodeDefinition>,
    runtime_policies: BTreeMap<String, crate::RuntimePolicyDefinition>,
}

impl NodeRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, node: VisionNodeDescriptor) -> CoreResult<()> {
        if node.id.trim().is_empty() || node.produces.is_empty() {
            return Err(CoreError::Validation(
                "node id cannot be empty and produces cannot be empty".to_owned(),
            ));
        }
        if self.nodes.insert(node.id.clone(), node).is_some() {
            return Err(CoreError::Validation(
                "vision node id is already registered".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn register_definition(&mut self, definition: crate::NodeDefinition) -> CoreResult<()> {
        definition.validate().map_err(CoreError::Validation)?;
        if !self.nodes.contains_key(&definition.id) {
            return Err(CoreError::Validation(format!(
                "public node definition {:?} has no executable operation descriptor",
                definition.id
            )));
        }
        if self
            .definitions
            .insert(definition.id.clone(), definition)
            .is_some()
        {
            return Err(CoreError::Validation(
                "public node definition id is already registered".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn register_runtime_policy(
        &mut self,
        policy: crate::RuntimePolicyDefinition,
    ) -> CoreResult<()> {
        if policy.id.trim().is_empty()
            || policy.display_name.trim().is_empty()
            || !policy.config_schema.is_object()
        {
            return Err(CoreError::Validation(
                "runtime policy id, display_name, and object schema are required".to_owned(),
            ));
        }
        if self
            .runtime_policies
            .insert(policy.id.clone(), policy)
            .is_some()
        {
            return Err(CoreError::Validation(
                "runtime policy id is already registered".to_owned(),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn nodes(&self) -> Vec<VisionNodeDescriptor> {
        self.nodes.values().cloned().collect()
    }

    #[must_use]
    pub fn definitions(&self) -> Vec<crate::NodeDefinition> {
        self.definitions.values().cloned().collect()
    }

    #[must_use]
    pub fn runtime_policies(&self) -> Vec<crate::RuntimePolicyDefinition> {
        self.runtime_policies.values().cloned().collect()
    }

    #[must_use]
    pub fn definition(&self, id: &str) -> Option<&crate::NodeDefinition> {
        self.definitions.get(id)
    }

    #[must_use]
    pub fn runtime_policy(&self, id: &str) -> Option<&crate::RuntimePolicyDefinition> {
        self.runtime_policies.get(id)
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&VisionNodeDescriptor> {
        self.nodes.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixtureBackend {
        kind: VisionBackendKind,
        capabilities: Vec<VisionCapability>,
    }

    #[async_trait]
    impl VisionModelBackend for FixtureBackend {
        fn id(&self) -> &str {
            "fixture"
        }

        fn kind(&self) -> VisionBackendKind {
            self.kind
        }

        fn capabilities(&self) -> Vec<VisionCapability> {
            self.capabilities.clone()
        }

        async fn infer(
            &self,
            _request: VisionInferenceRequest,
            _cancellation: CancellationToken,
        ) -> CoreResult<VisionInferenceResponse> {
            Ok(VisionInferenceResponse::default())
        }
    }

    fn registry(kind: VisionBackendKind, capabilities: Vec<VisionCapability>) -> ModelRegistry {
        let mut registry = ModelRegistry::new();
        registry
            .register_backend(Arc::new(FixtureBackend { kind, capabilities }))
            .expect("backend");
        registry
    }

    #[test]
    fn legacy_model_descriptor_migrates_to_structured_contract() {
        let legacy = serde_json::json!({
            "id": "legacy-detector",
            "backend_id": "fixture",
            "capabilities": ["object_detection"],
            "input_types": ["image"],
            "output_types": ["detection_set"],
            "model": "legacy",
            "model_version": "3",
            "endpoint_or_path": null,
            "secret_reference": null
        });
        let descriptor: VisionModelDescriptor =
            serde_json::from_value(legacy).expect("legacy descriptor");
        let mut registry = registry(
            VisionBackendKind::Mock,
            vec![VisionCapability::ObjectDetection],
        );
        registry.register_model(descriptor).expect("registered");

        let migrated = &registry.models()[0];
        assert_eq!(migrated.version.model_version, "3");
        assert_eq!(migrated.model_version, "3");
        assert_eq!(migrated.provider, "fixture");
        assert_eq!(migrated.backend.kind, Some(VisionBackendKind::Mock));
        assert_eq!(
            migrated.input_contract.input_types,
            vec![VisionInputType::Image]
        );
        assert_eq!(
            migrated.output_contract.output_types,
            vec![ArtifactKind::DetectionSet]
        );
        assert_eq!(migrated.score_semantics, ScoreSemantics::Unknown);
        assert_eq!(migrated.status, ModelAvailabilityStatus::Available);
    }

    #[test]
    fn rich_descriptor_preserves_version_license_and_open_vocabulary_capabilities() {
        let capabilities = vec![
            VisionCapability::OpenVocabularyDetection,
            VisionCapability::PhraseGrounding,
        ];
        let mut registry = registry(VisionBackendKind::HttpVision, capabilities.clone());
        registry
            .register_model(VisionModelDescriptor {
                id: "grounding-model".to_owned(),
                backend_id: "fixture".to_owned(),
                provider: "local-worker".to_owned(),
                backend: BackendDescriptor {
                    kind: Some(VisionBackendKind::HttpVision),
                    protocol_version: Some("1".to_owned()),
                    endpoint: Some("http://127.0.0.1:9000/v1/infer".to_owned()),
                },
                capabilities,
                model: "grounding-model".to_owned(),
                version: ModelVersionMetadata {
                    architecture: Some("grounding-transformer".to_owned()),
                    model_version: "1".to_owned(),
                    checkpoint_sha256: Some("a".repeat(64)),
                    training_dataset_version: Some("grounding-v1".to_owned()),
                    backend_protocol_version: "1".to_owned(),
                },
                input_contract: ModelInputContract {
                    input_types: vec![VisionInputType::Image, VisionInputType::Text],
                    supports_multiple_queries: true,
                    supports_visual_prompt: false,
                    max_queries: Some(32),
                },
                output_contract: ModelOutputContract {
                    output_types: vec![ArtifactKind::DetectionSet],
                    normalized_coordinates: true,
                    allows_empty: true,
                    label_space: Vec::new(),
                },
                score_semantics: ScoreSemantics::NotProvided,
                runtime_requirements: RuntimeRequirements {
                    devices: vec!["cuda".to_owned()],
                    minimum_gpu_memory_mb: Some(8_192),
                    dependencies: vec!["transformers".to_owned()],
                    supports_batch: true,
                },
                license: LicenseMetadata {
                    code_license: Some("Apache-2.0".to_owned()),
                    weight_license: Some("NVIDIA License".to_owned()),
                    source_url: Some(
                        "https://huggingface.co/example/model/blob/main/LICENSE".to_owned(),
                    ),
                    commercial_use: LicensePermission::Restricted,
                    redistribution: LicensePermission::Restricted,
                    usage_notes: vec!["Research/evaluation only".to_owned()],
                    verified_from_official_source: true,
                },
                status: ModelAvailabilityStatus::Available,
                ..VisionModelDescriptor::default()
            })
            .expect("rich descriptor");

        let stored = &registry.models()[0];
        assert_eq!(stored.model_version, "1");
        assert_eq!(stored.version.checkpoint_sha256, Some("a".repeat(64)));
        assert_eq!(stored.score_semantics, ScoreSemantics::NotProvided);
        assert_eq!(stored.license.commercial_use, LicensePermission::Restricted);
        assert!(stored.output_contract.allows_empty);
    }

    #[test]
    fn registry_rejects_invalid_model_metadata_and_disabled_resolution() {
        let mut registry = registry(
            VisionBackendKind::HttpVision,
            vec![VisionCapability::ObjectDetection],
        );
        let invalid = VisionModelDescriptor {
            id: "invalid".to_owned(),
            backend_id: "fixture".to_owned(),
            capabilities: vec![VisionCapability::ObjectDetection],
            version: ModelVersionMetadata {
                checkpoint_sha256: Some("not-a-digest".to_owned()),
                ..ModelVersionMetadata::default()
            },
            ..VisionModelDescriptor::default()
        };
        assert!(
            registry
                .register_model(invalid)
                .expect_err("invalid hash")
                .to_string()
                .contains("checkpoint_sha256")
        );

        registry
            .register_model(VisionModelDescriptor {
                id: "disabled".to_owned(),
                backend_id: "fixture".to_owned(),
                capabilities: vec![VisionCapability::ObjectDetection],
                status: ModelAvailabilityStatus::Disabled,
                ..VisionModelDescriptor::default()
            })
            .expect("disabled descriptor can remain registered");
        let Err(error) = registry.resolve("disabled") else {
            panic!("disabled model cannot execute")
        };
        assert!(error.to_string().contains("Disabled"));
    }

    #[test]
    fn unknown_worker_model_registers_from_manifest_without_a_core_variant() {
        let mut registry = registry(
            VisionBackendKind::HttpVision,
            vec![VisionCapability::ObjectDetection],
        );
        let manifest = crate::ExpertModelManifest {
            schema_version: "1".to_owned(),
            model_id: "test-edge-detector".to_owned(),
            display_name: "Test Edge Detector".to_owned(),
            architecture: Some("external-test-architecture".to_owned()),
            model_version: "2026.09".to_owned(),
            connection: crate::ModelConnection::VisionWorkerModel {
                worker_id: "fixture".to_owned(),
                worker_model_id: "test-edge-detector".to_owned(),
            },
            capabilities: std::collections::BTreeSet::from([
                crate::ModelCapability::ObjectDetection,
            ]),
            input_contracts: vec![crate::ArtifactContract::artifact(
                "image",
                ArtifactKind::Image,
                true,
                false,
            )],
            output_contracts: vec![crate::ArtifactContract::artifact(
                "detections",
                ArtifactKind::DetectionSet,
                true,
                true,
            )],
            prompt_contracts: Vec::new(),
            score_semantics: ScoreSemantics::RelativeConfidence,
            geometry_semantics: crate::GeometrySemantics::PredictedGeometry,
            label_space: Some(vec!["edge".to_owned()]),
            checkpoint: None,
            runtime_requirements: RuntimeRequirements::default(),
            license: LicenseMetadata::default(),
            availability: crate::ModelAvailability::Unknown,
            availability_evidence: crate::ModelAvailabilityEvidence::default(),
            metadata: BTreeMap::new(),
        };

        registry
            .register_expert_manifest(manifest)
            .expect("generic manifest registration");

        let descriptor = &registry.models()[0];
        assert_eq!(descriptor.id, "test-edge-detector");
        assert_eq!(
            descriptor.capabilities,
            vec![VisionCapability::ObjectDetection]
        );
        let stored = registry
            .expert_manifest("test-edge-detector")
            .expect("stored manifest");
        assert_eq!(
            stored.geometry_semantics,
            crate::GeometrySemantics::PredictedGeometry
        );
    }

    #[test]
    fn legacy_http_json_backend_kind_deserializes_as_http_vision() {
        let kind: VisionBackendKind =
            serde_json::from_str("\"http_json\"").expect("legacy backend kind");
        assert_eq!(kind, VisionBackendKind::HttpVision);
        assert_eq!(
            serde_json::to_string(&kind).expect("current backend kind"),
            "\"http_vision\""
        );
    }
}
