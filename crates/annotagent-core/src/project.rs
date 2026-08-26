//! Versioned project configuration and task dependency validation.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::{TaskId, TaskKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigIssue {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for ConfigIssue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSchema {
    pub version: u32,
    pub project: ProjectDescriptor,
    pub dataset: DatasetConfig,
    pub runtime: RuntimeConfig,
    pub tasks: Vec<TaskConfig>,
    pub review: ReviewConfig,
    pub export: ExportConfig,
}

impl ProjectSchema {
    pub fn from_yaml(input: &str) -> Result<Self, ConfigIssue> {
        let deserializer = serde_yaml::Deserializer::from_str(input);
        serde_path_to_error::deserialize(deserializer).map_err(|error| ConfigIssue {
            path: error.path().to_string(),
            message: error.inner().to_string(),
        })
    }

    #[must_use]
    pub fn validate(&self, catalog: &ValidationCatalog) -> Vec<ConfigIssue> {
        let mut issues = Vec::new();
        if self.version != 1 {
            issues.push(ConfigIssue {
                path: "version".to_owned(),
                message: format!("unsupported project schema version {}", self.version),
            });
        }
        let mut enabled_skill_ids = BTreeSet::new();
        for (index, skill) in self.project.enabled_skills.iter().enumerate() {
            if !valid_identifier(&skill.id) {
                issues.push(ConfigIssue {
                    path: format!("project.enabled_skills[{index}].id"),
                    message: format!("invalid Skill id {:?}", skill.id),
                });
            }
            if skill.version.trim().is_empty() {
                issues.push(ConfigIssue {
                    path: format!("project.enabled_skills[{index}].version"),
                    message: "Skill version cannot be empty".to_owned(),
                });
            }
            if !enabled_skill_ids.insert(skill.id.as_str()) {
                issues.push(ConfigIssue {
                    path: format!("project.enabled_skills[{index}].id"),
                    message: format!("duplicate enabled Skill {:?}", skill.id),
                });
            }
        }
        if self.dataset.root.as_os_str().is_empty() || self.dataset.root.is_absolute() {
            issues.push(ConfigIssue {
                path: "dataset.root".to_owned(),
                message: "dataset root must be a non-empty project-relative path".to_owned(),
            });
        }
        if self
            .dataset
            .root
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            issues.push(ConfigIssue {
                path: "dataset.root".to_owned(),
                message: "dataset root cannot contain '..'".to_owned(),
            });
        }

