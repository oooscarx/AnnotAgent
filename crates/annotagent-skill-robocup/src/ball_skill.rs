use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    AnnotationValidator, ArtifactKind, CoreError, CoreResult, CorrectionKind, NodePort,
    ReviewPolicy, Skill, SkillManifest, SkillResource, SkillResourceRequest, TaskId, TaskTemplate,
    WorkflowDraftNode, WorkflowEdge, WorkflowNodeKind, WorkflowTemplate,
};

use crate::{RoboCupBallHardNegativeValidator, RoboCupReviewPolicy};

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
}

impl RoboCupBallSkill {
    pub fn new() -> CoreResult<Self> {
        Ok(Self {
            manifest: SkillManifest::from_yaml(include_str!(
                "../../../skills/robocup/ball/manifest.yaml"
            ))
            .map_err(|error| CoreError::InvalidManifest(error.to_string()))?,
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
        description: "Image → generic detector → Core Filter → RoboCup Ball Validators → Review Gate → Commit".to_owned(),
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
                detector,
                detector_kind,
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
            },
            node(
                "gate",
                "core.confidence_gate",
                WorkflowNodeKind::Gate,
                vec![port("detections", ArtifactKind::DetectionSet)],
                vec![port("detections", ArtifactKind::DetectionSet)],
            ),
            node(
                "review",
                "core.human_review",
                WorkflowNodeKind::HumanReview,
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
            edge("detector", "filter", None),
            edge("filter", "validate_ball", None),
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
        assert!(
            templates
                .iter()
                .flat_map(|template| &template.nodes)
                .all(|node| node.model_binding.is_none())
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
