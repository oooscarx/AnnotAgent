//! Capability-driven open-vocabulary detection and phrase grounding.
//!
//! This Skill owns text-query validation and the Image -> `DetectionSet` operation. Concrete
//! model identities, HTTP transport, filtering, cropping, review, and commit remain outside it.

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

pub const OPEN_VOCABULARY_GROUNDING_SKILL_ID: &str = "annotagent.open_vocabulary_grounding";
pub const OPEN_VOCABULARY_GROUNDING_SKILL_VERSION: &str = "1";
pub const OPEN_VOCABULARY_DETECTION_OPERATION: &str = "open_vocabulary_grounding.detect";
pub const PHRASE_GROUNDING_OPERATION: &str = "open_vocabulary_grounding.ground_phrase";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageArtifactRef {
    pub reference: ArtifactRef,
    pub image_id: ImageId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingRequest {
    pub image: ImageArtifactRef,
    pub queries: Vec<GroundingQuery>,
    pub mode: GroundingMode,
    pub max_objects: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingQuery {
    pub id: String,
    pub text: String,
    pub target_label: Option<LabelId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundingMode {
    CategoryDetection,
    PhraseGrounding,
}

impl GroundingMode {
    #[must_use]
    pub const fn capability(self) -> VisionCapability {
        match self {
            Self::CategoryDetection => VisionCapability::OpenVocabularyDetection,
            Self::PhraseGrounding => VisionCapability::PhraseGrounding,
        }
    }
}

pub fn validate_grounding_queries(queries: &[GroundingQuery]) -> CoreResult<()> {
    if queries.is_empty() || queries.len() > 100 {
        return Err(CoreError::Validation(
            "grounding requires between 1 and 100 text queries".to_owned(),
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for query in queries {
        if query.id.trim().is_empty()
            || query.id.len() > 128
            || !ids.insert(query.id.as_str())
            || query.text.trim().is_empty()
            || query.text.len() > 2_000
            || query
                .target_label
                .as_ref()
                .is_some_and(|label| label.as_str().trim().is_empty())
        {
            return Err(CoreError::Validation(
                "grounding query ids must be unique and query text must be bounded".to_owned(),
            ));
        }
    }
    Ok(())
}

#[must_use]
pub fn grounding_parameter_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "required": ["queries"],
        "properties": {
            "queries": {
                "type": "array",
                "minItems": 1,
                "maxItems": 100,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "text"],
                    "properties": {
                        "id": {"type": "string", "minLength": 1, "maxLength": 128},
                        "text": {"type": "string", "minLength": 1, "maxLength": 2000},
                        "target_label": {"type": ["string", "null"]}
                    }
                }
            },
            "max_objects": {"type": "integer", "minimum": 1, "maximum": 10000},
            "generation_mode": {"type": "string", "enum": ["fast", "slow", "hybrid"]},
            "mock_empty": {"type": "boolean"}
        }
    })
}

pub struct OpenVocabularyGroundingSkill {
    manifest: SkillManifest,
}

impl Default for OpenVocabularyGroundingSkill {
    fn default() -> Self {
        Self {
            manifest: SkillManifest {
                version: 1,
                id: OPEN_VOCABULARY_GROUNDING_SKILL_ID.to_owned(),
                kind: SkillKind::Capability,
                skill_version: OPEN_VOCABULARY_GROUNDING_SKILL_VERSION.to_owned(),
                display_name: "Open-vocabulary grounding".to_owned(),
                description: "Find objects from category descriptions or referring phrases"
                    .to_owned(),
                rust_implementation: Some(
                    "annotagent_skill_open_vocabulary::OpenVocabularyGroundingSkill".to_owned(),
                ),
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                capabilities: vec![
                    "open_vocabulary_detection".to_owned(),
                    "phrase_grounding".to_owned(),
                ],
                requires: annotagent_core::SkillCapabilityRequirements::default(),
                optional_capabilities: Vec::new(),
                nodes: vec![
                    OPEN_VOCABULARY_DETECTION_OPERATION.to_owned(),
                    PHRASE_GROUNDING_OPERATION.to_owned(),
                ],
                tools: Vec::new(),
                validators: Vec::new(),
                policies: Vec::new(),
                templates: vec!["open-vocabulary.text-query-review".to_owned()],
                summary_resources: vec![
                    "open-vocabulary/summary.md".to_owned(),
                    "open-vocabulary/node.schema.json".to_owned(),
                ],
                task_resources: BTreeMap::new(),
                correction_taxonomy: Vec::new(),
                visual_profile: BTreeMap::new(),
            },
        }
    }
}

impl Skill for OpenVocabularyGroundingSkill {
    fn id(&self) -> &str {
        OPEN_VOCABULARY_GROUNDING_SKILL_ID
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn node_templates(&self) -> Vec<TaskTemplate> {
        vec![
            TaskTemplate {
                id: TaskId::from(OPEN_VOCABULARY_DETECTION_OPERATION),
                description: "Image + category queries → DetectionSet".to_owned(),
            },
            TaskTemplate {
                id: TaskId::from(PHRASE_GROUNDING_OPERATION),
                description: "Image + referring phrases → DetectionSet".to_owned(),
            },
        ]
    }

    fn workflow_templates(&self) -> Vec<WorkflowTemplate> {
        vec![text_query_review_template()]
    }

    fn resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        match request.resource_name.as_deref() {
            None => Ok(vec![summary_resource(), schema_resource()]),
            Some("open-vocabulary/summary.md") => Ok(vec![summary_resource()]),
            Some("open-vocabulary/node.schema.json") => Ok(vec![schema_resource()]),
            Some(other) => Err(CoreError::Validation(format!(
                "unknown Open-vocabulary Grounding resource {other:?}"
            ))),
        }
    }
}

fn summary_resource() -> SkillResource {
    SkillResource {
        name: "open-vocabulary/summary.md".to_owned(),
        media_type: "text/markdown".to_owned(),
        content: "Use bounded text queries to produce a DetectionSet. Query ids and optional Project Labels remain attached to every candidate. Missing model scores stay missing. Visual exemplar prompts are outside this Alpha contract.".to_owned(),
    }
}

fn schema_resource() -> SkillResource {
    SkillResource {
        name: "open-vocabulary/node.schema.json".to_owned(),
        media_type: "application/schema+json".to_owned(),
        content: serde_json::to_string_pretty(&grounding_parameter_schema())
            .expect("static Grounding schema serializes"),
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

fn text_query_review_template() -> WorkflowTemplate {
    let node = |id: &str, node_type: &str, kind, inputs, outputs| WorkflowDraftNode {
        id: id.to_owned(),
        node_type: node_type.to_owned(),
        kind,
        inputs,
        outputs,
        required_skills: vec![OPEN_VOCABULARY_GROUNDING_SKILL_ID.to_owned()],
        ..WorkflowDraftNode::default()
    };
    let mut detector = node(
        "grounding",
        OPEN_VOCABULARY_DETECTION_OPERATION,
        WorkflowNodeKind::VisionModel,
        vec![port("image", ArtifactKind::Image)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    detector.parameters = BTreeMap::from([
        (
            "queries".to_owned(),
            serde_json::json!([{
                "id": "target",
                "text": "describe the object to find",
                "target_label": "target"
            }]),
        ),
        ("generation_mode".to_owned(), serde_json::json!("hybrid")),
        ("max_objects".to_owned(), serde_json::json!(20)),
    ]);
    detector.model_binding = Some("mock-open-vocabulary".to_owned());
    WorkflowTemplate {
        id: "open-vocabulary.text-query-review".to_owned(),
        name: "Find objects by description".to_owned(),
        description: "Image → Text-query grounding → Human Review → Commit".to_owned(),
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
            edge("image", "image", "grounding", "image"),
            edge("grounding", "detections", "review", "detections"),
            edge("review", "detections", "commit", "detections"),
        ],
        resource_versions: BTreeMap::from([
            (
                "open-vocabulary/summary.md".to_owned(),
                OPEN_VOCABULARY_GROUNDING_SKILL_VERSION.to_owned(),
            ),
            (
                "open-vocabulary/node.schema.json".to_owned(),
                OPEN_VOCABULARY_GROUNDING_SKILL_VERSION.to_owned(),
            ),
        ]),
        allow_unvalidated_commit: false,
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

#[must_use]
pub fn open_vocabulary_node_descriptor() -> VisionNodeDescriptor {
    node_descriptor(
        OPEN_VOCABULARY_DETECTION_OPERATION,
        "Find objects by description",
        VisionCapability::OpenVocabularyDetection,
    )
}

#[must_use]
pub fn phrase_grounding_node_descriptor() -> VisionNodeDescriptor {
    node_descriptor(
        PHRASE_GROUNDING_OPERATION,
        "Ground referring phrases",
        VisionCapability::PhraseGrounding,
    )
}

fn node_descriptor(
    id: &str,
    display_name: &str,
    capability: VisionCapability,
) -> VisionNodeDescriptor {
    VisionNodeDescriptor {
        id: id.to_owned(),
        display_name: display_name.to_owned(),
        required_capabilities: vec![capability],
        accepts: vec![ArtifactKind::Image],
        produces: vec![ArtifactKind::DetectionSet],
        deterministic: false,
    }
}

pub struct GroundingSkillRunner {
    backend: Arc<dyn PipelineModelBackend>,
    model_id: String,
    image: Option<ModelImage>,
}

impl GroundingSkillRunner {
    pub fn new(
        backend: Arc<dyn PipelineModelBackend>,
        model_id: impl Into<String>,
        image: Option<ModelImage>,
    ) -> CoreResult<Self> {
        if !matches!(
            backend.capability(),
            VisionCapability::OpenVocabularyDetection | VisionCapability::PhraseGrounding
        ) {
            return Err(CoreError::Validation(
                "Grounding Skill requires an open-vocabulary or phrase-grounding backend"
                    .to_owned(),
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
impl DagNodeRunner for GroundingSkillRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        let capability = operation_capability(&context.node.node_type).ok_or_else(|| {
            DagNodeFailure::terminal(
                "wrong_skill_operation",
                "Grounding runner received another operation",
            )
        })?;
        if self.backend.capability() != capability {
            return Err(DagNodeFailure::terminal(
                "grounding_capability_mismatch",
                "Grounding node and backend capabilities do not match",
            ));
        }
        if !context
            .input_pipeline_artifacts
            .iter()
            .any(|artifact| matches!(artifact, PipelineArtifact::Image(_)))
        {
            return Err(DagNodeFailure::terminal(
                "missing_image_input",
                "Grounding requires Image input",
            ));
        }
        let mut parameters = context.node.parameters.clone();
        validate_parameters(&parameters)?;
        if let Some(max_objects) = parameters.remove("max_objects") {
            parameters.insert("max_detections".to_owned(), max_objects);
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
                    operation: capability,
                    image: self.image.clone(),
                    input_artifacts: context.input_pipeline_artifacts,
                    parameters,
                    timeout_ms: context
                        .node
                        .resources
                        .timeout_seconds
                        .map(|seconds| seconds.saturating_mul(1_000)),
                },
                context.cancellation,
            )
            .await
            .map_err(|error| DagNodeFailure::retryable("grounding_backend", error.to_string()))?;
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
                "invalid_grounding_output",
                "Grounding backend must return one scoped DetectionSet Artifact",
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

fn operation_capability(operation: &str) -> Option<VisionCapability> {
    match operation {
        OPEN_VOCABULARY_DETECTION_OPERATION => Some(VisionCapability::OpenVocabularyDetection),
        PHRASE_GROUNDING_OPERATION => Some(VisionCapability::PhraseGrounding),
        _ => None,
    }
}

fn validate_parameters(
    parameters: &BTreeMap<String, serde_json::Value>,
) -> Result<Vec<GroundingQuery>, DagNodeFailure> {
    for forbidden in [
        "visual_prompt",
        "visual_prompt_box",
        "visual_exemplar",
        "exemplar_image",
    ] {
        if parameters
            .get(forbidden)
            .is_some_and(|value| !value.is_null())
        {
            return Err(DagNodeFailure::terminal(
                "visual_prompt_unsupported",
                "Open-vocabulary Grounding Alpha supports text queries only",
            ));
        }
    }
    let queries: Vec<GroundingQuery> =
        serde_json::from_value(parameters.get("queries").cloned().ok_or_else(|| {
            DagNodeFailure::terminal("missing_grounding_queries", "queries are required")
        })?)
        .map_err(|_| {
            DagNodeFailure::terminal(
                "invalid_grounding_queries",
                "queries do not match the Grounding JSON Schema",
            )
        })?;
    validate_grounding_queries(&queries).map_err(|error| {
        DagNodeFailure::terminal("invalid_grounding_queries", error.to_string())
    })?;
    if parameters
        .get("max_objects")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|value| value == 0 || value > 10_000)
    {
        return Err(DagNodeFailure::terminal(
            "invalid_grounding_limit",
            "max_objects must be within 1..=10000",
        ));
    }
    if parameters
        .get("generation_mode")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|mode| !matches!(mode, "fast" | "slow" | "hybrid"))
    {
        return Err(DagNodeFailure::terminal(
            "invalid_generation_mode",
            "generation_mode must be fast, slow, or hybrid",
        ));
    }
    Ok(queries)
}

#[derive(Debug, Clone)]
pub struct MockGroundingBackend {
    id: String,
    capability: VisionCapability,
}

impl MockGroundingBackend {
    pub fn new(id: impl Into<String>, capability: VisionCapability) -> CoreResult<Self> {
        if !matches!(
            capability,
            VisionCapability::OpenVocabularyDetection | VisionCapability::PhraseGrounding
        ) {
            return Err(CoreError::Validation(
                "Mock Grounding Backend requires a Grounding capability".to_owned(),
            ));
        }
        Ok(Self {
            id: id.into(),
            capability,
        })
    }
}

#[async_trait]
impl PipelineModelBackend for MockGroundingBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn capability(&self) -> VisionCapability {
        self.capability
    }

    async fn infer_pipeline(
        &self,
        request: PipelineInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<PipelineInferenceResponse> {
        if cancellation.is_cancelled() {
            return Err(CoreError::Provider(
                "mock Grounding inference cancelled".to_owned(),
            ));
        }
        if request.operation != self.capability {
            return Err(CoreError::Validation(
                "mock Grounding request capability mismatch".to_owned(),
            ));
        }
        let queries: Vec<GroundingQuery> = serde_json::from_value(
            request
                .parameters
                .get("queries")
                .cloned()
                .ok_or_else(|| CoreError::Validation("queries are required".to_owned()))?,
        )
        .map_err(|error| CoreError::Validation(format!("invalid Grounding queries: {error}")))?;
        validate_grounding_queries(&queries)?;
        let empty = request
            .parameters
            .get("mock_empty")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let max_objects = request
            .parameters
            .get("max_detections")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(u64::MAX) as usize;
        let artifact_id = format!("detection-set:{}", request.request_id);
        let detections = if empty {
            Vec::new()
        } else {
            queries
                .iter()
                .take(max_objects)
                .enumerate()
                .map(|(index, query)| {
                    let offset = (index as f32 * 0.08).min(0.64);
                    Detection::from_source(
                        format!("grounding-{index}"),
                        Some(query.id.clone()),
                        None,
                        query.target_label.clone(),
                        NormalizedRect::new(0.08 + offset, 0.12 + offset, 0.2, 0.18)?,
                        DetectionScore::not_provided(),
                        DetectionSource {
                            model_id: request.model_id.clone(),
                            capability: request.operation,
                            artifact_id: artifact_id.clone(),
                        },
                    )
                    .map_err(CoreError::Validation)
                })
                .collect::<CoreResult<Vec<_>>>()?
        };
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
            metadata: BTreeMap::from([
                ("backend".to_owned(), serde_json::json!("mock")),
                (
                    "score_semantics".to_owned(),
                    serde_json::json!("not_provided"),
                ),
            ]),
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
    use annotagent_core::{
        ArtifactRef, ImageArtifact, ModelImage, PipelineArtifact, RunId, SkillResourceRequest,
    };

    use super::*;

    fn request(capability: VisionCapability, empty: bool) -> PipelineInferenceRequest {
        PipelineInferenceRequest {
            protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
            request_id: "request-1".to_owned(),
            run_id: RunId::new(),
            image_id: ImageId::new(),
            node_id: "grounding".to_owned(),
            model_id: "mock-open-vocabulary".to_owned(),
            operation: capability,
            image: Some(ModelImage {
                id: "image".to_owned(),
                mime_type: "image/png".to_owned(),
                data_base64: "iVBORw0KGgo=".to_owned(),
            }),
            input_artifacts: vec![PipelineArtifact::Image(ImageArtifact {
                reference: ArtifactRef {
                    artifact_id: "image:1".to_owned(),
                    source_node: "image".to_owned(),
                    port: "image".to_owned(),
                    artifact_type: ArtifactKind::Image,
                    item_id: None,
                },
                image_id: ImageId::new(),
                width: 1,
                height: 1,
                mime_type: "image/png".to_owned(),
                blob_ref: "memory:image".to_owned(),
            })],
            parameters: BTreeMap::from([
                (
                    "queries".to_owned(),
                    serde_json::json!([
                        {"id": "ball", "text": "a small football", "target_label": "football"},
                        {"id": "robot", "text": "a humanoid robot", "target_label": "robot"}
                    ]),
                ),
                ("mock_empty".to_owned(), serde_json::json!(empty)),
            ]),
            timeout_ms: Some(1_000),
        }
    }

    #[test]
    fn manifest_schema_and_template_are_capability_driven() {
        let skill = OpenVocabularyGroundingSkill::default();
        assert_eq!(skill.manifest().kind, SkillKind::Capability);
        assert!(skill.manifest().validate().is_empty());
        let serialized = serde_json::to_string(&skill.workflow_templates()).expect("template");
        assert!(!serialized.to_ascii_lowercase().contains("locateanything"));
        assert!(serialized.contains("review_gate"));
        let schema = skill
            .resources(&SkillResourceRequest {
                task_id: None,
                resource_name: Some("open-vocabulary/node.schema.json".to_owned()),
            })
            .expect("schema");
        let value: serde_json::Value = serde_json::from_str(&schema[0].content).expect("JSON");
        assert_eq!(value["additionalProperties"], false);
        assert!(value["properties"].get("visual_prompt").is_none());
    }

    #[test]
    fn query_validation_and_visual_prompt_fail_closed() {
        let duplicate = vec![
            GroundingQuery {
                id: "same".to_owned(),
                text: "one".to_owned(),
                target_label: None,
            },
            GroundingQuery {
                id: "same".to_owned(),
                text: "two".to_owned(),
                target_label: None,
            },
        ];
        assert!(validate_grounding_queries(&duplicate).is_err());
        assert!(
            validate_parameters(&BTreeMap::from([
                (
                    "queries".to_owned(),
                    serde_json::json!([{"id":"a","text":"target"}])
                ),
                (
                    "visual_prompt_box".to_owned(),
                    serde_json::json!([0, 0, 1, 1])
                ),
            ]))
            .is_err()
        );
    }

    #[tokio::test]
    async fn mock_maps_every_query_without_inventing_scores() {
        for capability in [
            VisionCapability::OpenVocabularyDetection,
            VisionCapability::PhraseGrounding,
        ] {
            let backend = MockGroundingBackend::new("mock-grounding", capability).expect("mock");
            let response = backend
                .infer_pipeline(request(capability, false), CancellationToken::new())
                .await
                .expect("inference");
            let PipelineArtifact::DetectionSet(set) = &response.artifacts[0] else {
                panic!("DetectionSet")
            };
            assert_eq!(set.detections.len(), 2);
            assert_eq!(set.detections[0].query_id.as_deref(), Some("ball"));
            assert_eq!(
                set.detections[0]
                    .project_label
                    .as_ref()
                    .map(LabelId::as_str),
                Some("football")
            );
            assert_eq!(set.detections[0].score, DetectionScore::not_provided());
        }
    }

    #[tokio::test]
    async fn mock_no_object_is_a_valid_empty_detection_set() {
        let backend =
            MockGroundingBackend::new("mock-grounding", VisionCapability::OpenVocabularyDetection)
                .expect("mock");
        let response = backend
            .infer_pipeline(
                request(VisionCapability::OpenVocabularyDetection, true),
                CancellationToken::new(),
            )
            .await
            .expect("empty success");
        let PipelineArtifact::DetectionSet(set) = &response.artifacts[0] else {
            panic!("DetectionSet")
        };
        assert!(set.detections.is_empty());
        set.validate().expect("valid empty DetectionSet");
    }
}
