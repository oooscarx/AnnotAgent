use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{DomainSkill, ValidationCatalog};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("skill {0:?} is already registered")]
    DuplicateSkill(String),
    #[error("skill manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("unknown skill {0:?}")]
    UnknownSkill(String),
    #[error("extension {kind} {id:?} is registered more than once")]
    DuplicateExtension { kind: &'static str, id: String },
}

#[derive(Default)]
pub struct SkillRegistry {
    skills: BTreeMap<String, Arc<dyn DomainSkill>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MergedVisualProfile {
    pub values: BTreeMap<String, serde_json::Value>,
    pub sources: BTreeMap<String, String>,
    pub conflicts: Vec<VisualProfileConflict>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualProfileConflict {
    pub key: String,
    pub kept_skill: String,
    pub ignored_skill: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NamespacedSkillExtensions {
    pub nodes: Vec<String>,
    pub tools: Vec<String>,
    pub validators: Vec<String>,
    pub refiners: Vec<String>,
}

impl SkillRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, skill: Arc<dyn DomainSkill>) -> Result<(), RegistryError> {
        let id = skill.id().to_owned();
        if self.skills.contains_key(&id) {
            return Err(RegistryError::DuplicateSkill(id));
        }
        if skill.manifest().id != id {
            return Err(RegistryError::InvalidManifest(format!(
                "trait id {id:?} does not match manifest id {:?}",
                skill.manifest().id
            )));
        }
        if let Some(issue) = skill.manifest().validate().into_iter().next() {
            return Err(RegistryError::InvalidManifest(issue.to_string()));
        }
        ensure_unique(
            "tool",
            skill
                .tool_factories()
                .into_iter()
                .map(|tool| tool.definition().name),
        )?;
        ensure_unique(
            "validator",
            skill
                .validators()
                .into_iter()
                .map(|validator| validator.id().to_owned()),
        )?;
        ensure_unique(
            "refiner",
            skill
                .refiners()
                .into_iter()
                .map(|refiner| refiner.id().to_owned()),
        )?;
        self.skills.insert(id, skill);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Arc<dyn DomainSkill>, RegistryError> {
        self.skills
            .get(id)
            .cloned()
            .ok_or_else(|| RegistryError::UnknownSkill(id.to_owned()))
    }

    #[must_use]
    pub fn list(&self) -> Vec<Arc<dyn DomainSkill>> {
        self.skills.values().cloned().collect()
    }

    #[must_use]
    pub fn validation_catalog(&self) -> ValidationCatalog {
        let mut catalog = ValidationCatalog::default();
        for skill in self.skills.values() {
            catalog.validators.extend(
                skill
                    .validators()
                    .into_iter()
                    .map(|validator| validator.id().to_owned()),
            );
            catalog.refiners.extend(
                skill
                    .refiners()
                    .into_iter()
                    .map(|refiner| refiner.id().to_owned()),
            );
            catalog.resources.extend(
                skill
                    .manifest()
                    .summary_resources
                    .iter()
                    .chain(skill.manifest().task_resources.values().flatten())
                    .cloned(),
            );
        }
        catalog
    }

    pub fn validation_catalog_for(
        &self,
        enabled_skill_ids: &[String],
    ) -> Result<ValidationCatalog, RegistryError> {
        let mut catalog = ValidationCatalog::default();
        let use_namespace = enabled_skill_ids.len() > 1;
        for skill_id in enabled_skill_ids {
            let skill = self.get(skill_id)?;
            catalog
                .validators
                .extend(skill.validators().into_iter().map(|validator| {
                    if use_namespace {
                        format!("{skill_id}.{}", validator.id())
                    } else {
                        validator.id().to_owned()
                    }
                }));
            catalog
                .refiners
                .extend(skill.refiners().into_iter().map(|refiner| {
                    if use_namespace {
                        format!("{skill_id}.{}", refiner.id())
                    } else {
                        refiner.id().to_owned()
                    }
                }));
            catalog.resources.extend(
                skill
                    .manifest()
                    .summary_resources
                    .iter()
                    .chain(skill.manifest().task_resources.values().flatten())
                    .map(|resource| {
                        if use_namespace {
                            format!("{skill_id}.{resource}")
                        } else {
                            resource.clone()
                        }
                    }),
            );
        }
        Ok(catalog)
    }

    /// Merges in sorted Skill-id order. The first owner wins and every collision is reported.
    pub fn merge_visual_profiles(
        &self,
        enabled_skill_ids: &[String],
    ) -> Result<MergedVisualProfile, RegistryError> {
        let mut ids = enabled_skill_ids.to_vec();
        ids.sort();
        ids.dedup();
        let mut merged = MergedVisualProfile::default();
        for skill_id in ids {
            let skill = self.get(&skill_id)?;
            for (key, value) in &skill.manifest().visual_profile {
                if let Some(kept_skill) = merged.sources.get(key) {
                    merged.conflicts.push(VisualProfileConflict {
                        key: key.clone(),
                        kept_skill: kept_skill.clone(),
                        ignored_skill: skill_id.clone(),
                    });
                } else {
                    merged.values.insert(key.clone(), value.clone());
                    merged.sources.insert(key.clone(), skill_id.clone());
                }
            }
        }
        Ok(merged)
    }

    pub fn namespaced_extensions_for(
        &self,
        enabled_skill_ids: &[String],
    ) -> Result<NamespacedSkillExtensions, RegistryError> {
        let mut ids = enabled_skill_ids.to_vec();
        ids.sort();
        ids.dedup();
        let mut extensions = NamespacedSkillExtensions::default();
        for skill_id in ids {
            let skill = self.get(&skill_id)?;
            extensions.nodes.extend(
                skill
                    .task_templates()
                    .into_iter()
                    .map(|node| format!("{skill_id}.{}", node.id)),
            );
            extensions.tools.extend(
                skill
                    .tool_factories()
                    .into_iter()
                    .map(|tool| format!("{skill_id}.{}", tool.definition().name)),
            );
            extensions.validators.extend(
                skill
                    .validators()
                    .into_iter()
                    .map(|validator| format!("{skill_id}.{}", validator.id())),
            );
            extensions.refiners.extend(
                skill
                    .refiners()
                    .into_iter()
                    .map(|refiner| format!("{skill_id}.{}", refiner.id())),
            );
        }
        Ok(extensions)
    }
}

fn ensure_unique(
    kind: &'static str,
    ids: impl IntoIterator<Item = String>,
) -> Result<(), RegistryError> {
    let mut known = std::collections::BTreeSet::new();
    for id in ids {
        if !known.insert(id.clone()) {
            return Err(RegistryError::DuplicateExtension { kind, id });
        }
    }
    Ok(())
}
