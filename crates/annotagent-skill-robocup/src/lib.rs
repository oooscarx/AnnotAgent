//! Production `RoboCup` perception skill for `AnnotAgent`.

mod ball;
mod ball_skill;
mod evaluation;
mod field;
mod policy;
mod recovery;
mod robot;
mod tools;

use std::sync::Arc;

use annotagent_core::{
    AgentTool, AnnotationRefiner, AnnotationValidator, CoreResult, CorrectionKind, DomainSkill,
    ReviewPolicy, SkillManifest, SkillResource, SkillResourceRequest, TaskGraph, TaskId, TaskNode,
    TaskTemplate,
};

pub use ball::*;
pub use ball_skill::*;
pub use evaluation::*;
pub use field::*;
pub use policy::*;
pub use recovery::*;
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
        vec![TaskTemplate {
            id: TaskId::from("objects"),
            description: "detect and validate RoboCup footballs only".to_owned(),
        }]
    }

    fn workflow(&self) -> TaskGraph {
        TaskGraph {
            nodes: vec![node("objects", &[])],
        }
    }

    fn tool_factories(&self) -> Vec<Arc<dyn AgentTool>> {
        vec![Arc::new(BallEvidenceTool)]
    }

    fn validators(&self) -> Vec<Arc<dyn AnnotationValidator>> {
        vec![Arc::new(BallHardNegativeValidator::default())]
    }

    fn refiners(&self) -> Vec<Arc<dyn AnnotationRefiner>> {
        Vec::new()
    }

    fn prompt_resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        let mut resources = vec![resource(
            "SKILL.md",
            include_str!("../../../skills/robocup/SKILL.md"),
        )];
        if let Some(task) = &request.task_id {
            let task_resource = match task.as_str() {
                "objects" => Some((
                    "tasks/ball.md",
                    include_str!("../../../skills/robocup/tasks/ball.md"),
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

    fn workflow_templates(&self) -> Vec<annotagent_core::WorkflowTemplate> {
        RoboCupBallSkill::new()
            .map(|skill| annotagent_core::Skill::workflow_templates(&skill))
            .unwrap_or_default()
    }

    fn project_template(&self) -> Option<&str> {
        Some(include_str!("../../../examples/robocup/project.yaml"))
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
