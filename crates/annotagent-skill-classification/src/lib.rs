//! Formal Classification Skill for whole-image and crop subjects.

use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ArtifactValidationState, Classification, ClassificationSetArtifact,
    CoreError, CoreResult, LabelId, ModelImage, NodePort, PIPELINE_VISION_PROTOCOL_VERSION,
    PipelineArtifact, PipelineInferenceRequest, PipelineInferenceResponse, PipelineModelBackend,
    Skill, SkillKind, SkillManifest, SkillResource, SkillResourceRequest, TaskId, TaskTemplate,
    VisionCapability, VisionNodeDescriptor, WorkflowDraftNode, WorkflowEdge, WorkflowNodeKind,
    WorkflowTemplate,
};
use annotagent_runtime::{DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner};
use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio_util::sync::CancellationToken;

pub const CLASSIFICATION_SKILL_ID: &str = "classification";
pub const CLASSIFICATION_SKILL_VERSION: &str = "1";
pub const CLASSIFICATION_OPERATION: &str = "classification.classify";
pub const CLASSIFICATION_VERIFY_OPERATION: &str = "classification.verify";

pub struct ClassificationCapabilitySkill {
    manifest: SkillManifest,
}

impl Default for ClassificationCapabilitySkill {
    fn default() -> Self {
        Self {
            manifest: SkillManifest {
                version: 1,
                id: CLASSIFICATION_SKILL_ID.to_owned(),
                kind: SkillKind::Capability,
                skill_version: CLASSIFICATION_SKILL_VERSION.to_owned(),
                display_name: "Classification".to_owned(),
                description: "Classify whole images, crops, candidates or attributes".to_owned(),
                rust_implementation: Some(
                    "annotagent_skill_classification::ClassificationCapabilitySkill".to_owned(),
                ),
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                capabilities: vec!["classification".to_owned()],
                nodes: vec![
                    CLASSIFICATION_OPERATION.to_owned(),
                    CLASSIFICATION_VERIFY_OPERATION.to_owned(),
                ],
                tools: Vec::new(),
                validators: vec![CLASSIFICATION_VERIFY_OPERATION.to_owned()],
                policies: Vec::new(),
                templates: vec!["classification.whole-image".to_owned()],
                summary_resources: vec!["classification/summary.md".to_owned()],
                task_resources: BTreeMap::new(),
                correction_taxonomy: Vec::new(),
                visual_profile: BTreeMap::new(),
            },
        }
    }
}

