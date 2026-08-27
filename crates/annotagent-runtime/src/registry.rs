use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    DomainSkill, Skill, SkillKind, SkillResource, SkillResourceRequest, ValidationCatalog,
};
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
    #[error("Skill {skill:?} requires {dependency:?} version {required:?}")]
    MissingDependency {
        skill: String,
        dependency: String,
        required: String,
    },
    #[error("Skill {skill:?} requires {dependency:?} version {required:?}, found {actual:?}")]
    DependencyVersion {
        skill: String,
        dependency: String,
        required: String,
        actual: String,
    },
    #[error("enabled Skills {left:?} and {right:?} conflict")]
    Conflict { left: String, right: String },
    #[error("Skill resource {resource:?} is not declared by {skill:?}")]
    UndeclaredResource { skill: String, resource: String },
    #[error("unsafe Skill resource path {0:?}")]
    UnsafeResource(String),
}

#[derive(Default)]
pub struct LayeredSkillRegistry {
    skills: BTreeMap<String, Arc<dyn Skill>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillCatalogEntry {
    pub id: String,
    pub version: String,
    pub kind: SkillKind,
    pub display_name: String,
    pub description: String,
    pub capabilities: Vec<String>,
}

impl LayeredSkillRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, skill: Arc<dyn Skill>) -> Result<(), RegistryError> {
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
        self.skills.insert(id, skill);
        Ok(())
    }

    #[must_use]
    pub fn catalog(&self) -> Vec<SkillCatalogEntry> {
        self.skills
            .values()
            .map(|skill| SkillCatalogEntry {
                id: skill.id().to_owned(),
                version: skill.manifest().skill_version.clone(),
                kind: skill.manifest().kind,
                display_name: skill.manifest().display_name.clone(),
                description: skill.manifest().description.clone(),
                capabilities: skill.manifest().capabilities.clone(),
            })
            .collect()
    }

    pub fn get(&self, id: &str) -> Result<Arc<dyn Skill>, RegistryError> {
        self.skills
            .get(id)
            .cloned()
            .ok_or_else(|| RegistryError::UnknownSkill(id.to_owned()))
    }

    #[must_use]
    pub fn list(&self) -> Vec<Arc<dyn Skill>> {
        self.skills.values().cloned().collect()
    }

    pub fn resolve_enabled(
        &self,
        enabled: &BTreeMap<String, String>,
    ) -> Result<Vec<Arc<dyn Skill>>, RegistryError> {
        let mut resolved = Vec::new();
        for (id, configured_version) in enabled {
            let skill = self.get(id)?;
            if skill.manifest().skill_version != *configured_version {
                return Err(RegistryError::DependencyVersion {
                    skill: id.clone(),
                    dependency: id.clone(),
                    required: configured_version.clone(),
                    actual: skill.manifest().skill_version.clone(),
                });
            }
            for dependency in &skill.manifest().dependencies {
                let Some(actual) = enabled.get(&dependency.id) else {
                    return Err(RegistryError::MissingDependency {
                        skill: id.clone(),
                        dependency: dependency.id.clone(),
                        required: dependency.version.clone(),
                    });
                };
                if actual != &dependency.version {
                    return Err(RegistryError::DependencyVersion {
                        skill: id.clone(),
                        dependency: dependency.id.clone(),
                        required: dependency.version.clone(),
                        actual: actual.clone(),
                    });
                }
            }
            for conflict in &skill.manifest().conflicts {
                if enabled.contains_key(conflict) {
                    return Err(RegistryError::Conflict {
                        left: id.clone(),
                        right: conflict.clone(),
                    });
                }
            }
            resolved.push(skill);
        }
        Ok(resolved)
    }

    pub fn load_resource(
        &self,
        skill_id: &str,
        request: &SkillResourceRequest,
    ) -> Result<Vec<SkillResource>, RegistryError> {
        let skill = self.get(skill_id)?;
        let Some(name) = request.resource_name.as_deref() else {
            return skill
                .resources(request)
                .map_err(|error| RegistryError::InvalidManifest(error.to_string()));
        };
        if name.is_empty()
            || std::path::Path::new(name).is_absolute()
            || name.split('/').any(|component| component == "..")
            || name.split('\\').any(|component| component == "..")
        {
            return Err(RegistryError::UnsafeResource(name.to_owned()));
        }
        let declared = skill
            .manifest()
            .summary_resources
            .iter()
            .chain(skill.manifest().task_resources.values().flatten())
            .any(|resource| resource == name);
        if !declared {
            return Err(RegistryError::UndeclaredResource {
                skill: skill_id.to_owned(),
                resource: name.to_owned(),
            });
        }
        skill
            .resources(request)
            .map_err(|error| RegistryError::InvalidManifest(error.to_string()))
    }
}

