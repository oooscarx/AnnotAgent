//! Backend-neutral Segmentation Capability.
//!
//! The Alpha advertises the semantic contract without pretending a runnable model exists. A
//! healthy Model Descriptor must provide one of the declared capabilities before an authoring
//! service may bind a segmentation node.

use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ArtifactValidationState, BoxPromptSetArtifact, CoreError,
    CoreResult, DetectionScore, MaskArtifactItem, MaskEncoding, MaskSetArtifact, ModelImage,
    NormalizedPoint, PIPELINE_VISION_PROTOCOL_VERSION, PipelineArtifact, PipelineInferenceRequest,
    PipelineInferenceResponse, PipelineModelBackend, ScoreSemantics, Skill, SkillKind,
    SkillManifest, SkillProductVisibility, SkillResource, SkillResourceRequest, VisionCapability,
};
use annotagent_runtime::{DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner};
use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio_util::sync::CancellationToken;

pub const SEGMENTATION_SKILL_ID: &str = "annotagent.segmentation";
pub const SEGMENTATION_SKILL_VERSION: &str = "1";
pub const PROMPTED_SEGMENTATION_OPERATION: &str = "capability.segment";
pub const SEMANTIC_SEGMENTATION_OPERATION: &str = "capability.semantic_segment";

pub struct SegmentationCapabilitySkill {
    manifest: SkillManifest,
}

/// Generic prompted-segmentation runner. The backend may be SAM, another compatible Worker, or
/// the offline test backend below; model branding never changes the node contract.
pub struct PromptedSegmentationRunner {
    backend: Arc<dyn PipelineModelBackend>,
    model_id: String,
    image: Option<ModelImage>,
}

/// Generic semantic-segmentation runner. Model branding and checkpoint loading stay behind the
/// `PipelineModelBackend`; this runner enforces the Image-to-SemanticMask protocol boundary.
pub struct SemanticSegmentationRunner {
    backend: Arc<dyn PipelineModelBackend>,
    model_id: String,
    image: Option<ModelImage>,
}

