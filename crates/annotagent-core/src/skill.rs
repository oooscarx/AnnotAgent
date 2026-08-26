//! Declarative skill metadata.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{ConfigIssue, TaskId};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillManifest {
    pub version: u32,
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub rust_implementation: Option<String>,
    #[serde(default)]
    pub summary_resources: Vec<String>,
    #[serde(default)]
    pub task_resources: BTreeMap<TaskId, Vec<String>>,
    #[serde(default)]
    pub correction_taxonomy: Vec<String>,
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
            || !self.id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
        {
            issues.push(ConfigIssue {
                path: "id".to_owned(),
                message: "skill id must be a non-empty portable identifier".to_owned(),
            });
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
