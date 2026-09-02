use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ArtifactValidationState, DetectionArtifactItem, DetectionScore,
    DetectionSetArtifact, DetectionSource, NormalizedRect, PipelineArtifact,
    PipelineInferenceRequest, PipelineInferenceResponse, ScoreSemantics, VisionCapability,
};
use annotagent_plugin_api::{
    ModelRuntimeDescriptor, PLUGIN_API_VERSION, PLUGIN_PROTOCOL_VERSION, PluginManifest,
    PluginRuntimeDescriptor,
};
use annotagent_plugin_sdk::{
    ExpertModelPlugin, InferenceContext, PluginSdkError, PluginServer, WarmupContext,
};
use async_trait::async_trait;

struct DummyDetector {
    manifest: PluginManifest,
}

impl DummyDetector {
    fn load() -> Result<Self, PluginSdkError> {
        let manifest = PluginManifest::from_toml(include_str!("../annotagent-plugin.toml"))
            .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        Ok(Self { manifest })
    }
}

#[async_trait]
impl ExpertModelPlugin for DummyDetector {
    fn descriptor(&self) -> PluginRuntimeDescriptor {
        PluginRuntimeDescriptor {
            plugin_id: self.manifest.id.clone(),
            plugin_version: self.manifest.version.clone(),
            plugin_api: PLUGIN_API_VERSION.to_owned(),
            protocol_version: PLUGIN_PROTOCOL_VERSION.to_owned(),
            capabilities: self.manifest.models[0].capabilities.clone(),
        }
    }

    fn models(&self) -> Vec<ModelRuntimeDescriptor> {
        vec![ModelRuntimeDescriptor {
            model: self.manifest.models[0].clone(),
            loaded: true,
            checkpoint_sha256: None,
            device: "cpu".to_owned(),
        }]
    }

    async fn warmup(&self, model_id: &str, _context: WarmupContext) -> Result<(), PluginSdkError> {
        if self.manifest.models[0].id == model_id {
            Ok(())
        } else {
            Err(PluginSdkError::Plugin("unknown model".to_owned()))
        }
    }

    async fn infer(
        &self,
        request: PipelineInferenceRequest,
        context: InferenceContext,
    ) -> Result<PipelineInferenceResponse, PluginSdkError> {
        if context.cancellation.is_cancelled() {
            return Err(PluginSdkError::Plugin("request cancelled".to_owned()));
        }
        let artifact_id = format!("dummy-detections:{}", request.request_id);
        let reference = ArtifactRef {
            artifact_id: artifact_id.clone(),
            source_node: request.node_id,
            port: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            item_id: None,
        };
        let detection = DetectionArtifactItem::from_source(
            "dummy-1",
            None,
            Some("object".to_owned()),
            None,
            NormalizedRect::new(0.25, 0.25, 0.5, 0.5)
                .map_err(|error| PluginSdkError::Plugin(error.to_string()))?,
            DetectionScore::new(Some(0.9), ScoreSemantics::DetectionConfidence)
                .map_err(PluginSdkError::Plugin)?,
            DetectionSource {
                model_id: request.model_id.clone(),
                capability: VisionCapability::ObjectDetection,
                artifact_id,
            },
        )
        .map_err(PluginSdkError::Plugin)?;
        Ok(PipelineInferenceResponse {
            request_id: Some(request.request_id),
            model_identity: Some(request.model_id.clone()),
            artifacts: vec![PipelineArtifact::DetectionSet(DetectionSetArtifact {
                schema_version: annotagent_core::DETECTION_ARTIFACT_SCHEMA_VERSION,
                reference,
                image_id: request.image_id,
                model_binding: request.model_id,
                validation_state: ArtifactValidationState::Unvalidated,
                detections: vec![detection],
                metadata: BTreeMap::from([(
                    "conformance_fixture".to_owned(),
                    serde_json::Value::Bool(true),
                )]),
            })],
            ..PipelineInferenceResponse::default()
        })
    }

    async fn cancel(&self, _request_id: &str) -> Result<(), PluginSdkError> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), PluginSdkError> {
    PluginServer::run_from_stdin(Arc::new(DummyDetector::load()?)).await
}
