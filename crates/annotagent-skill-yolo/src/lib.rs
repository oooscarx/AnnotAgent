//! Detection-only Skill. Cropping is intentionally owned by the generic Core Crop node.

use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    ArtifactKind, ArtifactRef, CoreError, CoreResult, Detection, DetectionSetArtifact, LabelId,
    ModelImage, NormalizedRect, PIPELINE_VISION_PROTOCOL_VERSION, PipelineArtifact,
    PipelineInferenceRequest, PipelineInferenceResponse, PipelineModelBackend, VisionCapability,
    VisionNodeDescriptor,
};
use annotagent_runtime::{DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner};
use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio_util::sync::CancellationToken;

pub const YOLO_SKILL_ID: &str = "yolo-detection";
pub const YOLO_SKILL_VERSION: &str = "1";
pub const YOLO_DETECTION_OPERATION: &str = "yolo_detection.detect";

#[must_use]
pub fn node_descriptor() -> VisionNodeDescriptor {
    VisionNodeDescriptor {
        id: YOLO_DETECTION_OPERATION.to_owned(),
        display_name: "YOLO Detection".to_owned(),
        required_capabilities: vec![VisionCapability::ObjectDetection],
        accepts: vec![ArtifactKind::Image],
        produces: vec![ArtifactKind::DetectionSet],
        deterministic: false,
    }
}

pub struct YoloDetectionSkillRunner {
    backend: Arc<dyn PipelineModelBackend>,
    model_id: String,
    image: Option<ModelImage>,
}

impl YoloDetectionSkillRunner {
    pub fn new(
        backend: Arc<dyn PipelineModelBackend>,
        model_id: impl Into<String>,
        image: Option<ModelImage>,
    ) -> CoreResult<Self> {
        if backend.capability() != VisionCapability::ObjectDetection {
            return Err(CoreError::Validation(
                "YOLO Detection Skill requires an ObjectDetection backend".to_owned(),
            ));
        }
        Ok(Self {
            backend,
            model_id: model_id.into(),
            image,
        })
    }
}

#[async_trait]
impl DagNodeRunner for YoloDetectionSkillRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        if context.node.node_type != YOLO_DETECTION_OPERATION {
            return Err(DagNodeFailure::terminal(
                "wrong_skill_operation",
                "YOLO Detection runner received another operation",
            ));
        }
        if !context
            .input_pipeline_artifacts
            .iter()
            .any(|artifact| matches!(artifact, PipelineArtifact::Image(_)))
        {
            return Err(DagNodeFailure::terminal(
                "missing_image_input",
                "YOLO Detection requires Image input",
            ));
        }
        let model_id = context
            .node
            .model_binding
            .as_deref()
            .unwrap_or(&self.model_id);
        let response = self
            .backend
            .infer_pipeline(
                PipelineInferenceRequest {
                    protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
                    request_id: uuid::Uuid::new_v4().to_string(),
                    run_id: context.run_id,
                    image_id: context.image_id,
                    node_id: context.node.id.clone(),
                    model_id: model_id.to_owned(),
                    operation: VisionCapability::ObjectDetection,
                    image: self.image.clone(),
                    input_artifacts: context.input_pipeline_artifacts,
                    parameters: context.node.parameters.clone(),
                    timeout_ms: context
                        .node
                        .resources
                        .timeout_seconds
                        .map(|seconds| seconds.saturating_mul(1_000)),
                },
                context.cancellation,
            )
            .await
            .map_err(|error| DagNodeFailure::retryable("detection_backend", error.to_string()))?;
        if let Some(error) = response.error {
            return Err(DagNodeFailure {
                code: error.code,
                summary: error.message,
                retryable: error.retryable,
            });
        }
        if response.artifacts.is_empty()
            || response.artifacts.iter().any(|artifact| {
                !matches!(artifact, PipelineArtifact::DetectionSet(_))
                    || artifact.image_id() != context.image_id
                    || artifact.reference().source_node != context.node.id
            })
        {
            return Err(DagNodeFailure::terminal(
                "invalid_detection_output",
                "YOLO backend must return scoped DetectionSet Artifacts only",
            ));
        }
        Ok(DagNodeOutput {
            pipeline_artifacts: response.artifacts,
            metadata: response.metadata,
            usage: annotagent_runtime::DagNodeUsage {
                input_tokens: 0,
                output_tokens: 0,
                cost: Decimal::ZERO,
            },
            ..DagNodeOutput::default()
        })
    }
}

#[derive(Debug, Clone)]
pub struct MockYoloBackend {
    id: String,
}

impl MockYoloBackend {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl PipelineModelBackend for MockYoloBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn capability(&self) -> VisionCapability {
        VisionCapability::ObjectDetection
    }

    async fn infer_pipeline(
        &self,
        request: PipelineInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<PipelineInferenceResponse> {
        if cancellation.is_cancelled() {
            return Err(CoreError::Provider(
                "mock YOLO inference cancelled".to_owned(),
            ));
        }
        let count = request
            .parameters
            .get("mock_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1)
            .min(100) as usize;
        let class_id = request
            .parameters
            .get("mock_class_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("0");
        let label = request
            .parameters
            .get("mock_label")
            .and_then(serde_json::Value::as_str)
            .map(LabelId::from);
        let confidence = request
            .parameters
            .get("mock_confidence")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.95) as f32;
        let detections = (0..count)
            .map(|index| {
                let offset = (index as f32 * 0.03).min(0.4);
                Ok(Detection {
                    id: format!("detection-{index}"),
                    class_id: class_id.to_owned(),
                    label: label.clone(),
                    rect: NormalizedRect::new(0.1 + offset, 0.1 + offset, 0.25, 0.25)?,
                    confidence,
                    attributes: BTreeMap::new(),
                })
            })
            .collect::<CoreResult<Vec<_>>>()?;
        let artifact = DetectionSetArtifact {
            reference: ArtifactRef {
                artifact_id: format!("detection-set:{}", request.request_id),
                source_node: request.node_id,
                port: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
                item_id: None,
            },
            image_id: request.image_id,
            model_binding: request.model_id,
            validation_state: annotagent_core::ArtifactValidationState::Unvalidated,
            detections,
            metadata: BTreeMap::from([("backend".to_owned(), serde_json::json!("mock"))]),
        };
        artifact.validate().map_err(CoreError::Validation)?;
        Ok(PipelineInferenceResponse {
            request_id: Some(request.request_id),
            model_identity: Some(self.id.clone()),
            artifacts: vec![PipelineArtifact::DetectionSet(artifact)],
            metadata: BTreeMap::from([("mode".to_owned(), serde_json::json!("mock"))]),
            ..PipelineInferenceResponse::default()
        })
    }
}
