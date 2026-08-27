//! Domain-neutral registries and contracts for hybrid vision workflow nodes.

use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::{CoreError, CoreResult, ImageId, ModelImage, RunId, TaskId, VisionArtifact};

pub const VISION_WORKER_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisionCapability {
    VisionLanguage,
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
    HttpJson,
    Onnx,
    DeterministicCv,
    Mock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Image,
    DetectionSet,
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
    pub capabilities: Vec<VisionCapability>,
    #[serde(default)]
    pub input_types: Vec<VisionInputType>,
    #[serde(default)]
    pub output_types: Vec<ArtifactKind>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub model_version: String,
    pub endpoint_or_path: Option<String>,
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
        if model.display_name.trim().is_empty() {
            model.display_name.clone_from(&model.id);
        }
        if model.model.trim().is_empty() {
            model.model.clone_from(&model.id);
        }
        if model.model_version.trim().is_empty() {
            "unversioned".clone_into(&mut model.model_version);
        }
        if model.input_types.is_empty() {
            model.input_types.push(VisionInputType::Image);
        }
        if model.output_types.is_empty() {
            model.output_types = model
                .capabilities
                .iter()
                .copied()
                .filter_map(capability_output_type)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
        }
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
        if self.models.insert(model.id.clone(), model).is_some() {
            return Err(CoreError::Validation(
                "vision model id is already registered".to_owned(),
            ));
        }
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

    pub fn resolve(
        &self,
        model_id: &str,
    ) -> CoreResult<(&VisionModelDescriptor, Arc<dyn VisionModelBackend>)> {
        let model = self
            .models
            .get(model_id)
            .ok_or_else(|| CoreError::Validation(format!("unknown vision model {model_id:?}")))?;
        let backend = self.backends.get(&model.backend_id).ok_or_else(|| {
            CoreError::Validation(format!("unknown vision backend {:?}", model.backend_id))
        })?;
        Ok((model, backend.clone()))
    }
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
        VisionCapability::ObjectDetection => Some(ArtifactKind::BoundingBox),
        VisionCapability::SemanticSegmentation => Some(ArtifactKind::SemanticMask),
        VisionCapability::InstanceSegmentation | VisionCapability::PromptedSegmentation => {
            Some(ArtifactKind::InstanceMask)
        }
        VisionCapability::Classification => Some(ArtifactKind::Classification),
        VisionCapability::KeypointDetection => Some(ArtifactKind::Keypoints),
        VisionCapability::VisionLanguage | VisionCapability::Embedding => None,
    }
}

#[derive(Debug, Default, Clone)]
pub struct NodeRegistry {
    nodes: BTreeMap<String, VisionNodeDescriptor>,
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

    #[must_use]
    pub fn nodes(&self) -> Vec<VisionNodeDescriptor> {
        self.nodes.values().cloned().collect()
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&VisionNodeDescriptor> {
        self.nodes.get(id)
    }
}