#[derive(Default)]
pub struct SkillRegistry {
    skills: BTreeMap<String, Arc<dyn DomainSkill>>,
    layered: LayeredSkillRegistry,
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

    pub fn register_layered(&mut self, skill: Arc<dyn Skill>) -> Result<(), RegistryError> {
        self.layered.register(skill)
    }

    pub fn get_layered(&self, id: &str) -> Result<Arc<dyn Skill>, RegistryError> {
        self.layered.get(id)
    }

    #[must_use]
    pub fn layered_catalog(&self) -> Vec<SkillCatalogEntry> {
        self.layered.catalog()
    }

    pub fn catalog_entry(&self, id: &str) -> Result<SkillCatalogEntry, RegistryError> {
        if let Ok(skill) = self.layered.get(id) {
            return Ok(SkillCatalogEntry {
                id: skill.id().to_owned(),
                version: skill.manifest().skill_version.clone(),
                kind: skill.manifest().kind,
                display_name: skill.manifest().display_name.clone(),
                description: skill.manifest().description.clone(),
                capabilities: skill.manifest().capabilities.clone(),
            });
        }
        let skill = self.get(id)?;
        Ok(SkillCatalogEntry {
            id: skill.id().to_owned(),
            version: skill.manifest().skill_version.clone(),
            kind: skill.manifest().kind,
            display_name: skill.manifest().display_name.clone(),
            description: skill.manifest().description.clone(),
            capabilities: skill.manifest().capabilities.clone(),
        })
    }

    pub fn resolve_layered_enabled(
        &self,
        enabled: &BTreeMap<String, String>,
    ) -> Result<Vec<Arc<dyn Skill>>, RegistryError> {
        self.layered.resolve_enabled(enabled)
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
            if let Ok(skill) = self.get(skill_id) {
                extend_validation_catalog(&mut catalog, skill.as_ref(), skill_id, use_namespace);
            } else {
                let skill = self.get_layered(skill_id)?;
                extend_layered_validation_catalog(
                    &mut catalog,
                    skill.as_ref(),
                    skill_id,
                    use_namespace,
                );
            }
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

fn extend_validation_catalog(
    catalog: &mut ValidationCatalog,
    skill: &dyn DomainSkill,
    skill_id: &str,
    use_namespace: bool,
) {
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
    extend_resources(catalog, skill.manifest(), skill_id, use_namespace);
}

fn extend_layered_validation_catalog(
    catalog: &mut ValidationCatalog,
    skill: &dyn Skill,
    skill_id: &str,
    use_namespace: bool,
) {
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
    extend_resources(catalog, skill.manifest(), skill_id, use_namespace);
}

fn extend_resources(
    catalog: &mut ValidationCatalog,
    manifest: &annotagent_core::SkillManifest,
    skill_id: &str,
    use_namespace: bool,
) {
    catalog.resources.extend(
        manifest
            .summary_resources
            .iter()
            .chain(manifest.task_resources.values().flatten())
            .map(|resource| {
                if use_namespace {
                    format!("{skill_id}.{resource}")
                } else {
                    resource.clone()
                }
            }),
    );
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
