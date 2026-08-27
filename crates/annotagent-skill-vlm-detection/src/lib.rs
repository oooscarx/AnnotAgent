//! Structured VLM object detection.
//!
//! This Skill owns only the Image -> `DetectionSet` model operation. Filtering, cropping,
//! review, and commit remain generic Core nodes.

use std::sync::Arc;

use annotagent_core::{
    ArtifactKind, CoreError, CoreResult, ModelImage, PipelineArtifact, PipelineInferenceRequest,
    PipelineModelBackend, VisionCapability, VisionNodeDescriptor,
};
use annotagent_runtime::{DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner};
use async_trait::async_trait;
use rust_decimal::Decimal;

pub const VLM_DETECTION_SKILL_ID: &str = "vlm-detection";
pub const VLM_DETECTION_SKILL_VERSION: &str = "1";
pub const VLM_DETECTION_OPERATION: &str = "vlm_detection.detect";

#[must_use]
pub fn node_descriptor() -> VisionNodeDescriptor {
    VisionNodeDescriptor {
        id: VLM_DETECTION_OPERATION.to_owned(),
        display_name: "VLM Detection".to_owned(),
        required_capabilities: vec![VisionCapability::VisionLanguage],
        accepts: vec![ArtifactKind::Image],
        produces: vec![ArtifactKind::DetectionSet],
        deterministic: false,
    }
}

pub struct VlmDetectionSkillRunner {
    backend: Arc<dyn PipelineModelBackend>,
    model_id: String,
    image: Option<ModelImage>,
}

impl VlmDetectionSkillRunner {
    pub fn new(
        backend: Arc<dyn PipelineModelBackend>,
        model_id: impl Into<String>,
        image: Option<ModelImage>,
    ) -> CoreResult<Self> {
        if backend.capability() != VisionCapability::VisionLanguage {
            return Err(CoreError::Validation(
                "VLM Detection Skill requires a VisionLanguage backend".to_owned(),
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
impl DagNodeRunner for VlmDetectionSkillRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        if context.node.node_type != VLM_DETECTION_OPERATION {
            return Err(DagNodeFailure::terminal(
                "wrong_skill_operation",
                "VLM Detection runner received another operation",
            ));
        }
        if !context
            .input_pipeline_artifacts
            .iter()
            .any(|artifact| matches!(artifact, PipelineArtifact::Image(_)))
        {
            return Err(DagNodeFailure::terminal(
                "missing_image_input",
                "VLM Detection requires Image input",
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
                    protocol_version: annotagent_core::PIPELINE_VISION_PROTOCOL_VERSION,
                    request_id: uuid::Uuid::new_v4().to_string(),
                    run_id: context.run_id,
                    image_id: context.image_id,
                    node_id: context.node.id.clone(),
                    model_id: model_id.to_owned(),
                    operation: VisionCapability::VisionLanguage,
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
            .map_err(|error| {
                DagNodeFailure::retryable("vlm_detection_backend", error.to_string())
            })?;
        if let Some(error) = response.error {
            return Err(DagNodeFailure {
                code: error.code,
                summary: error.message,
                retryable: error.retryable,
            });
        }
        if response.artifacts.len() != 1
            || response.artifacts.iter().any(|artifact| {
                !matches!(artifact, PipelineArtifact::DetectionSet(_))
                    || artifact.image_id() != context.image_id
                    || artifact.reference().source_node != context.node.id
            })
        {
            return Err(DagNodeFailure::terminal(
                "invalid_vlm_detection_output",
                "VLM Detection backend must return one scoped DetectionSet Artifact",
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
