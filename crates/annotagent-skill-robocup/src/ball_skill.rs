use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    AgentBudget, AnnotationRefiner, AnnotationValidator, ArtifactKind, CoreError, CoreResult,
    CorrectionKind, DetectionRecoveryPolicy, NodePort, ReviewPolicy, Skill, SkillManifest,
    SkillResource, SkillResourceRequest, TaskId, TaskTemplate, WorkflowDraftNode, WorkflowEdge,
    WorkflowNodeKind, WorkflowTemplate,
};

use crate::{
    RoboCupBallFieldRelationValidator, RoboCupBallForegroundRefiner,
    RoboCupBallHardNegativeValidator, RoboCupReviewPolicy,
};

pub const ROBOCUP_PACK_ID: &str = "robocup";
pub const ROBOCUP_BALL_SKILL_ID: &str = "robocup.ball";

pub struct RoboCupPackSkill {
    manifest: SkillManifest,
}

impl RoboCupPackSkill {
    pub fn new() -> CoreResult<Self> {
        Ok(Self {
            manifest: SkillManifest::from_yaml(include_str!(
                "../../../skills/robocup/manifest.yaml"
            ))
            .map_err(|error| CoreError::InvalidManifest(error.to_string()))?,
        })
    }
}

impl Skill for RoboCupPackSkill {
    fn id(&self) -> &str {
        ROBOCUP_PACK_ID
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        match request.resource_name.as_deref() {
            None => Ok(vec![
                resource("SKILL.md", include_str!("../../../skills/robocup/SKILL.md")),
                resource(
                    "resources/advisor.md",
                    include_str!("../../../skills/robocup/resources/advisor.md"),
                ),
            ]),
            Some("SKILL.md") => Ok(vec![resource(
                "SKILL.md",
                include_str!("../../../skills/robocup/SKILL.md"),
            )]),
            Some("resources/advisor.md") => Ok(vec![resource(
                "resources/advisor.md",
                include_str!("../../../skills/robocup/resources/advisor.md"),
            )]),
            Some(other) => Err(CoreError::Validation(format!(
                "unknown RoboCup Pack resource {other:?}"
            ))),
        }
    }
}

pub struct RoboCupBallSkill {
    manifest: SkillManifest,
    refiners: Vec<Arc<dyn AnnotationRefiner>>,
}

impl RoboCupBallSkill {
    pub fn new() -> CoreResult<Self> {
        Ok(Self {
            manifest: SkillManifest::from_yaml(include_str!(
                "../../../skills/robocup/ball/manifest.yaml"
            ))
            .map_err(|error| CoreError::InvalidManifest(error.to_string()))?,
            refiners: vec![Arc::new(RoboCupBallForegroundRefiner::default())],
        })
    }
}

impl Skill for RoboCupBallSkill {
    fn id(&self) -> &str {
        ROBOCUP_BALL_SKILL_ID
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn node_templates(&self) -> Vec<TaskTemplate> {
        vec![TaskTemplate {
            id: TaskId::from("robocup.ball.validate"),
            description: "Validate ball candidates against RoboCup hard negatives".to_owned(),
        }]
    }

    fn validators(&self) -> Vec<Arc<dyn AnnotationValidator>> {
        vec![
            Arc::new(RoboCupBallHardNegativeValidator::default()),
            Arc::new(RoboCupBallFieldRelationValidator),
        ]
    }

    fn refiners(&self) -> Vec<Arc<dyn AnnotationRefiner>> {
        self.refiners.clone()
    }
    fn review_policies(&self) -> Vec<(String, Arc<dyn ReviewPolicy>)> {
        vec![(
            "robocup.ball.review".to_owned(),
            Arc::new(RoboCupReviewPolicy),
        )]
    }

    fn workflow_templates(&self) -> Vec<WorkflowTemplate> {
        ball_templates()
    }

    fn resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        let requested = request.resource_name.as_deref();
        match requested {
            None => Ok(vec![
                resource(
                    "ball/SKILL.md",
                    include_str!("../../../skills/robocup/ball/SKILL.md"),
                ),
                resource(
                    "ball/resources/hard-negatives.md",
                    include_str!("../../../skills/robocup/ball/resources/hard-negatives.md"),
                ),
                resource(
                    "ball/resources/advisor.md",
                    include_str!("../../../skills/robocup/resources/advisor.md"),
                ),
            ]),
            Some("ball/SKILL.md") => Ok(vec![resource(
                "ball/SKILL.md",
                include_str!("../../../skills/robocup/ball/SKILL.md"),
            )]),
            Some("ball/resources/hard-negatives.md") => Ok(vec![resource(
                "ball/resources/hard-negatives.md",
                include_str!("../../../skills/robocup/ball/resources/hard-negatives.md"),
            )]),
            Some("ball/resources/advisor.md") => Ok(vec![resource(
                "ball/resources/advisor.md",
                include_str!("../../../skills/robocup/resources/advisor.md"),
            )]),
            Some(other) => Err(CoreError::Validation(format!(
                "unknown RoboCup Ball resource {other:?}"
            ))),
        }
    }

