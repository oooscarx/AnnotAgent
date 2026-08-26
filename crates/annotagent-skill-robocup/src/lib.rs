//! Production `RoboCup` perception skill for `AnnotAgent`.

mod ball;
mod field;
mod policy;
mod robot;
mod tools;

use std::sync::Arc;

use annotagent_core::{
    AgentTool, AnnotationRefiner, AnnotationValidator, CoreResult, CorrectionKind, DomainSkill,
    ReviewPolicy, SkillManifest, SkillResource, SkillResourceRequest, TaskGraph, TaskId, TaskNode,
    TaskTemplate,
};

pub use ball::*;
pub use field::*;
pub use policy::*;
pub use robot::*;
pub use tools::*;

pub struct RoboCupSkill {
    manifest: SkillManifest,
}

impl RoboCupSkill {
    pub fn new() -> CoreResult<Self> {
        let manifest =
            SkillManifest::from_yaml(include_str!("../../../skills/robocup/manifest.yaml"))
                .map_err(|error| annotagent_core::CoreError::InvalidManifest(error.to_string()))?;
        Ok(Self { manifest })
    }
}

impl DomainSkill for RoboCupSkill {
    fn id(&self) -> &str {
        &self.manifest.id
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn task_templates(&self) -> Vec<TaskTemplate> {
        [
            ("scene_type", "classify scene usability"),
            ("field_region", "outline playable field"),
            ("field_line", "trace and refine painted field lines"),
            ("penalty_mark", "locate penalty marks"),
            ("objects", "detect balls, robots, and people"),
            ("robot_attributes", "assign team color and robot state"),
        ]
        .into_iter()
        .map(|(id, description)| TaskTemplate {
            id: TaskId::from(id),
            description: description.to_owned(),
        })
        .collect()
    }

    fn workflow(&self) -> TaskGraph {
        TaskGraph {
            nodes: vec![
                node("scene_type", &[]),
                node("field_region", &["scene_type"]),
                node("field_line", &["field_region"]),
                node("penalty_mark", &["field_region"]),
                node("objects", &["field_region"]),
                node("robot_attributes", &["objects"]),
            ],
        }
    }

    fn tool_factories(&self) -> Vec<Arc<dyn AgentTool>> {
        vec![
            Arc::new(RoboCupFieldLineTool),
            Arc::new(BallEvidenceTool),
            Arc::new(TeamColorEvidenceTool),
        ]
    }

    fn validators(&self) -> Vec<Arc<dyn AnnotationValidator>> {
        vec![
            Arc::new(FieldContainmentValidator),
            Arc::new(WhiteLineAppearanceValidator::default()),
            Arc::new(PolylineContinuityValidator::default()),
            Arc::new(BallHardNegativeValidator::default()),
            Arc::new(RobotAttributeValidator),
        ]
    }

    fn refiners(&self) -> Vec<Arc<dyn AnnotationRefiner>> {
        vec![Arc::new(RoboCupFieldLineRefiner::default())]
    }

    fn prompt_resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        let mut resources = vec![resource(
            "SKILL.md",
            include_str!("../../../skills/robocup/SKILL.md"),
        )];
        if let Some(task) = &request.task_id {
            let task_resource = match task.as_str() {
                "field_region" => Some((
                    "tasks/field-region.md",
                    include_str!("../../../skills/robocup/tasks/field-region.md"),
                )),
                "field_line" => Some((
                    "tasks/field-line.md",
                    include_str!("../../../skills/robocup/tasks/field-line.md"),
                )),
                "objects" => Some((
                    "tasks/ball.md",
                    include_str!("../../../skills/robocup/tasks/ball.md"),
                )),
                "robot_attributes" => Some((
                    "tasks/robot.md",
                    include_str!("../../../skills/robocup/tasks/robot.md"),
                )),
                "penalty_mark" => Some((
                    "tasks/penalty-mark.md",
                    include_str!("../../../skills/robocup/tasks/penalty-mark.md"),
                )),
                _ => None,
            };
            if let Some((name, content)) = task_resource {
                resources.push(resource(name, content));
            }
        }
        Ok(resources)
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

    fn review_policy(&self) -> Arc<dyn ReviewPolicy> {
        Arc::new(RoboCupReviewPolicy)
    }
}

fn node(id: &str, dependencies: &[&str]) -> TaskNode {
    TaskNode {
        id: TaskId::from(id),
        depends_on: dependencies.iter().copied().map(TaskId::from).collect(),
    }
}

fn resource(name: &str, content: &str) -> SkillResource {
    SkillResource {
        name: name.to_owned(),
        media_type: "text/markdown".to_owned(),
        content: content.to_owned(),
    }
}
