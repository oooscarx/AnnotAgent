//! Safe, registry-bound workflow drafts, suggestions, and static validation.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{ArtifactKind, ModelRegistry, NodeRegistry, ProjectSchema, VisionCapability};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDraftStatus {
    Suggested,
    Editing,
    Validated,
    Published,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDraftNode {
    pub id: String,
    pub node_type: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub model_binding: Option<String>,
    #[serde(default)]
    pub validators: Vec<String>,
    #[serde(default)]
    pub refiners: Vec<String>,
    pub fallback: Option<String>,
    #[serde(default)]
    pub max_retries: u32,
    #[serde(default)]
    pub review_gate: bool,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDraft {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: WorkflowDraftStatus,
    pub nodes: Vec<WorkflowDraftNode>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowValidationIssue {
    pub code: String,
    pub path: String,
    pub message: String,
    pub blocking: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowValidationReport {
    pub valid: bool,
    pub issues: Vec<WorkflowValidationIssue>,
    pub execution_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowSuggestion {
    pub draft: WorkflowDraft,
    pub rationale: Vec<String>,
    pub unresolved_model_bindings: Vec<String>,
    pub warnings: Vec<String>,
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkflowConstraints {
    pub preferred_model_id: Option<String>,
    #[serde(default)]
    pub require_review_gate: bool,
    pub max_nodes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublishedWorkflowVersion {
    pub workflow_id: String,
    pub version: u32,
    pub project_id: String,
    pub source_draft_id: String,
    pub content_hash: String,
    pub draft: WorkflowDraft,
    pub published_at: DateTime<Utc>,
}

pub trait WorkflowAdvisor: Send + Sync {
    fn suggest_workflow(
        &self,
        project_id: &str,
        project_schema: &ProjectSchema,
        enabled_skills: &[String],
        node_catalog: &NodeRegistry,
        model_registry: &ModelRegistry,
        constraints: &WorkflowConstraints,
    ) -> WorkflowSuggestion;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RegistryWorkflowAdvisor;

impl WorkflowAdvisor for RegistryWorkflowAdvisor {
    fn suggest_workflow(
        &self,
        project_id: &str,
        project_schema: &ProjectSchema,
        enabled_skills: &[String],
        node_catalog: &NodeRegistry,
        model_registry: &ModelRegistry,
        constraints: &WorkflowConstraints,
    ) -> WorkflowSuggestion {
        let preferred = constraints.preferred_model_id.clone().or_else(|| {
            model_registry
                .models()
                .into_iter()
                .find(|model| {
                    model
                        .capabilities
                        .contains(&VisionCapability::VisionLanguage)
                })
                .map(|model| model.id)
        });
        let node_type = if node_catalog.get("vision_language").is_some() {
            "vision_language".to_owned()
        } else {
            node_catalog
                .nodes()
                .first()
                .map_or_else(|| "unresolved".to_owned(), |node| node.id.clone())
        };
        let mut nodes = project_schema
            .tasks
            .iter()
            .map(|task| WorkflowDraftNode {
                id: task.id.to_string(),
                node_type: node_type.clone(),
                depends_on: task.depends_on.iter().map(ToString::to_string).collect(),
                model_binding: preferred.clone(),
                validators: task.validators.clone(),
                refiners: task.refiners.clone(),
                fallback: None,
                max_retries: project_schema.runtime.max_retries,
                review_gate: false,
                parameters: BTreeMap::from([
                    ("task_kind".to_owned(), serde_json::json!(task.kind)),
                    ("required".to_owned(), serde_json::json!(task.required)),
                ]),
            })
            .collect::<Vec<_>>();
        if constraints.require_review_gate && node_catalog.get("review_gate").is_some() {
            let dependencies = nodes.iter().map(|node| node.id.clone()).collect();
            nodes.push(WorkflowDraftNode {
                id: "review_gate".to_owned(),
                node_type: "review_gate".to_owned(),
                depends_on: dependencies,
                model_binding: None,
                validators: Vec::new(),
                refiners: Vec::new(),
                fallback: None,
                max_retries: 0,
                review_gate: true,
                parameters: BTreeMap::new(),
            });
        }
        let mut warnings = Vec::new();
        if let Some(max_nodes) = constraints.max_nodes
            && nodes.len() > max_nodes
        {
            warnings.push(format!(
                "suggestion has {} nodes, above configured maximum {max_nodes}",
                nodes.len()
            ));
        }
        let unresolved_model_bindings = if preferred.is_none() {
            nodes
                .iter()
                .filter(|node| node.node_type == "vision_language")
                .map(|node| node.id.clone())
                .collect()
        } else {
            Vec::new()
        };
        let now = Utc::now();
        WorkflowSuggestion {
            draft: WorkflowDraft {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: project_id.to_owned(),
                name: format!("{} suggested workflow", project_schema.project.name),
                status: WorkflowDraftStatus::Suggested,
                nodes,
                created_at: now,
                updated_at: now,
            },
            rationale: vec![
                "Mapped each configured annotation task to a registered vision-language node."
                    .to_owned(),
                format!(
                    "Preserved validators and refiners from enabled Skills: {}.",
                    enabled_skills.join(", ")
                ),
            ],
            unresolved_model_bindings,
            warnings,
            alternatives: vec![
                "Bind detection or segmentation tasks to registered specialist backends."
                    .to_owned(),
                "Add a review gate after validators for conservative publishing.".to_owned(),
            ],
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WorkflowStaticValidator;

impl WorkflowStaticValidator {
    #[must_use]
    pub fn validate(
        &self,
        draft: &WorkflowDraft,
        node_catalog: &NodeRegistry,
        model_registry: &ModelRegistry,
    ) -> WorkflowValidationReport {
        let mut issues = Vec::new();
        let ids = draft
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<BTreeSet<_>>();
        if ids.len() != draft.nodes.len() {
            issues.push(issue(
                "duplicate_node_id",
                "nodes",
                "node ids must be unique",
            ));
        }
        for (index, node) in draft.nodes.iter().enumerate() {
            let path = format!("nodes[{index}]");
            let Some(descriptor) = node_catalog.get(&node.node_type) else {
                issues.push(issue(
                    "unknown_node",
                    &format!("{path}.node_type"),
                    &format!("node type {:?} is not registered", node.node_type),
                ));
                continue;
            };
            for dependency in &node.depends_on {
                if !ids.contains(dependency.as_str()) {
                    issues.push(issue(
                        "unknown_dependency",
                        &format!("{path}.depends_on"),
                        &format!("dependency {dependency:?} is not a draft node"),
                    ));
                }
            }
            if let Some(fallback) = &node.fallback
                && !ids.contains(fallback.as_str())
            {
                issues.push(issue(
                    "unknown_fallback",
                    &format!("{path}.fallback"),
                    &format!("fallback {fallback:?} is not a draft node"),
                ));
            }
            if descriptor.required_capabilities.is_empty() {
                continue;
            }
            let Some(model_id) = node.model_binding.as_deref() else {
                issues.push(issue(
                    "unresolved_model_binding",
                    &format!("{path}.model_binding"),
                    "this node requires a registered model binding",
                ));
                continue;
            };
            match model_registry.resolve(model_id) {
                Ok((model, _)) => {
                    for capability in &descriptor.required_capabilities {
                        if !model.capabilities.contains(capability) {
                            issues.push(issue(
                                "model_capability_mismatch",
                                &format!("{path}.model_binding"),
                                &format!("model {model_id:?} lacks {capability:?}"),
                            ));
                        }
                    }
                }
                Err(error) => issues.push(issue(
                    "unknown_model",
                    &format!("{path}.model_binding"),
                    &error.to_string(),
                )),
            }
        }
        let execution_order = topological_order(&draft.nodes).unwrap_or_else(|cycle| {
            issues.push(issue("workflow_cycle", "nodes", &cycle));
            Vec::new()
        });
        WorkflowValidationReport {
            valid: issues.iter().all(|issue| !issue.blocking),
            issues,
            execution_order,
        }
    }
}

fn issue(code: &str, path: &str, message: &str) -> WorkflowValidationIssue {
    WorkflowValidationIssue {
        code: code.to_owned(),
        path: path.to_owned(),
        message: message.to_owned(),
        blocking: true,
    }
}

fn topological_order(nodes: &[WorkflowDraftNode]) -> Result<Vec<String>, String> {
    let mut remaining = nodes
        .iter()
        .map(|node| {
            (
                node.id.clone(),
                node.depends_on.iter().cloned().collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut order = Vec::with_capacity(nodes.len());
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .filter(|(_, dependencies)| dependencies.iter().all(|id| order.contains(id)))
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return Err("workflow contains a dependency cycle".to_owned());
        }
        for id in ready {
            remaining.remove(&id);
            order.push(id);
        }
    }
    Ok(order)
}

#[must_use]
pub const fn all_artifact_kinds() -> [ArtifactKind; 7] {
    [
        ArtifactKind::Classification,
        ArtifactKind::BoundingBox,
        ArtifactKind::Keypoints,
        ArtifactKind::Polyline,
        ArtifactKind::Polygon,
        ArtifactKind::SemanticMask,
        ArtifactKind::InstanceMask,
    ]
}
