//! Domain-neutral registries and contracts for hybrid vision workflow nodes.

use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::{CoreError, CoreResult, ImageId, ModelImage, RunId, TaskId, VisionArtifact};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisionBackendKind {
    OpenAiCompatible,
    HttpJson,
    Onnx,
    Mock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Classification,
    BoundingBox,
    Keypoints,
    Polyline,
    Polygon,
    SemanticMask,
    InstanceMask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisionModelDescriptor {
    pub id: String,
    pub backend_id: String,
    pub capabilities: Vec<VisionCapability>,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VisionInferenceResponse {
    #[serde(default)]
    pub artifacts: Vec<VisionArtifact>,
    pub request_id: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
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

    pub fn register_model(&mut self, model: VisionModelDescriptor) -> CoreResult<()> {
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
