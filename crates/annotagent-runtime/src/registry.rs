use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{DomainSkill, ValidationCatalog};
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
        }
        catalog
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
