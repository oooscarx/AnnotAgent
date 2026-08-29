use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    AnnotationRefiner, AnnotationValidator, ArtifactKind, CoreError, CoreResult, CorrectionKind,
    NodePort, ReviewPolicy, Skill, SkillManifest, SkillResource, SkillResourceRequest, TaskId,
    TaskTemplate, WorkflowDraftNode, WorkflowEdge, WorkflowNodeKind, WorkflowTemplate,
};

use crate::{
    RoboCupBallForegroundRefiner, RoboCupBallHardNegativeValidator, RoboCupReviewPolicy,
    RoboCupSamHttpRefiner,
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
            None | Some("SKILL.md") => Ok(vec![resource(
                "SKILL.md",
                include_str!("../../../skills/robocup/SKILL.md"),
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
            refiners: vec![
                Arc::new(RoboCupBallForegroundRefiner::default()),
                Arc::new(RoboCupSamHttpRefiner::from_env()?),
            ],
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
        vec![Arc::new(RoboCupBallHardNegativeValidator::default())]
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
            ]),
            Some("ball/SKILL.md") => Ok(vec![resource(
                "ball/SKILL.md",
                include_str!("../../../skills/robocup/ball/SKILL.md"),
            )]),
            Some("ball/resources/hard-negatives.md") => Ok(vec![resource(
                "ball/resources/hard-negatives.md",
                include_str!("../../../skills/robocup/ball/resources/hard-negatives.md"),
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

fn edge(from: &str, to: &str, route: Option<&str>) -> WorkflowEdge {
    WorkflowEdge {
        from_node: from.to_owned(),
        from_port: "detections".to_owned(),
        to_node: to.to_owned(),
        to_port: "detections".to_owned(),
        route: route.map(ToOwned::to_owned),
    }
}

fn ball_templates() -> Vec<WorkflowTemplate> {
    [
        (
            "robocup.ball.vlm-bootstrap",
            "RoboCup Ball · VLM bootstrap",
            "vlm_detection.detect",
            WorkflowNodeKind::VisionLanguageModel,
        ),
        (
            "robocup.ball.detector-first",
            "RoboCup Ball · detector first",
            "yolo_detection.detect",
            WorkflowNodeKind::VisionModel,
        ),
    ]
    .into_iter()
    .map(|(id, name, detector, detector_kind)| WorkflowTemplate {
        id: id.to_owned(),
        name: name.to_owned(),
        description: "Image → generic detector → Core Filter → prompted SAM refinement → RoboCup Ball Validators → Review Gate → Commit".to_owned(),
        nodes: vec![
            node(
                "image",
                "core.image_input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("image", ArtifactKind::Image)],
            ),
            {
                let mut detector_node = node(
                    "detector",
                    detector,
                    detector_kind,
                    vec![port("image", ArtifactKind::Image)],
                    vec![port("detections", ArtifactKind::DetectionSet)],
                );
                detector_node.model_binding = Some(if detector == "vlm_detection.detect" {
                    "default-vision".to_owned()
                } else {
                    "mock-detector".to_owned()
                });
                detector_node
                    .parameters
                    .insert("labels".to_owned(), serde_json::json!(["ball"]));
                detector_node.parameters.insert(
                    "target_description".to_owned(),
                    serde_json::json!("the compact round RoboCup football itself; return a pixel-tight box around the visible ball, ignore white field markings and green turf, and verify that every box edge encloses the ball"),
                );
                detector_node.parameters.insert(
                    "instruction".to_owned(),
                    serde_json::json!("Use the untouched image for recognition and the grid copy only to calibrate the small ball position. Inspect the full frame before returning exactly the visible football."),
                );
                detector_node.parameters.insert(
                    "localization_grid".to_owned(),
                    serde_json::json!({"rows": 8, "columns": 8}),
                );
                detector_node
            },
            {
                let mut filter = node(
                    "filter",
                    "core.filter",
                    WorkflowNodeKind::Transform,
                    vec![port("detections", ArtifactKind::DetectionSet)],
                    vec![port("detections", ArtifactKind::DetectionSet)],
                );
                filter
                    .parameters
                    .insert("labels".to_owned(), serde_json::json!(["ball"]));
                filter
            },
            {
                let mut refiner = node(
                    "refine_ball",
                    "annotation_refiner",
                    WorkflowNodeKind::Refiner,
                    vec![port("detections", ArtifactKind::DetectionSet)],
                    vec![port("detections", ArtifactKind::DetectionSet)],
                );
                refiner.refiners = vec!["sam_prompted_refiner".to_owned()];
                refiner.parameters.insert("task_id".to_owned(), serde_json::json!("objects"));
                refiner
            },
            {
                let mut validator = node(
                    "validate_ball",
                    "static_validator",
                    WorkflowNodeKind::Validator,
                    vec![port("detections", ArtifactKind::DetectionSet)],
                    vec![port("detections", ArtifactKind::DetectionSet)],
                );
                validator.validators = vec!["ball_hard_negative".to_owned()];
                validator
                    .parameters
                    .insert("task_id".to_owned(), serde_json::json!("objects"));
                validator
            },
            node(
                "gate",
                "core.confidence_gate",
                WorkflowNodeKind::Gate,
                vec![port("detections", ArtifactKind::DetectionSet)],
                vec![port("detections", ArtifactKind::DetectionSet)],
            ),
            {
                let mut review = node(
                    "review",
                    "review_gate",
                    WorkflowNodeKind::HumanReview,
                    vec![port("detections", ArtifactKind::DetectionSet)],
                    vec![port("detections", ArtifactKind::DetectionSet)],
                );
                review
                    .parameters
                    .insert("task_id".to_owned(), serde_json::json!("objects"));
                review
            },
            {
                let mut commit = node(
                    "commit",
                    "commit",
                    WorkflowNodeKind::Commit,
                    vec![port("detections", ArtifactKind::DetectionSet)],
                    Vec::new(),
                );
                commit
                    .parameters
                    .insert("task_id".to_owned(), serde_json::json!("objects"));
                commit.inputs[0].multiple = true;
                commit
            },
        ],
        edges: vec![
            WorkflowEdge {
                from_node: "image".to_owned(),
                from_port: "image".to_owned(),
                to_node: "detector".to_owned(),
                to_port: "image".to_owned(),
                route: None,
            },
            edge("detector", "filter", None),
            edge("filter", "refine_ball", None),
            edge("refine_ball", "validate_ball", None),
            edge("validate_ball", "gate", None),
            edge("gate", "commit", Some("pass")),
            edge("gate", "review", Some("review")),
            edge("review", "commit", None),
        ],
        resource_versions: BTreeMap::from([
            ("ball/SKILL.md".to_owned(), "1".to_owned()),
            ("ball/resources/hard-negatives.md".to_owned(), "1".to_owned()),
        ]),
        allow_unvalidated_commit: false,
    })
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
        assert_eq!(ball.validators().len(), 1);
        let templates = ball.workflow_templates();
        assert_eq!(templates.len(), 2);
        assert!(templates.iter().all(|template| {
            template
                .nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.kind,
                        WorkflowNodeKind::VisionModel | WorkflowNodeKind::VisionLanguageModel
                    )
                })
                .all(|node| node.model_binding.is_some())
        }));
        assert!(
            ball.resources(&SkillResourceRequest {
                task_id: None,
                resource_name: Some("../secret".to_owned()),
            })
            .is_err()
        );
    }
}
