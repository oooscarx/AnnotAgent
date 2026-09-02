//! Durable plugin installation, version, weight and reference registry.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read as _,
    path::{Path, PathBuf},
};

use annotagent_core::{ModelAvailability, ModelCapability};
use annotagent_plugin_api::{
    PluginId, PluginImplementationStatus, PluginManifest, PluginModelReference, PluginStatus,
    PluginTestReport, PluginVersion, Sha256Digest,
};
use annotagent_plugin_host::{
    PackageSignatureState, PluginPackageError, current_target, verify_package,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REGISTRY_SCHEMA_VERSION: u32 = 1;
const MAX_WEIGHT_BYTES: u64 = 32 * 1024 * 1024 * 1024;

fn model_checkpoint_identity(mut weights: Vec<&PluginWeightSet>) -> Option<Sha256Digest> {
    if weights.is_empty() {
        return None;
    }
    weights.sort_by(|left, right| left.component_id.cmp(&right.component_id));
    if weights.len() == 1 && weights[0].component_id == "default" {
        return Some(weights[0].checkpoint_sha256.clone());
    }
    let identity = weights
        .iter()
        .map(|weight| format!("{}:{}", weight.component_id, weight.checkpoint_sha256))
        .collect::<Vec<_>>()
        .join("\n");
    Some(Sha256Digest::of_bytes(identity.as_bytes()))
}

#[derive(Debug, Error)]
pub enum PluginRegistryError {
    #[error("plugin package failed verification: {0}")]
    Package(#[from] PluginPackageError),
    #[error("plugin registry io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("plugin registry serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("plugin installation approval is incomplete: {0}")]
    Approval(String),
    #[error("plugin version is already installed")]
    AlreadyInstalled,
    #[error("plugin version is not installed")]
    NotInstalled,
    #[error("plugin model is not declared")]
    UnknownModel,
    #[error("plugin weight input is invalid: {0}")]
    InvalidWeight(String),
    #[error("plugin version is referenced and cannot be uninstalled: {0}")]
    Referenced(String),
    #[error("plugin state transition is invalid: {0}")]
    InvalidTransition(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallApproval {
    pub permissions_reviewed: bool,
    pub code_license_accepted: bool,
    pub weight_license_accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginInstallation {
    pub manifest: PluginManifest,
    pub package_digest: Sha256Digest,
    pub signature: String,
    pub status: PluginStatus,
    pub enabled: bool,
    pub installation_root: PathBuf,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_test: Option<PluginTestReport>,
}

impl PluginInstallation {
    #[must_use]
    pub fn key(&self) -> String {
        installation_key(&self.manifest.id, &self.manifest.version)
    }

    #[must_use]
    pub fn weights_ready(&self, weight_sets: &[PluginWeightSet]) -> bool {
        !self.manifest.weights.required
            || self.manifest.models.iter().all(|model| {
                let required = self
                    .manifest
                    .weights
                    .components
                    .iter()
                    .filter(|component| component.model_id == model.id)
                    .map(|component| component.id.as_str())
                    .collect::<Vec<_>>();
                if required.is_empty() {
                    has_weight_component(self, weight_sets, &model.id, "default")
                } else {
                    required.iter().all(|component| {
                        has_weight_component(self, weight_sets, &model.id, component)
                    })
                }
            })
    }
}

fn has_weight_component(
    installation: &PluginInstallation,
    weight_sets: &[PluginWeightSet],
    model_id: &str,
    component_id: &str,
) -> bool {
    weight_sets.iter().any(|weights| {
        weights.plugin_id == installation.manifest.id
            && weights.plugin_version == installation.manifest.version
            && weights.model_id == model_id
            && weights.component_id == component_id
    })
}

fn default_weight_component_id() -> String {
    "default".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginWeightSet {
    pub plugin_id: PluginId,
    pub plugin_version: PluginVersion,
    pub model_id: String,
    #[serde(default = "default_weight_component_id")]
    pub component_id: String,
    pub checkpoint_sha256: Sha256Digest,
    pub original_filename: String,
    pub stored_path: PathBuf,
    pub size_bytes: u64,
    pub provisioned_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginReference {
    pub plugin_id: PluginId,
    pub plugin_version: PluginVersion,
    pub kind: String,
    pub location: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLicenseAcceptance {
    pub plugin_id: PluginId,
    pub plugin_version: PluginVersion,
    pub code_license: String,
    pub weight_license: String,
    pub accepted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginEvent {
    pub plugin_id: PluginId,
    pub plugin_version: PluginVersion,
    pub event: String,
    pub detail: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginBackedModelProfile {
    pub reference: PluginModelReference,
    pub display_name: String,
    pub capabilities: BTreeSet<ModelCapability>,
    pub availability: ModelAvailability,
    pub plugin_status: PluginStatus,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct RegistryState {
    schema_version: u32,
    installations: BTreeMap<String, PluginInstallation>,
    weight_sets: Vec<PluginWeightSet>,
    references: Vec<PluginReference>,
    license_acceptances: Vec<PluginLicenseAcceptance>,
    events: Vec<PluginEvent>,
}

impl Default for RegistryState {
    fn default() -> Self {
        Self {
            schema_version: REGISTRY_SCHEMA_VERSION,
            installations: BTreeMap::new(),
            weight_sets: Vec::new(),
            references: Vec::new(),
            license_acceptances: Vec::new(),
            events: Vec::new(),
        }
    }
}

pub struct PluginRegistry {
    data_root: PathBuf,
    state: RegistryState,
}

impl PluginRegistry {
    pub fn open(data_root: impl Into<PathBuf>) -> Result<Self, PluginRegistryError> {
        let data_root = data_root.into();
        std::fs::create_dir_all(&data_root)?;
        let state_path = data_root.join("plugin-registry.json");
        let state = if state_path.is_file() {
            let state: RegistryState = serde_json::from_slice(&std::fs::read(&state_path)?)?;
            if state.schema_version != REGISTRY_SCHEMA_VERSION {
                return Err(PluginRegistryError::InvalidTransition(format!(
                    "unsupported registry schema {}",
                    state.schema_version
                )));
            }
            state
        } else {
            RegistryState::default()
        };
        Ok(Self { data_root, state })
    }

    #[must_use]
    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    pub fn install(
        &mut self,
        package: &Path,
        approval: &InstallApproval,
    ) -> Result<PluginInstallation, PluginRegistryError> {
        if !approval.permissions_reviewed || !approval.code_license_accepted {
            return Err(PluginRegistryError::Approval(
                "permissions and code license require explicit review".to_owned(),
            ));
        }
        let verified = verify_package(package)?;
        if verified.manifest.weights.required && !approval.weight_license_accepted {
            return Err(PluginRegistryError::Approval(
                "the declared weight license requires explicit acceptance".to_owned(),
            ));
        }
        let key = installation_key(&verified.manifest.id, &verified.manifest.version);
        if self.state.installations.contains_key(&key) {
            return Err(PluginRegistryError::AlreadyInstalled);
        }
        let destination = installation_root(
            &self.data_root,
            &verified.manifest.id,
            &verified.manifest.version,
        );
        if destination.exists() {
            return Err(PluginRegistryError::AlreadyInstalled);
        }
        let parent = destination.parent().ok_or_else(|| {
            PluginRegistryError::InvalidTransition("installation has no parent".to_owned())
        })?;
        std::fs::create_dir_all(parent)?;
        let staging = parent.join(format!(".installing-{}", uuid::Uuid::new_v4()));
        verified.extract_to(&staging)?;
        std::fs::rename(&staging, &destination)?;
        let now = Utc::now();
        let status =
            if verified.manifest.implementation_status == PluginImplementationStatus::Unsupported {
                PluginStatus::UnsupportedPlatform
            } else if verified.manifest.weights.required {
                PluginStatus::NeedsWeights
            } else {
                PluginStatus::Installed
            };
        let enabled =
            verified.manifest.implementation_status != PluginImplementationStatus::Unsupported;
        let installation = PluginInstallation {
            manifest: verified.manifest,
            package_digest: verified.package_digest,
            signature: match verified.signature {
                PackageSignatureState::Unsigned => "unsigned",
                PackageSignatureState::PresentUnverified => "present_unverified",
            }
            .to_owned(),
            status,
            enabled,
            installation_root: destination,
            installed_at: now,
            updated_at: now,
            last_test: None,
        };
        self.state
            .license_acceptances
            .push(PluginLicenseAcceptance {
                plugin_id: installation.manifest.id.clone(),
                plugin_version: installation.manifest.version.clone(),
                code_license: installation.manifest.license.code.clone(),
                weight_license: installation.manifest.license.weights.clone(),
                accepted_at: now,
            });
        self.event(&installation, "installed", "package verified and installed");
        self.state.installations.insert(key, installation.clone());
        self.persist()?;
        Ok(installation)
    }

    #[must_use]
    pub fn list(&self) -> Vec<PluginInstallation> {
        self.state.installations.values().cloned().collect()
    }

    pub fn get(
        &self,
        plugin_id: &PluginId,
        version: &PluginVersion,
    ) -> Result<&PluginInstallation, PluginRegistryError> {
        self.state
            .installations
            .get(&installation_key(plugin_id, version))
            .ok_or(PluginRegistryError::NotInstalled)
    }

    pub fn executable(
        &self,
        plugin_id: &PluginId,
        version: &PluginVersion,
    ) -> Result<PathBuf, PluginRegistryError> {
        let installation = self.get(plugin_id, version)?;
        let path = installation.installation_root.join(
            installation
                .manifest
                .runtime
                .entrypoint
                .replace("{target}", &current_target()),
        );
        if !path.starts_with(&installation.installation_root) || !path.is_file() {
            return Err(PluginRegistryError::InvalidTransition(
                "installed executable is missing or outside its version root".to_owned(),
            ));
        }
        Ok(path)
    }

    pub fn provision_local_weights(
        &mut self,
        plugin_id: &PluginId,
        version: &PluginVersion,
        model_id: &str,
        source: &Path,
        expected: Option<&Sha256Digest>,
    ) -> Result<PluginWeightSet, PluginRegistryError> {
        let installation = self.get(plugin_id, version)?;
        let components = installation
            .manifest
            .weights
            .components
            .iter()
            .filter(|component| component.model_id == model_id)
            .collect::<Vec<_>>();
        let component_id = match components.as_slice() {
            [] => "default".to_owned(),
            [component] => component.id.clone(),
            _ => {
                return Err(PluginRegistryError::InvalidWeight(
                    "model requires multiple weight components; provide a component id".to_owned(),
                ));
            }
        };
        self.provision_local_weight_component(
            plugin_id,
            version,
            model_id,
            &component_id,
            source,
            expected,
        )
    }

    pub fn provision_local_weight_component(
        &mut self,
        plugin_id: &PluginId,
        version: &PluginVersion,
        model_id: &str,
        component_id: &str,
        source: &Path,
        expected: Option<&Sha256Digest>,
    ) -> Result<PluginWeightSet, PluginRegistryError> {
        let installation = self.get(plugin_id, version)?.clone();
        if installation.manifest.implementation_status == PluginImplementationStatus::Unsupported {
            return Err(PluginRegistryError::InvalidTransition(
                "unsupported plugin versions cannot provision model weights".to_owned(),
            ));
        }
        if !installation
            .manifest
            .models
            .iter()
            .any(|model| model.id == model_id)
        {
            return Err(PluginRegistryError::UnknownModel);
        }
        let declared_components = installation
            .manifest
            .weights
            .components
            .iter()
            .filter(|component| component.model_id == model_id)
            .collect::<Vec<_>>();
        if declared_components.is_empty() {
            if component_id != "default" {
                return Err(PluginRegistryError::InvalidWeight(
                    "single-file model only accepts the default component".to_owned(),
                ));
            }
        } else if !declared_components
            .iter()
            .any(|component| component.id == component_id)
        {
            return Err(PluginRegistryError::InvalidWeight(format!(
                "unknown weight component {component_id} for model {model_id}"
            )));
        }
        let metadata = std::fs::metadata(source)?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_WEIGHT_BYTES {
            return Err(PluginRegistryError::InvalidWeight(
                "weight must be a bounded non-empty regular file".to_owned(),
            ));
        }
        let digest = hash_file(source)?;
        let declared_expected = declared_components
            .iter()
            .find(|component| component.id == component_id)
            .and_then(|component| component.sha256.as_ref());
        if expected
            .or(declared_expected)
            .is_some_and(|expected| expected != &digest)
        {
            return Err(PluginRegistryError::InvalidWeight(
                "checkpoint digest does not match the expected identity".to_owned(),
            ));
        }
        let filename = source
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .ok_or_else(|| {
                PluginRegistryError::InvalidWeight("weight filename is not UTF-8".to_owned())
            })?
            .to_owned();
        let stored_filename = declared_components
            .iter()
            .find(|component| component.id == component_id)
            .map_or(filename.as_str(), |component| component.filename.as_str());
        let directory = self
            .data_root
            .join("model-cache")
            .join(plugin_id.as_str())
            .join(version.to_string())
            .join(model_id)
            .join(digest.as_str());
        std::fs::create_dir_all(&directory)?;
        let destination = directory.join(stored_filename);
        if !destination.exists() {
            let temporary = directory.join(format!(".provisioning-{}", uuid::Uuid::new_v4()));
            std::fs::copy(source, &temporary)?;
            if hash_file(&temporary)? != digest {
                return Err(PluginRegistryError::InvalidWeight(
                    "copied checkpoint failed identity verification".to_owned(),
                ));
            }
            std::fs::rename(temporary, &destination)?;
        }
        let weights = PluginWeightSet {
            plugin_id: plugin_id.clone(),
            plugin_version: version.clone(),
            model_id: model_id.to_owned(),
            component_id: component_id.to_owned(),
            checkpoint_sha256: digest,
            original_filename: filename,
            stored_path: destination,
            size_bytes: metadata.len(),
            provisioned_at: Utc::now(),
        };
        self.state.weight_sets.retain(|existing| {
            !(existing.plugin_id == *plugin_id
                && existing.plugin_version == *version
                && existing.model_id == model_id
                && existing.component_id == component_id)
        });
        self.state.weight_sets.push(weights.clone());
        let weights_ready = self
            .state
            .installations
            .get(&installation_key(plugin_id, version))
            .ok_or(PluginRegistryError::NotInstalled)?
            .weights_ready(&self.state.weight_sets);
        self.set_status(
            plugin_id,
            version,
            if weights_ready {
                PluginStatus::Installed
            } else {
                PluginStatus::NeedsWeights
            },
        )?;
        Ok(weights)
    }

    pub fn record_test(
        &mut self,
        report: PluginTestReport,
    ) -> Result<PluginStatus, PluginRegistryError> {
        let key = installation_key(&report.plugin_id, &report.plugin_version);
        let installation = self
            .state
            .installations
            .get(&key)
            .ok_or(PluginRegistryError::NotInstalled)?;
        if installation.manifest.implementation_status == PluginImplementationStatus::Unsupported {
            return Err(PluginRegistryError::InvalidTransition(
                "unsupported plugin versions cannot run a readiness smoke test".to_owned(),
            ));
        }
        let weights_ready = installation.weights_ready(&self.state.weight_sets);
        let smoke_passed = report
            .checks
            .iter()
            .any(|check| check.name == "sample inference" && check.passed);
        let status = if report.passed && smoke_passed && weights_ready {
            PluginStatus::Ready
        } else if !weights_ready {
            PluginStatus::NeedsWeights
        } else {
            PluginStatus::FailedSmokeTest
        };
        let installation = self
            .state
            .installations
            .get_mut(&key)
            .ok_or(PluginRegistryError::NotInstalled)?;
        installation.last_test = Some(report);
        installation.status = status;
        installation.updated_at = Utc::now();
        let clone = installation.clone();
        self.event(&clone, "tested", &format!("status={status:?}"));
        self.persist()?;
        Ok(status)
    }

    pub fn weights_root(
        &self,
        plugin_id: &PluginId,
        version: &PluginVersion,
    ) -> Result<PathBuf, PluginRegistryError> {
        self.get(plugin_id, version)?;
        let root = self
            .data_root
            .join("model-cache")
            .join(plugin_id.as_str())
            .join(version.to_string());
        std::fs::create_dir_all(&root)?;
        Ok(root)
    }

    pub fn disable(
        &mut self,
        plugin_id: &PluginId,
        version: &PluginVersion,
    ) -> Result<(), PluginRegistryError> {
        let installation = self
            .state
            .installations
            .get_mut(&installation_key(plugin_id, version))
            .ok_or(PluginRegistryError::NotInstalled)?;
        installation.enabled = false;
        installation.status = PluginStatus::Disabled;
        installation.updated_at = Utc::now();
        self.persist()?;
        Ok(())
    }

    pub fn enable(
        &mut self,
        plugin_id: &PluginId,
        version: &PluginVersion,
    ) -> Result<PluginStatus, PluginRegistryError> {
        let key = installation_key(plugin_id, version);
        let weights_ready = self
            .state
            .installations
            .get(&key)
            .ok_or(PluginRegistryError::NotInstalled)?
            .weights_ready(&self.state.weight_sets);
        let installation = self
            .state
            .installations
            .get_mut(&key)
            .ok_or(PluginRegistryError::NotInstalled)?;
        installation.enabled =
            installation.manifest.implementation_status != PluginImplementationStatus::Unsupported;
        installation.status = if installation.manifest.implementation_status
            == PluginImplementationStatus::Unsupported
        {
            PluginStatus::UnsupportedPlatform
        } else if !weights_ready {
            PluginStatus::NeedsWeights
        } else if installation
            .last_test
            .as_ref()
            .is_some_and(|test| test.passed)
        {
            PluginStatus::Ready
        } else {
            PluginStatus::Installed
        };
        installation.updated_at = Utc::now();
        let status = installation.status;
        self.persist()?;
        Ok(status)
    }

    pub fn add_reference(&mut self, reference: PluginReference) -> Result<(), PluginRegistryError> {
        self.get(&reference.plugin_id, &reference.plugin_version)?;
        if !self.state.references.contains(&reference) {
            self.state.references.push(reference);
            self.persist()?;
        }
        Ok(())
    }

    #[must_use]
    pub fn references(
        &self,
        plugin_id: &PluginId,
        version: &PluginVersion,
    ) -> Vec<PluginReference> {
        self.state
            .references
            .iter()
            .filter(|reference| {
                reference.plugin_id == *plugin_id && reference.plugin_version == *version
            })
            .cloned()
            .collect()
    }

    pub fn uninstall(
        &mut self,
        plugin_id: &PluginId,
        version: &PluginVersion,
    ) -> Result<(), PluginRegistryError> {
        let references = self.references(plugin_id, version);
        if !references.is_empty() {
            return Err(PluginRegistryError::Referenced(
                references
                    .iter()
                    .map(|reference| format!("{} {}", reference.kind, reference.location))
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
        let key = installation_key(plugin_id, version);
        let installation = self
            .state
            .installations
            .remove(&key)
            .ok_or(PluginRegistryError::NotInstalled)?;
        let expected = installation_root(&self.data_root, plugin_id, version);
        if installation.installation_root != expected
            || !expected.starts_with(self.data_root.join("plugins"))
        {
            return Err(PluginRegistryError::InvalidTransition(
                "refusing to remove an unexpected installation path".to_owned(),
            ));
        }
        if expected.exists() {
            std::fs::remove_dir_all(&expected)?;
        }
        self.state.weight_sets.retain(|weights| {
            weights.plugin_id != *plugin_id || weights.plugin_version != *version
        });
        self.persist()?;
        Ok(())
    }

    #[must_use]
    pub fn ready_models(&self) -> Vec<PluginBackedModelProfile> {
        self.state
            .installations
            .values()
            .flat_map(|installation| {
                installation.manifest.models.iter().map(move |model| {
                    let model_weights = self
                        .state
                        .weight_sets
                        .iter()
                        .filter(|weights| {
                            weights.plugin_id == installation.manifest.id
                                && weights.plugin_version == installation.manifest.version
                                && weights.model_id == model.id
                        })
                        .collect::<Vec<_>>();
                    let checkpoint = model_checkpoint_identity(model_weights);
                    let contract = Sha256Digest::of_bytes(
                        &serde_json::to_vec(model).expect("model contract is serializable"),
                    );
                    PluginBackedModelProfile {
                        reference: PluginModelReference {
                            plugin_id: installation.manifest.id.clone(),
                            plugin_version: installation.manifest.version.clone(),
                            package_digest: installation.package_digest.clone(),
                            plugin_api_version: installation.manifest.plugin_api.clone(),
                            protocol_version: annotagent_plugin_api::PLUGIN_PROTOCOL_VERSION
                                .to_owned(),
                            model_id: model.id.clone(),
                            model_profile_revision: 1,
                            checkpoint_sha256: checkpoint,
                            capability_contract_hash: contract,
                        },
                        display_name: model.display_name.clone(),
                        capabilities: model.capabilities.iter().copied().collect(),
                        availability: if installation.status == PluginStatus::Ready {
                            ModelAvailability::Available
                        } else if installation.status == PluginStatus::NeedsWeights {
                            ModelAvailability::MissingWeights
                        } else if installation.status == PluginStatus::Disabled {
                            ModelAvailability::Disabled
                        } else {
                            ModelAvailability::Unknown
                        },
                        plugin_status: installation.status,
                        enabled: installation.enabled,
                    }
                })
            })
            .collect()
    }

    fn set_status(
        &mut self,
        plugin_id: &PluginId,
        version: &PluginVersion,
        status: PluginStatus,
    ) -> Result<(), PluginRegistryError> {
        let installation = self
            .state
            .installations
            .get_mut(&installation_key(plugin_id, version))
            .ok_or(PluginRegistryError::NotInstalled)?;
        installation.status = status;
        installation.updated_at = Utc::now();
        self.persist()?;
        Ok(())
    }

    fn event(&mut self, installation: &PluginInstallation, event: &str, detail: &str) {
        self.state.events.push(PluginEvent {
            plugin_id: installation.manifest.id.clone(),
            plugin_version: installation.manifest.version.clone(),
            event: event.to_owned(),
            detail: detail.to_owned(),
            created_at: Utc::now(),
        });
    }

    fn persist(&self) -> Result<(), PluginRegistryError> {
        let path = self.data_root.join("plugin-registry.json");
        let temporary = self.data_root.join("plugin-registry.json.partial");
        std::fs::write(&temporary, serde_json::to_vec_pretty(&self.state)?)?;
        std::fs::rename(temporary, path)?;
        Ok(())
    }
}

fn installation_key(plugin_id: &PluginId, version: &PluginVersion) -> String {
    format!("{plugin_id}@{version}")
}

fn installation_root(root: &Path, plugin_id: &PluginId, version: &PluginVersion) -> PathBuf {
    root.join("plugins")
        .join(plugin_id.as_str())
        .join(version.to_string())
}

fn hash_file(path: &Path) -> Result<Sha256Digest, PluginRegistryError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 128 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Sha256Digest::parse(format!("{:x}", hasher.finalize()))
        .map_err(|error| PluginRegistryError::InvalidWeight(error.to_string()))
}

#[must_use]
pub fn default_plugin_data_root() -> PathBuf {
    if let Some(path) = std::env::var_os("ANNOTAGENT_DATA_DIR") {
        return PathBuf::from(path);
    }
    if cfg!(target_os = "macos") {
        return std::env::var_os("HOME").map_or_else(
            || PathBuf::from(".annotagent-data"),
            |home| PathBuf::from(home).join("Library/Application Support/AnnotAgent"),
        );
    }
    if cfg!(target_os = "windows") {
        return std::env::var_os("APPDATA").map_or_else(
            || PathBuf::from(".annotagent-data"),
            |path| PathBuf::from(path).join("AnnotAgent"),
        );
    }
    std::env::var_os("XDG_DATA_HOME").map_or_else(
        || {
            std::env::var_os("HOME").map_or_else(
                || PathBuf::from(".annotagent-data"),
                |home| PathBuf::from(home).join(".local/share/annotagent"),
            )
        },
        |path| PathBuf::from(path).join("annotagent"),
    )
}

#[cfg(test)]
mod tests {
    use annotagent_plugin_api::{
        CommercialUseDeclaration, PluginWeightsManifest, WeightProvisioning,
    };
    use annotagent_plugin_host::pack_directory;

    use super::*;

    fn package(
        temp: &tempfile::TempDir,
        version: &str,
        weights_required: bool,
    ) -> (PathBuf, PluginManifest) {
        let source = temp.path().join(format!("source-{version}"));
        let binary = source
            .join("bin")
            .join(current_target())
            .join("annotagent-plugin-dummy-detector");
        std::fs::create_dir_all(binary.parent().expect("parent")).expect("dirs");
        let mut manifest = PluginManifest::from_toml(include_str!(
            "../../../plugins/dummy-detector/annotagent-plugin.toml"
        ))
        .expect("manifest");
        manifest.version = PluginVersion::parse(version).expect("version");
        manifest.weights = PluginWeightsManifest {
            bundled: false,
            required: weights_required,
            provisioning: if weights_required {
                WeightProvisioning::LocalPath
            } else {
                WeightProvisioning::None
            },
            checkpoint_sha256_required: weights_required,
            components: Vec::new(),
        };
        manifest.license.commercial_use = CommercialUseDeclaration::Allowed;
        std::fs::write(
            source.join(annotagent_plugin_api::PLUGIN_MANIFEST_FILE),
            manifest.to_toml().expect("toml"),
        )
        .expect("manifest");
        std::fs::write(binary, b"fixture-binary").expect("binary");
        let output = temp.path().join(format!("plugin-{version}.annotplugin"));
        pack_directory(&source, &output).expect("pack");
        (output, manifest)
    }

    fn approval() -> InstallApproval {
        InstallApproval {
            permissions_reviewed: true,
            code_license_accepted: true,
            weight_license_accepted: true,
        }
    }

    #[test]
    fn versions_coexist_and_references_protect_uninstall() {
        let temp = tempfile::tempdir().expect("temp");
        let (v1, manifest_v1) = package(&temp, "1.0.0", false);
        let (v2, _) = package(&temp, "1.1.0", false);
        let mut registry = PluginRegistry::open(temp.path().join("data")).expect("registry");
        registry.install(&v1, &approval()).expect("install v1");
        registry.install(&v2, &approval()).expect("install v2");
        assert_eq!(registry.list().len(), 2);
        registry
            .add_reference(PluginReference {
                plugin_id: manifest_v1.id.clone(),
                plugin_version: manifest_v1.version.clone(),
                kind: "published_workflow".to_owned(),
                location: "generic@v1".to_owned(),
                created_at: Utc::now(),
            })
            .expect("reference");
        assert!(
            registry
                .uninstall(&manifest_v1.id, &manifest_v1.version)
                .is_err()
        );
    }

    #[test]
    fn missing_weights_stays_unready_until_provision_and_passed_test() {
        let temp = tempfile::tempdir().expect("temp");
        let (package, manifest) = package(&temp, "2.0.0", true);
        let mut registry = PluginRegistry::open(temp.path().join("data")).expect("registry");
        let installed = registry.install(&package, &approval()).expect("install");
        assert_eq!(installed.status, PluginStatus::NeedsWeights);
        assert_ne!(
            registry.ready_models()[0].availability,
            ModelAvailability::Available
        );

        let weights = temp.path().join("weights.bin");
        std::fs::write(&weights, b"legal local fixture weights").expect("weights");
        let weight_set = registry
            .provision_local_weights(
                &manifest.id,
                &manifest.version,
                &manifest.models[0].id,
                &weights,
                None,
            )
            .expect("provision");
        assert_ne!(weight_set.stored_path, weights);
        let now = Utc::now();
        let status = registry
            .record_test(PluginTestReport {
                plugin_id: manifest.id,
                plugin_version: manifest.version,
                passed: true,
                checks: vec![annotagent_plugin_api::PluginTestCheck {
                    name: "sample inference".to_owned(),
                    passed: true,
                    detail: "typed fixture".to_owned(),
                }],
                started_at: now,
                finished_at: now,
            })
            .expect("test");
        assert_eq!(status, PluginStatus::Ready);
        assert_eq!(
            registry.ready_models()[0].availability,
            ModelAvailability::Available
        );

        let reopened = PluginRegistry::open(temp.path().join("data")).expect("reopen");
        assert_eq!(reopened.list()[0].status, PluginStatus::Ready);
    }
}
