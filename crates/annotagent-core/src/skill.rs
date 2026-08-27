//! Declarative skill metadata.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{ConfigIssue, TaskId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillKind {
    Capability,
    Domain,
    Pack,
}

impl Default for SkillKind {
    fn default() -> Self {
        Self::Domain
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillDependency {
    pub id: String,
    /// Exact version for Alpha. A future manifest schema may add semver ranges explicitly.
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillManifest {
    /// Manifest schema version.
    pub version: u32,
    pub id: String,
    #[serde(default)]
    pub kind: SkillKind,
    #[serde(default = "default_skill_version")]
    pub skill_version: String,
    pub display_name: String,
    pub description: String,
    pub rust_implementation: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<SkillDependency>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub nodes: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub validators: Vec<String>,
    #[serde(default)]
    pub policies: Vec<String>,
    #[serde(default)]
    pub templates: Vec<String>,
    #[serde(default)]
    pub summary_resources: Vec<String>,
    #[serde(default)]
    pub task_resources: BTreeMap<TaskId, Vec<String>>,
    #[serde(default)]
    pub correction_taxonomy: Vec<String>,
    /// Label or overlay id -> domain-neutral visual metadata (color, shape, line style, etc.).
    #[serde(default)]
    pub visual_profile: BTreeMap<String, serde_json::Value>,
}

fn default_skill_version() -> String {
    "1".to_owned()
}

impl SkillManifest {
    pub fn from_yaml(input: &str) -> Result<Self, ConfigIssue> {
        let deserializer = serde_yaml::Deserializer::from_str(input);
        serde_path_to_error::deserialize(deserializer).map_err(|error| ConfigIssue {
            path: error.path().to_string(),
            message: error.inner().to_string(),
        })
    }

    #[must_use]
    pub fn validate(&self) -> Vec<ConfigIssue> {
        let mut issues = Vec::new();
        if self.version != 1 {
            issues.push(ConfigIssue {
                path: "version".to_owned(),
                message: format!("unsupported skill manifest version {}", self.version),
            });
        }
        if self.id.is_empty()
            || self.id.starts_with('.')
            || self.id.ends_with('.')
            || self.id.contains("..")
            || !self.id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
            })
        {
            issues.push(ConfigIssue {
                path: "id".to_owned(),
                message: "skill id must be a non-empty portable identifier".to_owned(),
            });
        }
        if self.skill_version.trim().is_empty() {
            issues.push(ConfigIssue {
                path: "skill_version".to_owned(),
                message: "skill version cannot be empty".to_owned(),
            });
        }
        let mut dependencies = BTreeSet::new();
        for (index, dependency) in self.dependencies.iter().enumerate() {
            if dependency.id == self.id {
                issues.push(ConfigIssue {
                    path: format!("dependencies[{index}].id"),
                    message: "a Skill cannot depend on itself".to_owned(),
                });
            }
            if dependency.id.trim().is_empty() || dependency.version.trim().is_empty() {
                issues.push(ConfigIssue {
                    path: format!("dependencies[{index}]"),
                    message: "dependency id and version are required".to_owned(),
                });
            }
            if !dependencies.insert(&dependency.id) {
                issues.push(ConfigIssue {
                    path: format!("dependencies[{index}].id"),
                    message: format!("duplicate dependency {:?}", dependency.id),
                });
            }
        }
        let mut conflicts = BTreeSet::new();
        for (index, conflict) in self.conflicts.iter().enumerate() {
            if conflict == &self.id || !conflicts.insert(conflict) {
                issues.push(ConfigIssue {
                    path: format!("conflicts[{index}]"),
                    message: "conflicts must be unique and cannot name the Skill itself".to_owned(),
                });
            }
        }
        let mut seen = BTreeSet::new();
        for (index, kind) in self.correction_taxonomy.iter().enumerate() {
            if !seen.insert(kind) {
                issues.push(ConfigIssue {
                    path: format!("correction_taxonomy[{index}]"),
                    message: format!("duplicate correction kind {kind:?}"),
                });
            }
        }
        issues
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskTemplate {
    pub id: TaskId,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillResourceRequest {
    pub task_id: Option<TaskId>,
    pub resource_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillResource {
    pub name: String,
    pub media_type: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectionKind {
    pub code: String,
    pub description: String,
}
