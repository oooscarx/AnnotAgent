//! Detection-only Skill. Cropping is intentionally owned by the generic Core Crop node.

use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    ArtifactKind, ArtifactRef, CoreError, CoreResult, DETECTION_ARTIFACT_SCHEMA_VERSION, Detection,
    DetectionScore, DetectionSetArtifact, DetectionSource, LabelId, ModelImage, NodePort,
    NormalizedRect, PIPELINE_VISION_PROTOCOL_VERSION, PipelineArtifact, PipelineInferenceRequest,
    PipelineInferenceResponse, PipelineModelBackend, Skill, SkillKind, SkillManifest,
    SkillResource, SkillResourceRequest, TaskId, TaskTemplate, VisionCapability,
    VisionNodeDescriptor, WorkflowDraftNode, WorkflowEdge, WorkflowNodeKind, WorkflowTemplate,
};
use annotagent_runtime::{DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner};
use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio_util::sync::CancellationToken;

pub const YOLO_SKILL_ID: &str = "yolo-detection";
pub const YOLO_SKILL_VERSION: &str = "1";
pub const YOLO_DETECTION_OPERATION: &str = "yolo_detection.detect";

pub struct YoloCapabilitySkill {
    manifest: SkillManifest,
}

impl Default for YoloCapabilitySkill {
    fn default() -> Self {
        Self {
            manifest: SkillManifest {
                version: 1,
                id: YOLO_SKILL_ID.to_owned(),
                kind: SkillKind::Capability,
                skill_version: YOLO_SKILL_VERSION.to_owned(),
                display_name: "YOLO Detection".to_owned(),
                description: "Detection-only YOLO capability through Mock or HTTP JSON".to_owned(),
                rust_implementation: Some("annotagent_skill_yolo::YoloCapabilitySkill".to_owned()),
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                capabilities: vec!["object_detection".to_owned()],
                requires: annotagent_core::SkillCapabilityRequirements::default(),
                optional_capabilities: Vec::new(),
                nodes: vec![YOLO_DETECTION_OPERATION.to_owned()],
                tools: Vec::new(),
                validators: Vec::new(),
                policies: Vec::new(),
                templates: vec!["yolo.detection".to_owned()],
                summary_resources: vec!["yolo/summary.md".to_owned()],
                task_resources: BTreeMap::new(),
                correction_taxonomy: Vec::new(),
                visual_profile: BTreeMap::new(),
            },
        }
    }
}