impl Skill for ClassificationCapabilitySkill {
    fn id(&self) -> &str {
        CLASSIFICATION_SKILL_ID
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn node_templates(&self) -> Vec<TaskTemplate> {
        vec![
            TaskTemplate {
                id: TaskId::from(CLASSIFICATION_OPERATION),
                description: "Classify an Image or every subject in a CropSet".to_owned(),
            },
            TaskTemplate {
                id: TaskId::from(CLASSIFICATION_VERIFY_OPERATION),
                description: "Verify labels and confidence without another model call".to_owned(),
            },
        ]
    }

    fn workflow_templates(&self) -> Vec<WorkflowTemplate> {
        vec![whole_image_template()]
    }

    fn resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        match request.resource_name.as_deref() {
            None | Some("classification/summary.md") => Ok(vec![SkillResource {
                name: "classification/summary.md".to_owned(),
                media_type: "text/markdown".to_owned(),
                content: "Bind a registry model, declare allowed labels, and preserve every subject Artifact reference. Use multi_label only when the Project schema permits it.".to_owned(),
            }]),
            Some(other) => Err(CoreError::Validation(format!(
                "unknown Classification resource {other:?}"
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

fn whole_image_template() -> WorkflowTemplate {
    let node = |id: &str, node_type: &str, kind, inputs, outputs| WorkflowDraftNode {
        id: id.to_owned(),
        node_type: node_type.to_owned(),
        kind,
        inputs,
        outputs,
        required_skills: vec![CLASSIFICATION_SKILL_ID.to_owned()],
        ..WorkflowDraftNode::default()
    };
    WorkflowTemplate {
        id: "classification.whole-image".to_owned(),
        name: "Whole-image Classification".to_owned(),
        description: "Image → Classifier → Classification Verifier → Commit".to_owned(),
        nodes: vec![
            node(
                "image",
                "core.image_input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("image", ArtifactKind::Image)],
            ),
            node(
                "classifier",
                CLASSIFICATION_OPERATION,
                WorkflowNodeKind::VisionModel,
                vec![port("image", ArtifactKind::Image)],
                vec![port("classifications", ArtifactKind::ClassificationSet)],
            ),
            node(
                "verify",
                CLASSIFICATION_VERIFY_OPERATION,
                WorkflowNodeKind::Validator,
                vec![port("classifications", ArtifactKind::ClassificationSet)],
                vec![port("classifications", ArtifactKind::ClassificationSet)],
            ),
            node(
                "commit",
                "core.commit",
                WorkflowNodeKind::Commit,
                vec![port("classifications", ArtifactKind::ClassificationSet)],
                Vec::new(),
            ),
        ],
        edges: vec![
            WorkflowEdge {
                from_node: "image".to_owned(),
                from_port: "image".to_owned(),
                to_node: "classifier".to_owned(),
                to_port: "image".to_owned(),
                route: None,
            },
            WorkflowEdge {
                from_node: "classifier".to_owned(),
                from_port: "classifications".to_owned(),
                to_node: "verify".to_owned(),
                to_port: "classifications".to_owned(),
                route: None,
            },
            WorkflowEdge {
                from_node: "verify".to_owned(),
                from_port: "classifications".to_owned(),
                to_node: "commit".to_owned(),
                to_port: "classifications".to_owned(),
                route: None,
            },
        ],
        resource_versions: BTreeMap::from([(
            "classification/summary.md".to_owned(),
            CLASSIFICATION_SKILL_VERSION.to_owned(),
        )]),
        allow_unvalidated_commit: false,
    }
}

#[must_use]
pub fn node_descriptor() -> VisionNodeDescriptor {
    VisionNodeDescriptor {
        id: CLASSIFICATION_OPERATION.to_owned(),
        display_name: "Classification".to_owned(),
        required_capabilities: vec![VisionCapability::Classification],
        accepts: vec![ArtifactKind::Image, ArtifactKind::CropSet],
        produces: vec![ArtifactKind::ClassificationSet],
        deterministic: false,
    }
}

#[must_use]
pub fn verifier_node_descriptor() -> VisionNodeDescriptor {
    VisionNodeDescriptor {
        id: CLASSIFICATION_VERIFY_OPERATION.to_owned(),
        display_name: "Classification Verifier".to_owned(),
        required_capabilities: Vec::new(),
        accepts: vec![ArtifactKind::ClassificationSet],
        produces: vec![ArtifactKind::ClassificationSet],
        deterministic: true,
    }
}

pub struct ClassificationVerifierRunner;

#[async_trait]
impl DagNodeRunner for ClassificationVerifierRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        let allowed = context
            .node
            .parameters
            .get("labels")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(LabelId::from)
            .collect::<Vec<_>>();
        let minimum = context
            .node
            .parameters
            .get("minimum_confidence")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;
        let mut outputs = Vec::new();
        for input in context.input_pipeline_artifacts {
            let PipelineArtifact::ClassificationSet(mut set) = input else {
                continue;
            };
            if !allowed.is_empty()
                && set
                    .classifications
                    .iter()
                    .any(|classification| !allowed.contains(&classification.label))
            {
                return Err(DagNodeFailure::terminal(
                    "classification_label_not_allowed",
                    "ClassificationSet contains a label outside the verifier allow-list",
                ));
            }
            set.validation_state = if set
                .classifications
                .iter()
                .all(|classification| classification.confidence >= minimum)
            {
                ArtifactValidationState::Valid
            } else {
                ArtifactValidationState::NeedsReview
            };
            set.reference.source_node.clone_from(&context.node.id);
            "classifications".clone_into(&mut set.reference.port);
            set.validate()
                .map_err(|error| DagNodeFailure::terminal("invalid_classification_set", error))?;
            outputs.push(PipelineArtifact::ClassificationSet(set));
        }
        if outputs.is_empty() {
            return Err(DagNodeFailure::terminal(
                "classification_input_missing",
                "Classification Verifier requires a ClassificationSet",
            ));
        }
        Ok(DagNodeOutput {
            pipeline_artifacts: outputs,
            ..DagNodeOutput::default()
        })
    }
}

pub struct ClassificationSkillRunner {
    backend: Arc<dyn PipelineModelBackend>,
    model_id: String,
    image: Option<ModelImage>,
}

impl ClassificationSkillRunner {
    pub fn new(
        backend: Arc<dyn PipelineModelBackend>,
        model_id: impl Into<String>,
        image: Option<ModelImage>,
    ) -> CoreResult<Self> {
        if backend.capability() != VisionCapability::Classification {
            return Err(CoreError::Validation(
                "Classification Skill requires a Classification backend".to_owned(),
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
impl DagNodeRunner for ClassificationSkillRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        if context.node.node_type != CLASSIFICATION_OPERATION {
            return Err(DagNodeFailure::terminal(
                "wrong_skill_operation",
                "Classification runner received another operation",
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
                    operation: VisionCapability::Classification,
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
                DagNodeFailure::retryable("classification_backend", error.to_string())
            })?;
        if let Some(error) = response.error {
            return Err(DagNodeFailure {
                code: error.code,
                summary: error.message,
                retryable: error.retryable,
            });
        }
        if response.artifacts.is_empty()
            || response.artifacts.iter().any(|artifact| {
                !matches!(artifact, PipelineArtifact::ClassificationSet(_))
                    || artifact.image_id() != context.image_id
                    || artifact.reference().source_node != context.node.id
            })
        {
            return Err(DagNodeFailure::terminal(
                "invalid_classification_output",
                "Classification backend must return scoped ClassificationSet Artifacts",
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
pub struct MockClassificationBackend {
    id: String,
}

impl MockClassificationBackend {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl PipelineModelBackend for MockClassificationBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn capability(&self) -> VisionCapability {
        VisionCapability::Classification
    }

    async fn infer_pipeline(
        &self,
        request: PipelineInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<PipelineInferenceResponse> {
        if cancellation.is_cancelled() {
            return Err(CoreError::Provider(
                "mock Classification inference cancelled".to_owned(),
            ));
        }
        let labels = request
            .parameters
            .get("labels")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(LabelId::from)
            .collect::<Vec<_>>();
        let label = request
            .parameters
            .get("mock_label")
            .and_then(serde_json::Value::as_str)
            .map(LabelId::from)
            .or_else(|| labels.first().cloned())
            .ok_or_else(|| {
                CoreError::Validation(
                    "mock Classification requires labels or mock_label".to_owned(),
                )
            })?;
        let confidence = request
            .parameters
            .get("mock_confidence")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.95) as f32;
        let subjects = subjects(&request.input_artifacts);
        if subjects.is_empty() {
            return Err(CoreError::Validation(
                "Classification requires Image or CropSet input".to_owned(),
            ));
        }
        let multi_label = request
            .parameters
            .get("multi_label")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let selected_labels = if multi_label {
            let configured = request
                .parameters
                .get("mock_labels")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .map(LabelId::from)
                .filter(|candidate| labels.contains(candidate))
                .collect::<Vec<_>>();
            if configured.is_empty() {
                vec![label.clone()]
            } else {
                configured
            }
        } else {
            vec![label.clone()]
        };
        let scores = selected_labels
            .iter()
            .cloned()
            .map(|label| (label, confidence))
            .collect::<BTreeMap<_, _>>();
        let classifications = subjects
            .into_iter()
            .enumerate()
            .flat_map(|(subject_index, (subject, parent))| {
                selected_labels
                    .iter()
                    .enumerate()
                    .map(|(label_index, selected)| Classification {
                        id: format!("classification-{subject_index}-{label_index}"),
                        subject: subject.clone(),
                        parent: parent.clone(),
                        label: selected.clone(),
                        confidence,
                        scores: scores.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let artifact = ClassificationSetArtifact {
            reference: ArtifactRef {
                artifact_id: format!("classification-set:{}", request.request_id),
                source_node: request.node_id,
                port: "classifications".to_owned(),
                artifact_type: ArtifactKind::ClassificationSet,
                item_id: None,
            },
            image_id: request.image_id,
            model_binding: request.model_id,
            validation_state: annotagent_core::ArtifactValidationState::Valid,
            classifications,
        };
        artifact.validate().map_err(CoreError::Validation)?;
        Ok(PipelineInferenceResponse {
            request_id: Some(request.request_id),
            model_identity: Some(self.id.clone()),
            artifacts: vec![PipelineArtifact::ClassificationSet(artifact)],
            metadata: BTreeMap::from([("mode".to_owned(), serde_json::json!("mock"))]),
            ..PipelineInferenceResponse::default()
        })
    }
}

fn subjects(inputs: &[PipelineArtifact]) -> Vec<(ArtifactRef, Option<ArtifactRef>)> {
    let mut subjects = Vec::new();
    for artifact in inputs {
        match artifact {
            PipelineArtifact::Image(image) => {
                subjects.push((image.reference.clone(), None));
            }
            PipelineArtifact::CropSet(crops) => {
                subjects.extend(
                    crops
                        .crops
                        .iter()
                        .map(|crop| (crops.reference.item(&crop.id), Some(crop.parent.clone()))),
                );
            }
            _ => {}
        }
    }
    subjects
}

#[cfg(test)]
mod tests {
    use annotagent_core::{ImageArtifact, ImageId, ProjectId, RunId, SkillKind, WorkflowDraftNode};
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn image(image_id: ImageId) -> PipelineArtifact {
        PipelineArtifact::Image(ImageArtifact {
            reference: ArtifactRef {
                artifact_id: format!("image:{image_id}"),
                source_node: "image".to_owned(),
                port: "image".to_owned(),
                artifact_type: ArtifactKind::Image,
                item_id: None,
            },
            image_id,
            width: 32,
            height: 24,
            mime_type: "image/png".to_owned(),
            blob_ref: "workspace://fixture".to_owned(),
        })
    }

    #[test]
    fn capability_manifest_and_template_are_registry_bound() {
        let skill = ClassificationCapabilitySkill::default();
        assert_eq!(skill.manifest().kind, SkillKind::Capability);
        assert!(skill.manifest().validate().is_empty());
        let template = &skill.workflow_templates()[0];
        assert!(
            template
                .nodes
                .iter()
                .any(|node| node.node_type == CLASSIFICATION_OPERATION)
        );
        assert!(
            template
                .nodes
                .iter()
                .any(|node| node.node_type == CLASSIFICATION_VERIFY_OPERATION)
        );
    }

    #[tokio::test]
    async fn mock_supports_single_and_multi_label_whole_image_classification() {
        let backend = MockClassificationBackend::new("mock");
        let image_id = ImageId::new();
        let request = |parameters| PipelineInferenceRequest {
            protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
            request_id: uuid::Uuid::new_v4().to_string(),
            run_id: RunId::new(),
            image_id,
            node_id: "classifier".to_owned(),
            model_id: "mock".to_owned(),
            operation: VisionCapability::Classification,
            image: None,
            input_artifacts: vec![image(image_id)],
            parameters,
            timeout_ms: Some(1_000),
        };
        let single = backend
            .infer_pipeline(
                request(BTreeMap::from([(
                    "labels".to_owned(),
                    serde_json::json!(["a", "b"]),
                )])),
                CancellationToken::new(),
            )
            .await
            .expect("single label");
        let PipelineArtifact::ClassificationSet(single) = &single.artifacts[0] else {
            panic!("ClassificationSet")
        };
        assert_eq!(single.classifications.len(), 1);

        let multi = backend
            .infer_pipeline(
                request(BTreeMap::from([
                    ("labels".to_owned(), serde_json::json!(["a", "b"])),
                    ("multi_label".to_owned(), serde_json::json!(true)),
                    ("mock_labels".to_owned(), serde_json::json!(["a", "b"])),
                ])),
                CancellationToken::new(),
            )
            .await
            .expect("multi label");
        let PipelineArtifact::ClassificationSet(multi) = &multi.artifacts[0] else {
            panic!("ClassificationSet")
        };
        assert_eq!(multi.classifications.len(), 2);
        assert!(
            multi
                .classifications
                .iter()
                .all(|item| item.subject.artifact_id.starts_with("image:"))
        );
    }

    #[tokio::test]
    async fn verifier_routes_low_confidence_to_review_without_losing_subject() {
        let image_id = ImageId::new();
        let subject = image(image_id).reference().clone();
        let input = PipelineArtifact::ClassificationSet(ClassificationSetArtifact {
            reference: ArtifactRef {
                artifact_id: "classifications".to_owned(),
                source_node: "classifier".to_owned(),
                port: "classifications".to_owned(),
                artifact_type: ArtifactKind::ClassificationSet,
                item_id: None,
            },
            image_id,
            model_binding: "mock".to_owned(),
            validation_state: ArtifactValidationState::Unvalidated,
            classifications: vec![Classification {
                id: "c1".to_owned(),
                subject: subject.clone(),
                parent: None,
                label: LabelId::from("a"),
                confidence: 0.4,
                scores: BTreeMap::from([(LabelId::from("a"), 0.4)]),
            }],
        });
        let node = WorkflowDraftNode {
            id: "verify".to_owned(),
            node_type: CLASSIFICATION_VERIFY_OPERATION.to_owned(),
            parameters: BTreeMap::from([
                ("labels".to_owned(), serde_json::json!(["a"])),
                ("minimum_confidence".to_owned(), serde_json::json!(0.8)),
            ]),
            ..WorkflowDraftNode::default()
        };
        let output = ClassificationVerifierRunner
            .run(DagNodeContext {
                project_id: ProjectId::new(),
                run_id: RunId::new(),
                image_id,
                node: &node,
                input_artifacts: Vec::new(),
                input_pipeline_artifacts: vec![input],
                cancellation: CancellationToken::new(),
            })
            .await
            .expect("verification");
        let PipelineArtifact::ClassificationSet(set) = &output.pipeline_artifacts[0] else {
            panic!("ClassificationSet")
        };
        assert_eq!(set.validation_state, ArtifactValidationState::NeedsReview);
        assert_eq!(set.classifications[0].subject, subject);
    }
}
