//! Backend-neutral Object Detection Capability Skill.
//!
//! The Skill owns target-label mapping and bounded post-processing. Concrete detector brands,
//! HTTP transport, cropping, review, and commit remain separate Registry/Core concerns.

use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    ArtifactKind, ArtifactRef, CoreError, CoreResult, DETECTION_ARTIFACT_SCHEMA_VERSION, Detection,
    DetectionScore, DetectionSetArtifact, DetectionSource, ImageId, LabelId, ModelImage, NodePort,
    NormalizedRect, PIPELINE_VISION_PROTOCOL_VERSION, PipelineArtifact, PipelineInferenceRequest,
    PipelineInferenceResponse, PipelineModelBackend, Skill, SkillKind, SkillManifest,
    SkillResource, SkillResourceRequest, TaskId, TaskTemplate, VisionCapability,
    VisionNodeDescriptor, WorkflowDraftNode, WorkflowEdge, WorkflowNodeKind, WorkflowTemplate,
};
use annotagent_runtime::{DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner};
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

pub const OBJECT_DETECTION_SKILL_ID: &str = "annotagent.object_detection";
pub const OBJECT_DETECTION_SKILL_VERSION: &str = "1";
pub const OBJECT_DETECTION_OPERATION: &str = "object_detection.detect";
pub type ModelBindingId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageArtifactRef {
    pub reference: ArtifactRef,
    pub image_id: ImageId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectDetectionRequest {
    pub image: ImageArtifactRef,
    pub model_binding: ModelBindingId,
    pub target_labels: Vec<LabelId>,
    #[serde(default)]
    pub options: DetectionOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DetectionOptions {
    pub confidence_threshold: Option<f32>,
    pub iou_threshold: Option<f32>,
    pub max_detections: Option<u32>,
    #[serde(default)]
    pub class_mapping: BTreeMap<String, LabelId>,
}

impl DetectionOptions {
    pub fn validate(&self, target_labels: &[LabelId]) -> CoreResult<()> {
        for (name, value) in [
            ("confidence_threshold", self.confidence_threshold),
            ("iou_threshold", self.iou_threshold),
        ] {
            if value.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
                return Err(CoreError::Validation(format!(
                    "Object Detection {name} must be finite and within [0,1]"
                )));
            }
        }
        if self.max_detections == Some(0) || self.max_detections.is_some_and(|value| value > 10_000)
        {
            return Err(CoreError::Validation(
                "Object Detection max_detections must be within 1..=10000".to_owned(),
            ));
        }
        let target_set = target_labels
            .iter()
            .map(LabelId::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        if target_set.len() != target_labels.len()
            || target_set.contains("")
            || self
                .class_mapping
                .iter()
                .any(|(model_label, project_label)| {
                    model_label.trim().is_empty()
                        || project_label.as_str().trim().is_empty()
                        || !target_set.is_empty() && !target_set.contains(project_label.as_str())
                })
        {
            return Err(CoreError::Validation(
                "Object Detection target labels and class mapping must be unique, non-empty, and Project-scoped"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

#[must_use]
pub fn object_detection_parameter_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "required": ["target_labels", "class_mapping"],
        "properties": {
            "target_labels": {
                "type": "array",
                "minItems": 1,
                "maxItems": 10000,
                "uniqueItems": true,
                "items": {"type": "string", "minLength": 1}
            },
            "confidence_threshold": {"type": "number", "minimum": 0, "maximum": 1},
            "iou_threshold": {"type": "number", "minimum": 0, "maximum": 1},
            "max_detections": {"type": "integer", "minimum": 1, "maximum": 10000},
            "class_mapping": {
                "type": "object",
                "additionalProperties": {"type": "string", "minLength": 1}
            },
            "mock_empty": {"type": "boolean"},
            "mock_count": {"type": "integer", "minimum": 0, "maximum": 100},
            "mock_model_label": {"type": "string", "minLength": 1},
            "mock_confidence": {"type": "number", "minimum": 0, "maximum": 1}
        }
    })
}

pub struct ObjectDetectionCapabilitySkill {
    manifest: SkillManifest,
}

impl Default for ObjectDetectionCapabilitySkill {
    fn default() -> Self {
        Self {
            manifest: SkillManifest {
                version: 1,
                id: OBJECT_DETECTION_SKILL_ID.to_owned(),
                kind: SkillKind::Capability,
                skill_version: OBJECT_DETECTION_SKILL_VERSION.to_owned(),
                display_name: "Object detection".to_owned(),
                description:
                    "Run a registered trained detector and map model classes to Project Labels"
                        .to_owned(),
                rust_implementation: Some(
                    "annotagent_skill_object_detection::ObjectDetectionCapabilitySkill".to_owned(),
                ),
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                capabilities: vec!["object_detection".to_owned()],
                nodes: vec![OBJECT_DETECTION_OPERATION.to_owned()],
                tools: Vec::new(),
                validators: Vec::new(),
                policies: Vec::new(),
                templates: vec!["object-detection.specialist-review".to_owned()],
                summary_resources: vec![
                    "object-detection/summary.md".to_owned(),
                    "object-detection/node.schema.json".to_owned(),
                ],
                task_resources: BTreeMap::new(),
                correction_taxonomy: Vec::new(),
                visual_profile: BTreeMap::new(),
            },
        }
    }
}

impl Skill for ObjectDetectionCapabilitySkill {
    fn id(&self) -> &str {
        OBJECT_DETECTION_SKILL_ID
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn node_templates(&self) -> Vec<TaskTemplate> {
        vec![TaskTemplate {
            id: TaskId::from(OBJECT_DETECTION_OPERATION),
            description: "Image + registered detector → mapped DetectionSet".to_owned(),
        }]
    }

    fn workflow_templates(&self) -> Vec<WorkflowTemplate> {
        vec![specialist_review_template()]
    }

    fn resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        match request.resource_name.as_deref() {
            None => Ok(vec![summary_resource(), schema_resource()]),
            Some("object-detection/summary.md") => Ok(vec![summary_resource()]),
            Some("object-detection/node.schema.json") => Ok(vec![schema_resource()]),
            Some(other) => Err(CoreError::Validation(format!(
                "unknown Object Detection resource {other:?}"
            ))),
        }
    }
}

fn summary_resource() -> SkillResource {
    SkillResource {
        name: "object-detection/summary.md".to_owned(),
        media_type: "text/markdown".to_owned(),
        content: "Bind any ObjectDetection model, map its model-native classes to Project Labels, and produce a DetectionSet. The Skill does not crop pixels and detector brands remain Model Registry data."
            .to_owned(),
    }
}

fn schema_resource() -> SkillResource {
    SkillResource {
        name: "object-detection/node.schema.json".to_owned(),
        media_type: "application/schema+json".to_owned(),
        content: serde_json::to_string_pretty(&object_detection_parameter_schema())
            .expect("static Object Detection schema serializes"),
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

fn node(
    id: &str,
    node_type: &str,
    kind: WorkflowNodeKind,
    inputs: Vec<NodePort>,
    outputs: Vec<NodePort>,
) -> WorkflowDraftNode {
    WorkflowDraftNode {
        id: id.to_owned(),
        node_type: node_type.to_owned(),
        kind,
        inputs,
        outputs,
        required_skills: vec![OBJECT_DETECTION_SKILL_ID.to_owned()],
        ..WorkflowDraftNode::default()
    }
}

fn edge(from: &str, from_port: &str, to: &str, to_port: &str) -> WorkflowEdge {
    WorkflowEdge {
        from_node: from.to_owned(),
        from_port: from_port.to_owned(),
        to_node: to.to_owned(),
        to_port: to_port.to_owned(),
        route: None,
    }
}

fn specialist_review_template() -> WorkflowTemplate {
    let mut detector = node(
        "detector",
        OBJECT_DETECTION_OPERATION,
        WorkflowNodeKind::VisionModel,
        vec![port("image", ArtifactKind::Image)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    detector.model_binding = Some("mock-object-detector".to_owned());
    detector.parameters = BTreeMap::from([
        ("target_labels".to_owned(), serde_json::json!(["target"])),
        (
            "class_mapping".to_owned(),
            serde_json::json!({"target": "target"}),
        ),
        ("confidence_threshold".to_owned(), serde_json::json!(0.25)),
        ("iou_threshold".to_owned(), serde_json::json!(0.7)),
        ("max_detections".to_owned(), serde_json::json!(100)),
    ]);
    WorkflowTemplate {
        id: "object-detection.specialist-review".to_owned(),
        name: "Trained detector with review".to_owned(),
        description: "Image → trained detector → Human Review → Commit".to_owned(),
        nodes: vec![
            node(
                "image",
                "core.image_input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("image", ArtifactKind::Image)],
            ),
            detector,
            node(
                "review",
                "review_gate",
                WorkflowNodeKind::HumanReview,
                vec![port("detections", ArtifactKind::DetectionSet)],
                vec![port("detections", ArtifactKind::DetectionSet)],
            ),
            node(
                "commit",
                "commit",
                WorkflowNodeKind::Commit,
                vec![port("detections", ArtifactKind::DetectionSet)],
                Vec::new(),
            ),
        ],
        edges: vec![
            edge("image", "image", "detector", "image"),
            edge("detector", "detections", "review", "detections"),
            edge("review", "detections", "commit", "detections"),
        ],
        resource_versions: BTreeMap::from([
            (
                "object-detection/summary.md".to_owned(),
                OBJECT_DETECTION_SKILL_VERSION.to_owned(),
            ),
            (
                "object-detection/node.schema.json".to_owned(),
                OBJECT_DETECTION_SKILL_VERSION.to_owned(),
            ),
        ]),
        allow_unvalidated_commit: false,
    }
}

#[must_use]
pub fn node_descriptor() -> VisionNodeDescriptor {
    VisionNodeDescriptor {
        id: OBJECT_DETECTION_OPERATION.to_owned(),
        display_name: "Object Detection".to_owned(),
        required_capabilities: vec![VisionCapability::ObjectDetection],
        accepts: vec![ArtifactKind::Image],
        produces: vec![ArtifactKind::DetectionSet],
        deterministic: false,
    }
}

pub struct ObjectDetectionSkillRunner {
    backend: Arc<dyn PipelineModelBackend>,
    model_id: String,
    image: Option<ModelImage>,
}

impl ObjectDetectionSkillRunner {
    pub fn new(
        backend: Arc<dyn PipelineModelBackend>,
        model_id: impl Into<String>,
        image: Option<ModelImage>,
    ) -> CoreResult<Self> {
        if backend.capability() != VisionCapability::ObjectDetection {
            return Err(CoreError::Validation(
                "Object Detection Skill requires an ObjectDetection backend".to_owned(),
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
impl DagNodeRunner for ObjectDetectionSkillRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        if context.node.node_type != OBJECT_DETECTION_OPERATION {
            return Err(DagNodeFailure::terminal(
                "wrong_skill_operation",
                "Object Detection runner received another operation",
            ));
        }
        if !context
            .input_pipeline_artifacts
            .iter()
            .any(|artifact| matches!(artifact, PipelineArtifact::Image(_)))
        {
            return Err(DagNodeFailure::terminal(
                "missing_image_input",
                "Object Detection requires Image input",
            ));
        }
        let (target_labels, options) = parse_parameters(&context.node.parameters)?;
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
        postprocess_detection_sets(&mut response.artifacts, &target_labels, &options)?;
        if response.artifacts.len() != 1
            || response.artifacts.iter().any(|artifact| {
                !matches!(artifact, PipelineArtifact::DetectionSet(_))
                    || artifact.image_id() != context.image_id
                    || artifact.reference().source_node != context.node.id
            })
        {
            return Err(DagNodeFailure::terminal(
                "invalid_detection_output",
                "Object Detection backend must return exactly one scoped DetectionSet",
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

fn parse_parameters(
    parameters: &BTreeMap<String, serde_json::Value>,
) -> Result<(Vec<LabelId>, DetectionOptions), DagNodeFailure> {
    let target_labels = parameters
        .get("target_labels")
        .or_else(|| parameters.get("labels"))
        .map_or_else(
            || Ok(Vec::new()),
            |value| serde_json::from_value::<Vec<LabelId>>(value.clone()),
        )
        .map_err(|_| {
            DagNodeFailure::terminal(
                "invalid_detection_parameters",
                "target_labels must be an array of Project Label ids",
            )
        })?;
    let options = DetectionOptions {
        confidence_threshold: parameter_f32(parameters, "confidence_threshold")?,
        iou_threshold: parameter_f32(parameters, "iou_threshold")?,
        max_detections: parameters.get("max_detections").map_or(Ok(None), |value| {
            value
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .map(Some)
                .ok_or_else(|| {
                    DagNodeFailure::terminal(
                        "invalid_detection_parameters",
                        "max_detections must be an unsigned 32-bit integer",
                    )
                })
        })?,
        class_mapping: parameters
            .get("class_mapping")
            .map_or_else(
                || Ok(BTreeMap::new()),
                |value| serde_json::from_value(value.clone()),
            )
            .map_err(|_| {
                DagNodeFailure::terminal(
                    "invalid_detection_parameters",
                    "class_mapping must map model class strings to Project Label ids",
                )
            })?,
    };
    options.validate(&target_labels).map_err(|error| {
        DagNodeFailure::terminal("invalid_detection_parameters", error.to_string())
    })?;
    Ok((target_labels, options))
}

fn parameter_f32(
    parameters: &BTreeMap<String, serde_json::Value>,
    name: &str,
) -> Result<Option<f32>, DagNodeFailure> {
    parameters.get(name).map_or(Ok(None), |value| {
        value
            .as_f64()
            .map(|value| Some(value as f32))
            .ok_or_else(|| {
                DagNodeFailure::terminal(
                    "invalid_detection_parameters",
                    format!("{name} must be numeric"),
                )
            })
    })
}

fn postprocess_detection_sets(
    artifacts: &mut [PipelineArtifact],
    target_labels: &[LabelId],
    options: &DetectionOptions,
) -> Result<(), DagNodeFailure> {
    let requested = target_labels
        .iter()
        .map(LabelId::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    for artifact in artifacts {
        let PipelineArtifact::DetectionSet(set) = artifact else {
            continue;
        };
        for detection in &mut set.detections {
            if let Some(model_label) = detection.model_label.as_deref() {
                if let Some(mapped) = options.class_mapping.get(model_label) {
                    detection.project_label = Some(mapped.clone());
                } else if requested.contains(model_label) {
                    detection.project_label = Some(LabelId::from(model_label));
                }
            }
        }
        set.detections.retain(|detection| {
            let score_passes = options.confidence_threshold.is_none_or(|threshold| {
                detection
                    .score
                    .comparable_confidence()
                    .is_none_or(|score| score >= threshold)
            });
            let label_passes = requested.is_empty()
                || detection
                    .project_label
                    .as_ref()
                    .is_some_and(|label| requested.contains(label.as_str()));
            score_passes && label_passes
        });
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
        if let Some(threshold) = options.iou_threshold {
            let mut kept = Vec::<Detection>::new();
            for candidate in set.detections.drain(..) {
                let suppressed = kept.iter().any(|existing| {
                    existing.project_label == candidate.project_label
                        && intersection_over_union(existing.bbox, candidate.bbox) > threshold
                });
                if !suppressed {
                    kept.push(candidate);
                }
            }
            set.detections = kept;
        }
        if let Some(maximum) = options.max_detections {
            set.detections.truncate(maximum as usize);
        }
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
pub struct MockObjectDetectionBackend {
    id: String,
}

impl MockObjectDetectionBackend {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl PipelineModelBackend for MockObjectDetectionBackend {
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
                "mock Object Detection inference cancelled".to_owned(),
            ));
        }
        let count = if request
            .parameters
            .get("mock_empty")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            0
        } else {
            request
                .parameters
                .get("mock_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1)
                .min(100) as usize
        };
        let model_label = request
            .parameters
            .get("mock_model_label")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("target");
        let confidence = request
            .parameters
            .get("mock_confidence")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.9) as f32;
        let artifact_id = format!("detection-set:{}", request.request_id);
        let detections = (0..count)
            .map(|index| {
                let offset = (index as f32 * 0.02).min(0.5);
                Detection::from_source(
                    format!("specialist-{index}"),
                    None,
                    Some(model_label.to_owned()),
                    None,
                    NormalizedRect::new(0.1 + offset, 0.1 + offset, 0.2, 0.2)?,
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
    use super::*;
    use annotagent_core::{
        ImageArtifact, ModelImage, PipelineArtifact, ProjectId, RunId, SkillResourceRequest,
    };

    #[test]
    fn manifest_schema_and_template_are_backend_neutral() {
        let skill = ObjectDetectionCapabilitySkill::default();
        assert!(skill.manifest().validate().is_empty());
        let encoded = serde_json::to_string(&(
            skill.manifest(),
            skill.workflow_templates(),
            object_detection_parameter_schema(),
        ))
        .expect("Skill JSON");
        assert!(encoded.contains(OBJECT_DETECTION_SKILL_ID));
        assert!(!encoded.to_ascii_lowercase().contains("yolo"));
        assert!(!encoded.to_ascii_lowercase().contains("rfdetr"));
        assert!(!encoded.contains("core.crop"));
        assert!(
            skill
                .resources(&SkillResourceRequest {
                    task_id: None,
                    resource_name: Some("object-detection/node.schema.json".to_owned()),
                })
                .expect("schema")[0]
                .content
                .contains("class_mapping")
        );
    }

    #[tokio::test]
    async fn mock_runner_maps_filters_suppresses_limits_and_preserves_score() {
        let run_id = RunId::new();
        let image_id = ImageId::new();
        let mut node = specialist_review_template().nodes[1].clone();
        node.parameters.insert(
            "class_mapping".to_owned(),
            serde_json::json!({"football": "ball"}),
        );
        node.parameters
            .insert("target_labels".to_owned(), serde_json::json!(["ball"]));
        node.parameters
            .insert("mock_model_label".to_owned(), serde_json::json!("football"));
        node.parameters
            .insert("mock_count".to_owned(), serde_json::json!(3));
        node.parameters
            .insert("mock_confidence".to_owned(), serde_json::json!(0.87));
        node.parameters
            .insert("iou_threshold".to_owned(), serde_json::json!(0.5));
        node.parameters
            .insert("max_detections".to_owned(), serde_json::json!(2));
        let image = PipelineArtifact::Image(ImageArtifact {
            reference: ArtifactRef {
                artifact_id: "image:1".to_owned(),
                source_node: "image".to_owned(),
                port: "image".to_owned(),
                artifact_type: ArtifactKind::Image,
                item_id: None,
            },
            image_id,
            width: 100,
            height: 100,
            mime_type: "image/png".to_owned(),
            blob_ref: "workspace://fixture".to_owned(),
        });
        let runner = ObjectDetectionSkillRunner::new(
            Arc::new(MockObjectDetectionBackend::new("mock-specialist")),
            "mock-object-detector",
            Some(ModelImage {
                id: "fixture".to_owned(),
                mime_type: "image/png".to_owned(),
                data_base64: "eA==".to_owned(),
            }),
        )
        .expect("runner");
        let output = runner
            .run(DagNodeContext {
                project_id: ProjectId::new(),
                run_id,
                image_id,
                node: &node,
                input_artifacts: Vec::new(),
                input_pipeline_artifacts: vec![image],
                input_metadata: BTreeMap::new(),
                cancellation: CancellationToken::new(),
            })
            .await
            .expect("mapped detections");
        let PipelineArtifact::DetectionSet(set) = &output.pipeline_artifacts[0] else {
            panic!("expected DetectionSet")
        };
        assert!(!set.detections.is_empty());
        assert!(set.detections.len() <= 2);
        assert!(set.detections.iter().all(|detection| {
            detection.model_label.as_deref() == Some("football")
                && detection.project_label.as_ref().map(LabelId::as_str) == Some("ball")
                && detection.score.comparable_confidence() == Some(0.87)
        }));
    }

    #[tokio::test]
    async fn empty_detection_set_is_success_and_invalid_mapping_fails_closed() {
        let mut node = specialist_review_template().nodes[1].clone();
        node.parameters
            .insert("mock_empty".to_owned(), serde_json::json!(true));
        let image_id = ImageId::new();
        let image = PipelineArtifact::Image(ImageArtifact {
            reference: ArtifactRef {
                artifact_id: "image:empty".to_owned(),
                source_node: "image".to_owned(),
                port: "image".to_owned(),
                artifact_type: ArtifactKind::Image,
                item_id: None,
            },
            image_id,
            width: 1,
            height: 1,
            mime_type: "image/png".to_owned(),
            blob_ref: "workspace://empty".to_owned(),
        });
        let runner = ObjectDetectionSkillRunner::new(
            Arc::new(MockObjectDetectionBackend::new("mock-specialist")),
            "mock-object-detector",
            None,
        )
        .expect("runner");
        let output = runner
            .run(DagNodeContext {
                project_id: ProjectId::new(),
                run_id: RunId::new(),
                image_id,
                node: &node,
                input_artifacts: Vec::new(),
                input_pipeline_artifacts: vec![image.clone()],
                input_metadata: BTreeMap::new(),
                cancellation: CancellationToken::new(),
            })
            .await
            .expect("empty DetectionSet");
        let PipelineArtifact::DetectionSet(set) = &output.pipeline_artifacts[0] else {
            panic!("expected DetectionSet")
        };
        assert!(set.detections.is_empty());

        node.parameters.insert(
            "class_mapping".to_owned(),
            serde_json::json!({"target": "outside-project"}),
        );
        let error = runner
            .run(DagNodeContext {
                project_id: ProjectId::new(),
                run_id: RunId::new(),
                image_id,
                node: &node,
                input_artifacts: Vec::new(),
                input_pipeline_artifacts: vec![image],
                input_metadata: BTreeMap::new(),
                cancellation: CancellationToken::new(),
            })
            .await
            .expect_err("mapping outside target labels must fail");
        assert_eq!(error.code, "invalid_detection_parameters");
    }
}
