//! Structured VLM object detection.
//!
//! This Skill owns only the Image -> `DetectionSet` model operation. Filtering, cropping,
//! review, and commit remain generic Core nodes.

use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    ArtifactKind, CoreError, CoreResult, ModelImage, NodePort, PipelineArtifact,
    PipelineInferenceRequest, PipelineModelBackend, Skill, SkillKind, SkillManifest, SkillResource,
    SkillResourceRequest, TaskId, TaskTemplate, VisionCapability, VisionNodeDescriptor,
    WorkflowDraftNode, WorkflowEdge, WorkflowNodeKind, WorkflowTemplate,
};
use annotagent_runtime::{DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner};
use async_trait::async_trait;
use rust_decimal::Decimal;

pub const VLM_DETECTION_SKILL_ID: &str = "vlm-detection";
pub const VLM_DETECTION_SKILL_VERSION: &str = "1";
pub const VLM_DETECTION_OPERATION: &str = "vlm_detection.detect";

pub struct VlmDetectionCapabilitySkill {
    manifest: SkillManifest,
}

impl Default for VlmDetectionCapabilitySkill {
    fn default() -> Self {
        Self {
            manifest: SkillManifest {
                version: 1,
                id: VLM_DETECTION_SKILL_ID.to_owned(),
                kind: SkillKind::Capability,
                skill_version: VLM_DETECTION_SKILL_VERSION.to_owned(),
                display_name: "VLM Detection".to_owned(),
                description:
                    "Registry-bounded structured object grounding with a vision-language model"
                        .to_owned(),
                rust_implementation: Some(
                    "annotagent_skill_vlm_detection::VlmDetectionCapabilitySkill".to_owned(),
                ),
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                capabilities: vec!["vision_language_detection".to_owned()],
                nodes: vec![VLM_DETECTION_OPERATION.to_owned()],
                tools: vec!["submit_detections".to_owned()],
                validators: Vec::new(),
                policies: Vec::new(),
                templates: vec!["vlm-detection.structured".to_owned()],
                summary_resources: vec!["vlm-detection/summary.md".to_owned()],
                task_resources: BTreeMap::new(),
                correction_taxonomy: Vec::new(),
                visual_profile: BTreeMap::new(),
            },
        }
    }
}

impl Skill for VlmDetectionCapabilitySkill {
    fn id(&self) -> &str {
        VLM_DETECTION_SKILL_ID
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn node_templates(&self) -> Vec<TaskTemplate> {
        vec![TaskTemplate {
            id: TaskId::from(VLM_DETECTION_OPERATION),
            description: "Image → structured DetectionSet, including valid empty sets".to_owned(),
        }]
    }

    fn workflow_templates(&self) -> Vec<WorkflowTemplate> {
        vec![structured_detection_template()]
    }

    fn resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        match request.resource_name.as_deref() {
            None | Some("vlm-detection/summary.md") => Ok(vec![SkillResource {
                name: "vlm-detection/summary.md".to_owned(),
                media_type: "text/markdown".to_owned(),
                content: "Declare allowed labels and a visual target definition. The model submits only a scoped DetectionSet; Core owns Filter, Crop, gates and Commit.".to_owned(),
            }]),
            Some(other) => Err(CoreError::Validation(format!(
                "unknown VLM Detection resource {other:?}"
            ))),
        }
    }
}

fn port(id: &str, artifact_type: ArtifactKind) -> NodePort {
    NodePort {
        id: id.to_owned(),
        artifact_type,
        required: true,
        multiple: false,
    }
}

fn structured_detection_template() -> WorkflowTemplate {
    let node = |id: &str, node_type: &str, kind, inputs, outputs| WorkflowDraftNode {
        id: id.to_owned(),
        node_type: node_type.to_owned(),
        kind,
        inputs,
        outputs,
        required_skills: vec![VLM_DETECTION_SKILL_ID.to_owned()],
        ..WorkflowDraftNode::default()
    };
    WorkflowTemplate {
        id: "vlm-detection.structured".to_owned(),
        name: "Structured VLM Detection".to_owned(),
        description: "Image → VLM Detection → Confidence Gate → Commit".to_owned(),
        nodes: vec![
            node(
                "image",
                "core.image_input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("image", ArtifactKind::Image)],
            ),
            node(
                "detector",
                VLM_DETECTION_OPERATION,
                WorkflowNodeKind::VisionLanguageModel,
                vec![port("image", ArtifactKind::Image)],
                vec![port("detections", ArtifactKind::DetectionSet)],
            ),
            node(
                "gate",
                "core.confidence_gate",
                WorkflowNodeKind::Gate,
                vec![port("detections", ArtifactKind::DetectionSet)],
                vec![port("detections", ArtifactKind::DetectionSet)],
            ),
            node(
                "commit",
                "core.commit",
                WorkflowNodeKind::Commit,
                vec![port("detections", ArtifactKind::DetectionSet)],
                Vec::new(),
            ),
        ],
        edges: vec![
            WorkflowEdge {
                from_node: "image".to_owned(),
                from_port: "image".to_owned(),
                to_node: "detector".to_owned(),
                to_port: "image".to_owned(),
                route: None,
            },
            WorkflowEdge {
                from_node: "detector".to_owned(),
                from_port: "detections".to_owned(),
                to_node: "gate".to_owned(),
                to_port: "detections".to_owned(),
                route: None,
            },
            WorkflowEdge {
                from_node: "gate".to_owned(),
                from_port: "detections".to_owned(),
                to_node: "commit".to_owned(),
                to_port: "detections".to_owned(),
                route: Some("pass".to_owned()),
            },
        ],
        resource_versions: BTreeMap::from([(
            "vlm-detection/summary.md".to_owned(),
            VLM_DETECTION_SKILL_VERSION.to_owned(),
        )]),
        allow_unvalidated_commit: false,
    }
}

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

#[cfg(test)]
mod tests {
    use annotagent_core::{SkillKind, SkillResourceRequest};

    use super::*;

    #[test]
    fn manifest_and_template_keep_crop_outside_detection_skill() {
        let skill = VlmDetectionCapabilitySkill::default();
        assert_eq!(skill.manifest().kind, SkillKind::Capability);
        assert!(skill.manifest().validate().is_empty());
        let serialized = serde_json::to_string(&skill.workflow_templates()).expect("templates");
        assert!(!serialized.contains("core.crop"));
        assert_eq!(
            skill
                .resources(&SkillResourceRequest {
                    task_id: None,
                    resource_name: Some("vlm-detection/summary.md".to_owned()),
                })
                .expect("resource")
                .len(),
            1
        );
    }
}