    fn correction_taxonomy(&self) -> Vec<CorrectionKind> {
        self.manifest
            .correction_taxonomy
            .iter()
            .map(|code| CorrectionKind {
                code: code.clone(),
                description: code.replace('_', " "),
            })
            .collect()
    }
}

fn resource(name: &str, content: &str) -> SkillResource {
    SkillResource {
        name: name.to_owned(),
        media_type: "text/markdown".to_owned(),
        content: content.to_owned(),
    }
}

fn port(id: &str, kind: ArtifactKind) -> NodePort {
    NodePort {
        id: id.to_owned(),
        artifact_type: kind,
        required: true,
        multiple: false,
    }
}

fn optional_port(id: &str, kind: ArtifactKind) -> NodePort {
    NodePort {
        required: false,
        ..port(id, kind)
    }
}

fn node(
    id: &str,
    operation: &str,
    kind: WorkflowNodeKind,
    inputs: Vec<NodePort>,
    outputs: Vec<NodePort>,
) -> WorkflowDraftNode {
    WorkflowDraftNode {
        id: id.to_owned(),
        node_type: operation.to_owned(),
        kind,
        inputs,
        outputs,
        required_skills: vec![ROBOCUP_BALL_SKILL_ID.to_owned()],
        ..WorkflowDraftNode::default()
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

fn ball_templates() -> Vec<WorkflowTemplate> {
    vec![vlm_bootstrap_template(), specialist_fallback_template()]
}

fn vlm_bootstrap_template() -> WorkflowTemplate {
    let mut detector = node(
        "detector",
        "vlm_detection.detect",
        WorkflowNodeKind::VisionLanguageModel,
        vec![port("image", ArtifactKind::Image)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    detector.model_binding = Some("default-vision".to_owned());
    detector
        .parameters
        .insert("labels".to_owned(), serde_json::json!(["ball"]));
    detector.parameters.insert(
        "target_description".to_owned(),
        serde_json::json!("the compact round RoboCup football itself; ignore white field markings, footwear, robots, and green turf"),
    );
    let mut select = node(
        "select_football",
        "core.filter",
        WorkflowNodeKind::Transform,
        vec![port("detections", ArtifactKind::DetectionSet)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    select.required_skills.clear();
    select.parameters.extend([
        ("labels".to_owned(), serde_json::json!(["ball", "football"])),
        (
            "class_mapping".to_owned(),
            serde_json::json!({"ball": "ball", "football": "ball", "sports ball": "ball"}),
        ),
        ("minimum_confidence".to_owned(), serde_json::json!(0.0)),
    ]);
    let mut validator = node(
        "validate_ball",
        "static_validator",
        WorkflowNodeKind::Validator,
        vec![port("detections", ArtifactKind::DetectionSet)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    validator.validators = vec![
        "robocup.ball.ball_hard_negative".to_owned(),
        "robocup.ball.robocup_ball_field_relation".to_owned(),
    ];
    validator
        .parameters
        .insert("task_id".to_owned(), serde_json::json!("objects"));
    let mut gate = node(
        "gate",
        "core.confidence_gate",
        WorkflowNodeKind::Gate,
        vec![port("detections", ArtifactKind::DetectionSet)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    gate.parameters
        .insert("threshold".to_owned(), serde_json::json!(0.92));
    WorkflowTemplate {
        id: "robocup.ball.vlm-bootstrap".to_owned(),
        name: "RoboCup Ball · VLM bootstrap".to_owned(),
        description: "Image → one VLM detector → Select football candidates → RoboCup validation → Decision → review or save".to_owned(),
        nodes: vec![
            node(
                "image",
                "core.image_input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("image", ArtifactKind::Image)],
            ),
            detector,
            select,
            validator,
            gate,
            node(
                "review",
                "review_gate",
                WorkflowNodeKind::HumanReview,
                vec![port("detections", ArtifactKind::DetectionSet)],
                vec![port("detections", ArtifactKind::DetectionSet)],
            ),
            {
                let mut commit = node(
                    "commit",
                    "commit",
                    WorkflowNodeKind::Commit,
                    vec![port("detections", ArtifactKind::DetectionSet)],
                    Vec::new(),
                );
                commit.inputs[0].multiple = true;
                commit
            },
        ],
        edges: vec![
            edge("image", "image", "detector", "image", None),
            edge("detector", "detections", "select_football", "detections", None),
            edge("select_football", "detections", "validate_ball", "detections", None),
            edge("validate_ball", "detections", "gate", "detections", None),
            edge("gate", "detections", "commit", "detections", Some("pass")),
            edge("gate", "detections", "review", "detections", Some("review")),
            edge("review", "detections", "commit", "detections", None),
        ],
        resource_versions: ball_resources(),
        allow_unvalidated_commit: false,
    }
}

fn specialist_fallback_template() -> WorkflowTemplate {
    let required = |node: &mut WorkflowDraftNode, skills: &[&str]| {
        node.required_skills = skills.iter().map(|skill| (*skill).to_owned()).collect();
    };
    let mut specialist = node(
        "specialist",
        "object_detection.detect",
        WorkflowNodeKind::VisionModel,
        vec![port("image", ArtifactKind::Image)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    required(&mut specialist, &["annotagent.detection"]);
    specialist.parameters.extend([
        ("target_labels".to_owned(), serde_json::json!(["football"])),
        (
            "recover_on_backend_error".to_owned(),
            serde_json::json!(true),
        ),
        (
            "class_mapping".to_owned(),
            serde_json::json!({"ball": "football", "football": "football"}),
        ),
    ]);

    let mut validate_primary = node(
        "validate_primary",
        "static_validator",
        WorkflowNodeKind::Validator,
        vec![port("detections", ArtifactKind::DetectionSet)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    validate_primary.validators = vec![
        "robocup.ball.ball_hard_negative".to_owned(),
        "robocup.ball.robocup_ball_field_relation".to_owned(),
    ];
    validate_primary.parameters.extend([
        ("task_id".to_owned(), serde_json::json!("footballs")),
        (
            "correction_memory_skill_id".to_owned(),
            serde_json::json!(ROBOCUP_BALL_SKILL_ID),
        ),
    ]);

    let mut policy = DetectionRecoveryPolicy::default();
    policy.initial_gate.accept_when[0].minimum_score = Some(0.9);
    policy.initial_gate.fallback_when[1].specialist_score_below = Some(0.72);
    let mut recovery = node(
        "recovery",
        "agent.detection_recovery",
        WorkflowNodeKind::Gate,
        vec![
            port("image", ArtifactKind::Image),
            port("primary", ArtifactKind::DetectionSet),
        ],
        vec![port("candidates", ArtifactKind::CandidateClusterSet)],
    );
    required(
        &mut recovery,
        &[ROBOCUP_BALL_SKILL_ID, "annotagent.detection"],
    );
    recovery.review_gate = true;
    recovery.parameters.extend([
        (
            "queries".to_owned(),
            serde_json::json!([{
                "id": "football",
                "text": "the compact round football on the playing field, excluding white shoes, socks, penalty marks, and line intersections",
                "target_label": "football"
            }]),
        ),
        (
            "recovery_policy".to_owned(),
            serde_json::to_value(policy).expect("static Recovery policy serializes"),
        ),
        (
            "agent_budget".to_owned(),
            serde_json::to_value(AgentBudget {
                max_steps: 4,
                max_tool_calls: 4,
                max_tokens: None,
                max_cost: None,
            })
            .expect("static Agent budget serializes"),
        ),
    ]);

    let mut project_candidates = node(
        "project_candidates",
        "core.project_detection_candidates",
        WorkflowNodeKind::Transform,
        vec![port("candidates", ArtifactKind::CandidateClusterSet)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    project_candidates.required_skills.clear();
    let mut validate_recovered = node(
        "validate_recovered",
        "static_validator",
        WorkflowNodeKind::Validator,
        vec![port("detections", ArtifactKind::DetectionSet)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    validate_recovered
        .validators
        .clone_from(&validate_primary.validators);
    validate_recovered.parameters = validate_primary.parameters.clone();

    let mut crop = node(
        "crop_verify",
        "core.crop",
        WorkflowNodeKind::Transform,
        vec![
            port("image", ArtifactKind::Image),
            port("detections", ArtifactKind::DetectionSet),
        ],
        vec![port("crops", ArtifactKind::CropSet)],
    );
    crop.required_skills.clear();
    crop.parameters
        .insert("padding".to_owned(), serde_json::json!(0.12));
    let mut classifier = node(
        "classify_crop",
        "classification.classify",
        WorkflowNodeKind::VisionModel,
        vec![port("crops", ArtifactKind::CropSet)],
        vec![port("classifications", ArtifactKind::ClassificationSet)],
    );
    required(&mut classifier, &["annotagent.classification"]);
    classifier.parameters.insert(
        "labels".to_owned(),
        serde_json::json!(["football", "not_football"]),
    );
    let mut verify = node(
        "verify_crop",
        "classification.verify",
        WorkflowNodeKind::Validator,
        vec![
            port("classifications", ArtifactKind::ClassificationSet),
            optional_port("detections", ArtifactKind::DetectionSet),
        ],
        vec![port("classifications", ArtifactKind::ClassificationSet)],
    );
    required(
        &mut verify,
        &["annotagent.classification", ROBOCUP_BALL_SKILL_ID],
    );
    verify.parameters.extend([
        (
            "labels".to_owned(),
            serde_json::json!(["football", "not_football"]),
        ),
        ("accept_labels".to_owned(), serde_json::json!(["football"])),
        (
            "reject_labels".to_owned(),
            serde_json::json!(["not_football"]),
        ),
        ("minimum_confidence".to_owned(), serde_json::json!(0.72)),
        (
            "review_on_validation_issue".to_owned(),
            serde_json::json!(true),
        ),
    ]);
    let mut attach = node(
        "attach_verified",
        "core.attach_result",
        WorkflowNodeKind::CandidateMerge,
        vec![
            port("detections", ArtifactKind::DetectionSet),
            port("classifications", ArtifactKind::ClassificationSet),
        ],
        vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
    );
    attach.required_skills.clear();
    attach.inputs[1].multiple = true;
    attach.parameters.extend([
        ("task_id".to_owned(), serde_json::json!("footballs")),
        (
            "class_mapping".to_owned(),
            serde_json::json!({"football": "football"}),
        ),
    ]);
    let mut verified_gate = node(
        "verified_gate",
        "core.confidence_gate",
        WorkflowNodeKind::Gate,
        vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
        vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
    );
    verified_gate.required_skills.clear();
    verified_gate
        .parameters
        .insert("threshold".to_owned(), serde_json::json!(0.72));

    WorkflowTemplate {
        id: "robocup.ball.specialist_with_open_vocab_fallback".to_owned(),
        name: "RoboCup Ball · specialist with open-vocabulary fallback".to_owned(),
        description: "Image → specialist capability → domain evidence → bounded open-vocabulary fallback → candidate projection → Crop Verify → commit, reject, or review".to_owned(),
        nodes: vec![
            node(
                "image",
                "core.image_input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("image", ArtifactKind::Image)],
            ),
            specialist,
            validate_primary,
            recovery,
            project_candidates,
            validate_recovered,
            crop,
            classifier,
            verify,
            attach,
            verified_gate,
            node(
                "review_evidence",
                "review_gate",
                WorkflowNodeKind::HumanReview,
                vec![port("candidates", ArtifactKind::CandidateClusterSet)],
                vec![port("candidates", ArtifactKind::CandidateClusterSet)],
            ),
            node(
                "review_verified",
                "review_gate",
                WorkflowNodeKind::HumanReview,
                vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
                vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
            ),
            {
                let mut commit = node(
                    "commit_evidence",
                    "commit",
                    WorkflowNodeKind::Commit,
                    vec![port("candidates", ArtifactKind::CandidateClusterSet)],
                    Vec::new(),
                );
                commit.inputs[0].multiple = true;
                commit
            },
            {
                let mut commit = node(
                    "commit_verified",
                    "commit",
                    WorkflowNodeKind::Commit,
                    vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
                    Vec::new(),
                );
                commit.inputs[0].multiple = true;
                commit
            },
            {
                let mut reject = node(
                    "reject_hard_negative",
                    "core.reject_candidates",
                    WorkflowNodeKind::Export,
                    vec![port("classifications", ArtifactKind::ClassificationSet)],
                    vec![port("classifications", ArtifactKind::ClassificationSet)],
                );
                reject.parameters.insert(
                    "reason".to_owned(),
                    serde_json::json!("crop verification classified the candidate as a RoboCup hard negative"),
                );
                reject
            },
        ],
        edges: vec![
            edge("image", "image", "specialist", "image", None),
            edge("specialist", "detections", "validate_primary", "detections", None),
            edge("image", "image", "recovery", "image", None),
            edge("validate_primary", "detections", "recovery", "primary", None),
            edge("recovery", "candidates", "commit_evidence", "candidates", Some("accept")),
            edge("recovery", "candidates", "review_evidence", "candidates", Some("review")),
            edge("recovery", "candidates", "project_candidates", "candidates", Some("verify")),
            edge("project_candidates", "detections", "validate_recovered", "detections", None),
            edge("image", "image", "crop_verify", "image", None),
            edge("validate_recovered", "detections", "crop_verify", "detections", None),
            edge("crop_verify", "crops", "classify_crop", "crops", None),
            edge("classify_crop", "classifications", "verify_crop", "classifications", None),
            edge("validate_recovered", "detections", "verify_crop", "detections", None),
            edge("validate_recovered", "detections", "attach_verified", "detections", None),
            edge("verify_crop", "classifications", "attach_verified", "classifications", Some("accept")),
            edge("verify_crop", "classifications", "attach_verified", "classifications", Some("review")),
            edge("verify_crop", "classifications", "reject_hard_negative", "classifications", Some("reject")),
            edge("attach_verified", "candidates", "verified_gate", "candidates", None),
            edge("verified_gate", "candidates", "commit_verified", "candidates", Some("pass")),
            edge("verified_gate", "candidates", "review_verified", "candidates", Some("review")),
            edge("review_verified", "candidates", "commit_verified", "candidates", None),
            edge("review_evidence", "candidates", "commit_evidence", "candidates", None),
        ],
        resource_versions: hybrid_resources(),
        allow_unvalidated_commit: false,
    }
}

fn ball_resources() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("ball/SKILL.md".to_owned(), "1".to_owned()),
        (
            "ball/resources/hard-negatives.md".to_owned(),
            "1".to_owned(),
        ),
        ("ball/resources/advisor.md".to_owned(), "1".to_owned()),
    ])
}

fn hybrid_resources() -> BTreeMap<String, String> {
    ball_resources()
        .into_iter()
        .map(|(resource, version)| (format!("{ROBOCUP_BALL_SKILL_ID}.{resource}"), version))
        .collect()
}

#[cfg(test)]
mod tests {
    use annotagent_core::{SkillKind, SkillResourceRequest};

    use super::*;

    #[test]
    fn pack_and_ball_domain_are_separate_and_templates_are_model_agnostic() {
        let pack = RoboCupPackSkill::new().expect("Pack");
        let ball = RoboCupBallSkill::new().expect("Ball Skill");
        assert_eq!(pack.manifest().kind, SkillKind::Pack);
        assert_eq!(ball.manifest().kind, SkillKind::Domain);
        assert_eq!(ball.validators().len(), 2);
        let templates = ball.workflow_templates();
        assert_eq!(templates.len(), 2);
        let hybrid = templates
            .iter()
            .find(|template| template.id == "robocup.ball.specialist_with_open_vocab_fallback")
            .expect("hybrid template");
        assert!(
            hybrid
                .nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.kind,
                        WorkflowNodeKind::VisionModel | WorkflowNodeKind::VisionLanguageModel
                    ) || node.node_type == "agent.detection_recovery"
                })
                .all(|node| node.model_binding.is_none())
        );
        let serialized = serde_json::to_string(hybrid).expect("template JSON");
        assert!(!serialized.contains("rfdetr"));
        assert!(!serialized.contains("locate"));
        assert_eq!(
            ball.manifest().requires.capabilities,
            ["detection", "human_review"]
        );
        let advisor = ball
            .resources(&SkillResourceRequest {
                task_id: None,
                resource_name: Some("ball/resources/advisor.md".to_owned()),
            })
            .expect("Advisor resource");
        assert!(advisor[0].content.contains("smallest Pipeline"));
        assert!(
            advisor[0]
                .content
                .contains("Never add an unavailable, Unknown, disabled")
        );
        assert!(
            ball.resources(&SkillResourceRequest {
                task_id: None,
                resource_name: Some("../secret".to_owned()),
            })
            .is_err()
        );
    }
}