        let mut task_positions = HashMap::new();
        for (index, task) in self.tasks.iter().enumerate() {
            if !valid_identifier(task.id.as_str()) {
                issues.push(ConfigIssue {
                    path: format!("tasks[{index}].id"),
                    message: format!("invalid task id {:?}", task.id.as_str()),
                });
            }
            if let Some(previous) = task_positions.insert(task.id.clone(), index) {
                issues.push(ConfigIssue {
                    path: format!("tasks[{index}].id"),
                    message: format!(
                        "duplicate task id {:?}; first defined at tasks[{previous}]",
                        task.id.as_str()
                    ),
                });
            }
            let mut labels = BTreeSet::new();
            for (label_index, label) in task.labels.iter().enumerate() {
                if !labels.insert(label) {
                    issues.push(ConfigIssue {
                        path: format!("tasks[{index}].labels[{label_index}]"),
                        message: format!("duplicate label id {label:?}"),
                    });
                }
                if !valid_identifier(label) {
                    issues.push(ConfigIssue {
                        path: format!("tasks[{index}].labels[{label_index}]"),
                        message: format!("invalid label id {label:?}"),
                    });
                }
            }
            for (attribute_name, attribute) in &task.attributes {
                if attribute.required
                    && attribute.kind == AttributeKind::Enum
                    && attribute.values.is_empty()
                {
                    issues.push(ConfigIssue {
                        path: format!("tasks[{index}].attributes.{attribute_name}.values"),
                        message: "required enum attribute must declare values".to_owned(),
                    });
                }
            }
            for (validator_index, validator) in task.validators.iter().enumerate() {
                if !catalog.validators.contains(validator) {
                    issues.push(ConfigIssue {
                        path: format!("tasks[{index}].validators[{validator_index}]"),
                        message: format!("unknown validator {validator:?}"),
                    });
                }
            }
            for (refiner_index, refiner) in task.refiners.iter().enumerate() {
                if !catalog.refiners.contains(refiner) {
                    issues.push(ConfigIssue {
                        path: format!("tasks[{index}].refiners[{refiner_index}]"),
                        message: format!("unknown refiner {refiner:?}"),
                    });
                }
            }
        }
        for (index, task) in self.tasks.iter().enumerate() {
            for (dependency_index, dependency) in task.depends_on.iter().enumerate() {
                if !task_positions.contains_key(dependency) {
                    issues.push(ConfigIssue {
                        path: format!("tasks[{index}].depends_on[{dependency_index}]"),
                        message: format!("unknown task id {:?}", dependency.as_str()),
                    });
                }
            }
            if let Some(target) = &task.target_task
                && !task_positions.contains_key(target)
            {
                issues.push(ConfigIssue {
                    path: format!("tasks[{index}].target_task"),
                    message: format!("unknown task id {:?}", target.as_str()),
                });
            }
        }
        if let Err(cycle) =
            TaskGraph::from_tasks(&self.tasks).and_then(|graph| graph.topological_order())
        {
            issues.push(ConfigIssue {
                path: "tasks".to_owned(),
                message: cycle,
            });
        }
        issues
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectDescriptor {
    pub name: String,
    /// Legacy single-Skill binding. New projects use `enabled_skills`.
    #[serde(default)]
    pub skill: String,
    #[serde(default)]
    pub skill_version: String,
    #[serde(default)]
    pub enabled_skills: Vec<EnabledSkillConfig>,
    #[serde(default = "default_language")]
    pub language: String,
}

impl ProjectDescriptor {
    /// Returns a deterministic Skill id -> version projection for both old and new schemas.
    #[must_use]
    pub fn enabled_skill_versions(&self) -> BTreeMap<String, String> {
        if !self.enabled_skills.is_empty() {
            return self
                .enabled_skills
                .iter()
                .map(|skill| (skill.id.clone(), skill.version.clone()))
                .collect();
        }
        if self.skill.trim().is_empty() {
            BTreeMap::new()
        } else {
            BTreeMap::from([(self.skill.clone(), self.skill_version.clone())])
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnabledSkillConfig {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub configuration: BTreeMap<String, String>,
}

fn default_language() -> String {
    "en".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetConfig {
    pub root: std::path::PathBuf,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default = "default_true")]
    pub recursive: bool,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    #[serde(default = "default_parallel_images")]
    pub max_parallel_images: usize,
    #[serde(default = "default_model_turns")]
    pub max_model_turns_per_task: u32,
    #[serde(default = "default_tool_calls")]
    pub max_tool_calls_per_task: u32,
    #[serde(default = "default_recovery_turns")]
    pub max_recovery_turns_per_task: u32,
    #[serde(default = "default_task_timeout")]
    pub task_timeout_seconds: u64,
    #[serde(default = "default_provider_timeout")]
    pub provider_request_timeout_seconds: u64,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub auto_resume: bool,
}

const fn default_parallel_images() -> usize {
    2
}
const fn default_model_turns() -> u32 {
    8
}
const fn default_tool_calls() -> u32 {
    12
}
const fn default_recovery_turns() -> u32 {
    2
}
const fn default_task_timeout() -> u64 {
    300
}
const fn default_provider_timeout() -> u64 {
    120
}
const fn default_retries() -> u32 {
    3
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskConfig {
    pub id: TaskId,
    pub kind: TaskKind,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub multi_label: bool,
    #[serde(default)]
    pub depends_on: Vec<TaskId>,
    #[serde(default)]
    pub validators: Vec<String>,
    #[serde(default)]
    pub refiners: Vec<String>,
    #[serde(default)]
    pub target_task: Option<TaskId>,
    #[serde(default)]
    pub target_labels: Vec<String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, AttributeDefinition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeKind {
    Enum,
    String,
    Number,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeDefinition {
    #[serde(rename = "type")]
    pub kind: AttributeKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewConfig {
    pub auto_accept_confidence: f32,
    pub force_review_below: f32,
    #[serde(default)]
    pub force_review_on_warning_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportConfig {
    #[serde(default)]
    pub formats: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ValidationCatalog {
    pub validators: BTreeSet<String>,
    pub refiners: BTreeSet<String>,
    pub resources: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: TaskId,
    pub depends_on: Vec<TaskId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGraph {
    pub nodes: Vec<TaskNode>,
}

impl TaskGraph {
    pub fn from_tasks(tasks: &[TaskConfig]) -> Result<Self, String> {
        let known: BTreeSet<_> = tasks.iter().map(|task| task.id.clone()).collect();
        for task in tasks {
            for dependency in &task.depends_on {
                if !known.contains(dependency) {
                    return Err(format!(
                        "task {:?} depends on unknown task {:?}",
                        task.id.as_str(),
                        dependency.as_str()
                    ));
                }
            }
        }
        Ok(Self {
            nodes: tasks
                .iter()
                .map(|task| TaskNode {
                    id: task.id.clone(),
                    depends_on: task.depends_on.clone(),
                })
                .collect(),
        })
    }

    pub fn topological_order(&self) -> Result<Vec<TaskId>, String> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Mark {
            Visiting,
            Done,
        }

        fn visit(
            id: &TaskId,
            dependencies: &BTreeMap<TaskId, Vec<TaskId>>,
            marks: &mut BTreeMap<TaskId, Mark>,
            stack: &mut Vec<TaskId>,
            output: &mut Vec<TaskId>,
        ) -> Result<(), String> {
            match marks.get(id) {
                Some(Mark::Done) => return Ok(()),
                Some(Mark::Visiting) => {
                    let start = stack.iter().position(|item| item == id).unwrap_or(0);
                    let mut cycle: Vec<String> =
                        stack[start..].iter().map(ToString::to_string).collect();
                    cycle.push(id.to_string());
                    return Err(format!("task dependency cycle: {}", cycle.join(" -> ")));
                }
                None => {}
            }
            marks.insert(id.clone(), Mark::Visiting);
            stack.push(id.clone());
            if let Some(items) = dependencies.get(id) {
                for dependency in items {
                    visit(dependency, dependencies, marks, stack, output)?;
                }
            }
            stack.pop();
            marks.insert(id.clone(), Mark::Done);
            output.push(id.clone());
            Ok(())
        }

        let dependencies: BTreeMap<_, _> = self
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node.depends_on.clone()))
            .collect();
        let mut marks = BTreeMap::new();
        let mut output = Vec::new();
        for id in dependencies.keys() {
            visit(id, &dependencies, &mut marks, &mut Vec::new(), &mut output)?;
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, dependencies: &[&str]) -> TaskConfig {
        TaskConfig {
            id: TaskId::from(id),
            kind: TaskKind::Classification,
            labels: vec!["label".to_owned()],
            required: false,
            multi_label: false,
            depends_on: dependencies.iter().copied().map(TaskId::from).collect(),
            validators: Vec::new(),
            refiners: Vec::new(),
            target_task: None,
            target_labels: Vec::new(),
            attributes: BTreeMap::new(),
        }
    }

    #[test]
    fn graph_orders_dependencies_first() {
        let graph = TaskGraph::from_tasks(&[task("last", &["first"]), task("first", &[])])
            .expect("valid graph");
        assert_eq!(
            graph.topological_order().expect("acyclic"),
            vec![TaskId::from("first"), TaskId::from("last")]
        );
    }

    #[test]
    fn graph_reports_cycle() {
        let graph = TaskGraph::from_tasks(&[task("a", &["b"]), task("b", &["a"])])
            .expect("known dependencies");
        assert!(graph.topological_order().is_err());
    }

    #[test]
    fn yaml_reports_unknown_field_path() {
        let yaml = r"
version: 1
project: { name: demo, skill: dummy, skill_version: '1', unexpected: true }
dataset: { root: images }
runtime: {}
tasks: []
review: { auto_accept_confidence: 0.9, force_review_below: 0.5 }
export: { formats: [] }
";
        let error = ProjectSchema::from_yaml(yaml).expect_err("unknown field must fail");
        assert!(error.path.contains("project"));
    }
}