impl SemanticSegmentationRunner {
    pub fn new(
        backend: Arc<dyn PipelineModelBackend>,
        model_id: impl Into<String>,
        image: Option<ModelImage>,
    ) -> CoreResult<Self> {
        if backend.capability() != VisionCapability::SemanticSegmentation {
            return Err(CoreError::Validation(
                "Semantic Segmentation requires a SemanticSegmentation backend".to_owned(),
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
impl DagNodeRunner for SemanticSegmentationRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        if context.node.node_type != SEMANTIC_SEGMENTATION_OPERATION {
            return Err(DagNodeFailure::terminal(
                "wrong_skill_operation",
                "Semantic Segmentation runner received another operation",
            ));
        }
        let image_count = context
            .input_pipeline_artifacts
            .iter()
            .filter(|artifact| matches!(artifact, PipelineArtifact::Image(_)))
            .count();
        if image_count != 1 || context.input_pipeline_artifacts.len() != 1 {
            return Err(DagNodeFailure::terminal(
                "invalid_semantic_segmentation_inputs",
                "Semantic Segmentation requires exactly one Image Artifact",
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
                    operation: VisionCapability::SemanticSegmentation,
                    image: self.image.clone(),
                    input_artifacts: context.input_pipeline_artifacts.clone(),
                    parameters: context.node.parameters.clone(),
                    timeout_ms: context
                        .node
                        .resources
                        .timeout_seconds
                        .map(|seconds| seconds.saturating_mul(1_000)),
                },
                context.cancellation.clone(),
            )
            .await
            .map_err(|error| {
                DagNodeFailure::retryable("semantic_segmentation_backend", error.to_string())
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
                !matches!(artifact, PipelineArtifact::SemanticMask(_))
                    || artifact.image_id() != context.image_id
                    || artifact.reference().source_node != context.node.id
            })
        {
            return Err(DagNodeFailure::terminal(
                "invalid_semantic_segmentation_output",
                "Semantic Segmentation backend must return exactly one scoped SemanticMask",
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

impl PromptedSegmentationRunner {
    pub fn new(
        backend: Arc<dyn PipelineModelBackend>,
        model_id: impl Into<String>,
        image: Option<ModelImage>,
    ) -> CoreResult<Self> {
        if backend.capability() != VisionCapability::PromptedSegmentation {
            return Err(CoreError::Validation(
                "Prompted Segmentation requires a PromptedSegmentation backend".to_owned(),
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
impl DagNodeRunner for PromptedSegmentationRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        if context.node.node_type != PROMPTED_SEGMENTATION_OPERATION {
            return Err(DagNodeFailure::terminal(
                "wrong_skill_operation",
                "Prompted Segmentation runner received another operation",
            ));
        }
        let has_image = context
            .input_pipeline_artifacts
            .iter()
            .any(|artifact| matches!(artifact, PipelineArtifact::Image(_)));
        let prompt_sets = context
            .input_pipeline_artifacts
            .iter()
            .filter(|artifact| {
                matches!(
                    artifact,
                    PipelineArtifact::BoxPromptSet(_) | PipelineArtifact::PointPromptSet(_)
                )
            })
            .count();
        if !has_image || prompt_sets != 1 {
            return Err(DagNodeFailure::terminal(
                "invalid_segmentation_inputs",
                "Prompted Segmentation requires Image and exactly one BoxPromptSet or PointPromptSet",
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
                    operation: VisionCapability::PromptedSegmentation,
                    image: self.image.clone(),
                    input_artifacts: context.input_pipeline_artifacts.clone(),
                    parameters: context.node.parameters.clone(),
                    timeout_ms: context
                        .node
                        .resources
                        .timeout_seconds
                        .map(|seconds| seconds.saturating_mul(1_000)),
                },
                context.cancellation.clone(),
            )
            .await
            .map_err(|error| {
                DagNodeFailure::retryable("prompted_segmentation_backend", error.to_string())
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
                !matches!(artifact, PipelineArtifact::MaskSet(_))
                    || artifact.image_id() != context.image_id
                    || artifact.reference().source_node != context.node.id
            })
        {
            return Err(DagNodeFailure::terminal(
                "invalid_segmentation_output",
                "Prompted Segmentation backend must return exactly one scoped MaskSet",
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
pub struct MockPromptedSegmentationBackend {
    id: String,
}

impl MockPromptedSegmentationBackend {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl PipelineModelBackend for MockPromptedSegmentationBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn capability(&self) -> VisionCapability {
        VisionCapability::PromptedSegmentation
    }

    async fn infer_pipeline(
        &self,
        request: PipelineInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<PipelineInferenceResponse> {
        if cancellation.is_cancelled() {
            return Err(CoreError::Provider(
                "mock prompted segmentation cancelled".to_owned(),
            ));
        }
        let prompts = request
            .input_artifacts
            .iter()
            .find_map(|artifact| match artifact {
                PipelineArtifact::BoxPromptSet(prompts) => Some(prompts),
                _ => None,
            })
            .ok_or_else(|| {
                CoreError::Validation(
                    "mock prompted segmentation currently requires BoxPromptSet".to_owned(),
                )
            })?;
        let inset = request
            .parameters
            .get("mock_inset")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.08) as f32;
        if !inset.is_finite() || !(0.0..0.5).contains(&inset) {
            return Err(CoreError::Validation(
                "mock_inset must be finite and within [0,0.5)".to_owned(),
            ));
        }
        let masks = prompts
            .prompts
            .iter()
            .map(|prompt| mock_mask(prompt, prompts, inset))
            .collect::<CoreResult<Vec<_>>>()?;
        let artifact = MaskSetArtifact {
            reference: ArtifactRef {
                artifact_id: format!(
                    "mask-set:{}:{}:{}",
                    request.run_id, request.image_id, request.node_id
                ),
                source_node: request.node_id.clone(),
                port: "masks".to_owned(),
                artifact_type: ArtifactKind::MaskSet,
                item_id: None,
            },
            image_id: request.image_id,
            model_binding: request.model_id.clone(),
            source_prompts: prompts.reference.clone(),
            validation_state: ArtifactValidationState::Unvalidated,
            masks,
            metadata: BTreeMap::from([(
                "backend".to_owned(),
                serde_json::json!("mock_prompted_segmentation"),
            )]),
        };
        artifact.validate().map_err(CoreError::Validation)?;
        Ok(PipelineInferenceResponse {
            protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
            request_id: Some(request.request_id),
            model_identity: Some(request.model_id),
            artifacts: vec![PipelineArtifact::MaskSet(artifact)],
            ..PipelineInferenceResponse::default()
        })
    }
}

fn mock_mask(
    prompt: &annotagent_core::BoxPrompt,
    prompts: &BoxPromptSetArtifact,
    inset: f32,
) -> CoreResult<MaskArtifactItem> {
    let dx = prompt.bbox.width() * inset;
    let dy = prompt.bbox.height() * inset;
    let left = prompt.bbox.x() + dx;
    let top = prompt.bbox.y() + dy;
    let right = prompt.bbox.x() + prompt.bbox.width() - dx;
    let bottom = prompt.bbox.y() + prompt.bbox.height() - dy;
    let point = |x, y| NormalizedPoint::new(x, y);
    Ok(MaskArtifactItem {
        mask_id: format!("mask:{}", prompt.id),
        prompt: prompts.reference.item(&prompt.id),
        mask: MaskEncoding::Polygon {
            rings: vec![vec![
                point(left, top)?,
                point(right, top)?,
                point(right, bottom)?,
                point(left, bottom)?,
            ]],
        },
        score: DetectionScore::new(Some(0.95), ScoreSemantics::RelativeConfidence)
            .map_err(CoreError::Validation)?,
        attributes: BTreeMap::new(),
    })
}

impl Default for SegmentationCapabilitySkill {
    fn default() -> Self {
        Self {
            manifest: SkillManifest {
                version: 1,
                id: SEGMENTATION_SKILL_ID.to_owned(),
                kind: SkillKind::Capability,
                skill_version: SEGMENTATION_SKILL_VERSION.to_owned(),
                display_name: "Segmentation".to_owned(),
                description:
                    "Create semantic, prompted or instance masks with a compatible Model Backend"
                        .to_owned(),
                product_visibility: SkillProductVisibility::Primary,
                deprecated_alias_for: None,
                rust_implementation: Some(
                    "annotagent_skill_segmentation::SegmentationCapabilitySkill".to_owned(),
                ),
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                capabilities: vec![
                    "semantic_segmentation".to_owned(),
                    "prompted_segmentation".to_owned(),
                    "instance_segmentation".to_owned(),
                ],
                requires: annotagent_core::SkillCapabilityRequirements::default(),
                optional_capabilities: Vec::new(),
                nodes: Vec::new(),
                tools: Vec::new(),
                validators: Vec::new(),
                policies: Vec::new(),
                templates: Vec::new(),
                summary_resources: vec!["segmentation/summary.md".to_owned()],
                task_resources: BTreeMap::new(),
                correction_taxonomy: Vec::new(),
                visual_profile: BTreeMap::new(),
            },
        }
    }
}

impl Skill for SegmentationCapabilitySkill {
    fn id(&self) -> &str {
        SEGMENTATION_SKILL_ID
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        match request.resource_name.as_deref() {
            None | Some("segmentation/summary.md") => Ok(vec![SkillResource {
                name: "segmentation/summary.md".to_owned(),
                media_type: "text/markdown".to_owned(),
                content: "Segmentation is a generic Capability. Bind only a healthy Model Backend that declares semantic_segmentation, prompted_segmentation or instance_segmentation. SAM is one optional prompted-segmentation backend, not a Skill."
                    .to_owned(),
            }]),
            Some(other) => Err(CoreError::Validation(format!(
                "unknown Segmentation resource {other:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segmentation_is_generic_and_does_not_claim_an_available_backend() {
        let skill = SegmentationCapabilitySkill::default();
        assert_eq!(skill.id(), SEGMENTATION_SKILL_ID);
        assert!(skill.manifest().nodes.is_empty());
        assert!(skill.manifest().templates.is_empty());
        assert!(
            skill
                .manifest()
                .capabilities
                .contains(&"prompted_segmentation".to_owned())
        );
        assert!(!skill.manifest().description.contains("SAM"));
    }
}
