//! Production `RoboCup` perception skill for `AnnotAgent`.

mod ball;
mod ball_skill;
mod evaluation;
mod field;
mod policy;
mod recovery;
mod robot;
mod sam;
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
pub use sam::*;
pub use tools::*;

pub struct RoboCupSkill {
    manifest: SkillManifest,
    refiners: Vec<Arc<dyn AnnotationRefiner>>,
}

impl RoboCupSkill {
    pub fn new() -> CoreResult<Self> {
        let manifest =
            SkillManifest::from_yaml(include_str!("../../../skills/robocup/manifest.yaml"))
                .map_err(|error| annotagent_core::CoreError::InvalidManifest(error.to_string()))?;
        Ok(Self {
            manifest,
            refiners: vec![Arc::new(RoboCupBallForegroundRefiner::default())],
        })
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
        vec![
            Arc::new(BallHardNegativeValidator::default()),
            Arc::new(RoboCupBallFieldRelationValidator),
        ]
    }

    fn refiners(&self) -> Vec<Arc<dyn AnnotationRefiner>> {
        self.refiners.clone()
    }

    fn prompt_resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        if let Some(resource_name) = request.resource_name.as_deref() {
            return match resource_name {
                "SKILL.md" => Ok(vec![resource(
                    "SKILL.md",
                    include_str!("../../../skills/robocup/SKILL.md"),
                )]),
                "resources/advisor.md" => Ok(vec![resource(
                    "resources/advisor.md",
                    include_str!("../../../skills/robocup/resources/advisor.md"),
                )]),
                "resources/detection-prompt.md" => Ok(vec![resource(
                    "resources/detection-prompt.md",
                    include_str!("../../../skills/robocup/ball/resources/detection-prompt.md"),
                )]),
                "resources/local-relocalization-prompt.md" => Ok(vec![resource(
                    "resources/local-relocalization-prompt.md",
                    include_str!(
                        "../../../skills/robocup/ball/resources/local-relocalization-prompt.md"
                    ),
                )]),
                "resources/crop-verification-prompt.md" => Ok(vec![resource(
                    "resources/crop-verification-prompt.md",
                    include_str!(
                        "../../../skills/robocup/ball/resources/crop-verification-prompt.md"
                    ),
                )]),
                "resources/hard-negatives.md" => Ok(vec![resource(
                    "resources/hard-negatives.md",
                    include_str!("../../../skills/robocup/ball/resources/hard-negatives.md"),
                )]),
                "tasks/ball.md" => Ok(vec![resource(
                    "tasks/ball.md",
                    include_str!("../../../skills/robocup/tasks/ball.md"),
                )]),
                other => Err(annotagent_core::CoreError::Validation(format!(
                    "unknown RoboCup resource {other:?}"
                ))),
            };
        }
        let mut resources = vec![
            resource("SKILL.md", include_str!("../../../skills/robocup/SKILL.md")),
            resource(
                "resources/advisor.md",
                include_str!("../../../skills/robocup/resources/advisor.md"),
            ),
            resource(
                "resources/detection-prompt.md",
                include_str!("../../../skills/robocup/ball/resources/detection-prompt.md"),
            ),
            resource(
                "resources/local-relocalization-prompt.md",
                include_str!(
                    "../../../skills/robocup/ball/resources/local-relocalization-prompt.md"
                ),
            ),
            resource(
                "resources/crop-verification-prompt.md",
                include_str!("../../../skills/robocup/ball/resources/crop-verification-prompt.md"),
            ),
            resource(
                "resources/hard-negatives.md",
                include_str!("../../../skills/robocup/ball/resources/hard-negatives.md"),
            ),
        ];
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
            .map(|skill| {
                annotagent_core::Skill::workflow_templates(&skill)
                    .into_iter()
                    .filter(|template| {
                        matches!(
                            template.id.as_str(),
                            "robocup.ball.vlm-bootstrap" | "robocup.ball.small-object-recovery"
                        )
                    })
                    .map(|mut template| {
                        for node in &mut template.nodes {
                            node.required_skills = vec!["robocup".to_owned()];
                            if let Some(resource_id) = node
                                .parameters
                                .get("prompt_resource_id")
                                .and_then(serde_json::Value::as_str)
                                .and_then(|id| id.strip_prefix("ball/"))
                                .map(ToOwned::to_owned)
                            {
                                node.parameters.insert(
                                    "prompt_resource_id".to_owned(),
                                    serde_json::json!(resource_id),
                                );
                            }
                            for validator in &mut node.validators {
                                if let Some(unqualified) = validator.strip_prefix("robocup.ball.") {
                                    *validator = unqualified.to_owned();
                                }
                            }
                        }
                        template.resource_versions = template
                            .resource_versions
                            .into_iter()
                            .map(|(resource, version)| {
                                (
                                    resource
                                        .strip_prefix("ball/")
                                        .unwrap_or(&resource)
                                        .to_owned(),
                                    version,
                                )
                            })
                            .chain([
                                ("SKILL.md".to_owned(), "1".to_owned()),
                                ("resources/advisor.md".to_owned(), "1".to_owned()),
                                ("tasks/ball.md".to_owned(), "1".to_owned()),
                            ])
                            .collect();
                        template
                    })
                    .collect()
            })
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