impl Skill for YoloCapabilitySkill {
    fn id(&self) -> &str {
        YOLO_SKILL_ID
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn node_templates(&self) -> Vec<TaskTemplate> {
        vec![TaskTemplate {
            id: TaskId::from(YOLO_DETECTION_OPERATION),
            description: "Image → DetectionSet; never crops pixels".to_owned(),
        }]
    }

    fn workflow_templates(&self) -> Vec<WorkflowTemplate> {
        vec![detection_template()]
    }

    fn resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        match request.resource_name.as_deref() {
            None | Some("yolo/summary.md") => Ok(vec![SkillResource {
                name: "yolo/summary.md".to_owned(),
                media_type: "text/markdown".to_owned(),
                content: "YOLO produces DetectionSet only. Configure class_mapping, confidence_threshold and nms_iou_threshold. Compose core.filter then core.crop for Detect & Crop.".to_owned(),
            }]),
            Some(other) => Err(CoreError::Validation(format!(
                "unknown YOLO resource {other:?}"
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

fn detection_template() -> WorkflowTemplate {
    let node = |id: &str, node_type: &str, kind, inputs, outputs| WorkflowDraftNode {
        id: id.to_owned(),
        node_type: node_type.to_owned(),
        kind,
        inputs,
        outputs,
        required_skills: vec![YOLO_SKILL_ID.to_owned()],
        ..WorkflowDraftNode::default()
    };
    WorkflowTemplate {
        id: "yolo.detection".to_owned(),
        name: "YOLO Detection".to_owned(),
        description: "Image → YOLO → Filter → Confidence Gate → Commit".to_owned(),
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
                YOLO_DETECTION_OPERATION,
                WorkflowNodeKind::VisionModel,
                vec![port("image", ArtifactKind::Image)],
                vec![port("detections", ArtifactKind::DetectionSet)],
            ),
            node(
                "filter",
                "core.filter",
                WorkflowNodeKind::Transform,
                vec![port("detections", ArtifactKind::DetectionSet)],
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
            edge("image", "image", "detector", "image", None),
            edge("detector", "detections", "filter", "detections", None),
            edge("filter", "detections", "gate", "detections", None),
            edge("gate", "detections", "commit", "detections", Some("pass")),
        ],
        resource_versions: BTreeMap::from([(
            "yolo/summary.md".to_owned(),
            YOLO_SKILL_VERSION.to_owned(),
        )]),
        allow_unvalidated_commit: false,
    }
}

fn edge(from: &str, from_port: &str, to: &str, to_port: &str, route: Option<&str>) -> WorkflowEdge {
    WorkflowEdge {
        from_node: from.to_owned(),
        from_port: from_port.to_owned(),
        to_node: to.to_owned(),
        to_port: to_port.to_owned(),
        route: route.map(ToOwned::to_owned),
    }
}

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
        let mut response = self
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
        postprocess_detection_sets(&mut response.artifacts, &context.node.parameters)?;
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

fn postprocess_detection_sets(
    artifacts: &mut [PipelineArtifact],
    parameters: &BTreeMap<String, serde_json::Value>,
) -> Result<(), DagNodeFailure> {
    let threshold = parameters
        .get("confidence_threshold")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0) as f32;
    let nms_threshold = parameters
        .get("nms_iou_threshold")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(1.0) as f32;
    if !(0.0..=1.0).contains(&threshold) || !(0.0..=1.0).contains(&nms_threshold) {
        return Err(DagNodeFailure::terminal(
            "invalid_detection_parameters",
            "confidence_threshold and nms_iou_threshold must be within [0,1]",
        ));
    }
    let mapping = parameters
        .get("class_mapping")
        .and_then(serde_json::Value::as_object);
    for artifact in artifacts {
        let PipelineArtifact::DetectionSet(set) = artifact else {
            continue;
        };
        // Missing or non-comparable scores must survive confidence post-processing so a later
        // Evidence Gate or Human Review can decide their disposition.
        set.detections.retain(|detection| {
            detection
                .score
                .comparable_confidence()
                .is_none_or(|confidence| confidence >= threshold)
        });
        for detection in &mut set.detections {
            if let Some(label) = mapping
                .and_then(|mapping| {
                    mapping.get(detection.model_label.as_deref().unwrap_or_default())
                })
                .and_then(serde_json::Value::as_str)
            {
                detection.project_label = Some(LabelId::from(label));
            }
        }
        set.detections.sort_by(|left, right| {
            match (
                left.score.comparable_confidence(),
                right.score.comparable_confidence(),
            ) {
                (Some(left), Some(right)) => right.total_cmp(&left),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then_with(|| left.detection_id.cmp(&right.detection_id))
        });
        let mut kept = Vec::<Detection>::new();
        for candidate in set.detections.drain(..) {
            let suppressed = kept.iter().any(|existing| {
                existing.model_label == candidate.model_label
                    && intersection_over_union(existing.bbox, candidate.bbox) > nms_threshold
            });
            if !suppressed {
                kept.push(candidate);
            }
        }
        set.detections = kept;
        set.validate()
            .map_err(|error| DagNodeFailure::terminal("invalid_detection_output", error))?;
    }
    Ok(())
}

fn intersection_over_union(left: NormalizedRect, right: NormalizedRect) -> f32 {
    let intersection = left.intersection_area(right);
    let union = left.area() + right.area() - intersection;
    if union <= f32::EPSILON {
        0.0
    } else {
        intersection / union
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
        let artifact_id = format!("detection-set:{}", request.request_id);
        let detections = (0..count)
            .map(|index| {
                let offset = (index as f32 * 0.03).min(0.4);
                Detection::from_source(
                    format!("detection-{index}"),
                    None,
                    Some(class_id.to_owned()),
                    label.clone(),
                    NormalizedRect::new(0.1 + offset, 0.1 + offset, 0.25, 0.25)?,
                    DetectionScore::relative(confidence).map_err(CoreError::Validation)?,
                    DetectionSource {
                        model_id: request.model_id.clone(),
                        capability: VisionCapability::ObjectDetection,
                        artifact_id: artifact_id.clone(),
                    },
                )
                .map_err(CoreError::Validation)
            })
            .collect::<CoreResult<Vec<_>>>()?;
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

#[cfg(test)]
mod tests {
    use annotagent_core::{SkillKind, SkillResourceRequest};

    use super::*;

    #[test]
    fn capability_manifest_and_template_do_not_claim_crop() {
        let skill = YoloCapabilitySkill::default();
        assert_eq!(skill.manifest().kind, SkillKind::Capability);
        assert!(skill.manifest().validate().is_empty());
        let serialized = serde_json::to_string(&skill.workflow_templates()).expect("templates");
        assert!(!serialized.contains("core.crop"));
        assert!(
            skill
                .resources(&SkillResourceRequest {
                    task_id: None,
                    resource_name: Some("yolo/summary.md".to_owned()),
                })
                .expect("resource")[0]
                .content
                .contains("core.crop")
        );
    }
}
