use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use annotagent_core::{ModelAvailability, ModelCapability, ModelProfileId};
use annotagent_model_bundle::{
    ModelBundleId, ModelBundleManifest, ModelBundleSignatureState, ModelBundleStatus,
    ModelFileRole, ModelInstanceId, ModelInstanceStatus, Sha256Digest, SmokeTestResult,
    SmokeTestStatus, VerifiedModelBundle, verify_model_bundle,
};
use annotagent_plugin_api::{
    PluginId, PluginManifest, PluginRuntimeStatus, PluginVersion,
    Sha256Digest as PluginSha256Digest,
};
use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{
    ModelBundleCompatibility, ModelBundleCompatibilityResolver, ModelCatalog, ModelCatalogError,
    OnnxContractInspection, inspect_onnx_contract,
};

const MODEL_REGISTRY_SCHEMA_VERSION: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedLocalCatalogSource {
    pub catalog_id: String,
    pub root: PathBuf,
    pub catalog_sha256: Sha256Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseAcceptanceActor {
    LocalUser,
    Administrator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelLicenseAcceptance {
    pub bundle_id: ModelBundleId,
    pub bundle_version: Version,
    pub license_digest: Sha256Digest,
    pub accepted_at: DateTime<Utc>,
    pub accepted_by: LicenseAcceptanceActor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelBundleInstallSource {
    LocalImport,
    CuratedCatalog { catalog_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelBundleVerification {
    pub verified_at: DateTime<Utc>,
    pub bundle_digest: Sha256Digest,
    pub signature: ModelBundleSignatureState,
    pub file_count: usize,
    pub manifest_valid: bool,
    pub checksums_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledModelBundle {
    pub manifest: ModelBundleManifest,
    pub bundle_digest: Sha256Digest,
    pub status: ModelBundleStatus,
    pub source: ModelBundleInstallSource,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub verification: ModelBundleVerification,
    pub enabled: bool,
    pub content_root: PathBuf,
}

impl InstalledModelBundle {
    #[must_use]
    pub fn key(&self) -> String {
        bundle_key(&self.manifest.id, &self.manifest.version)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelBundleEvent {
    pub bundle_id: ModelBundleId,
    pub bundle_version: Version,
    pub event: String,
    pub detail: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledModelInstance {
    pub id: ModelInstanceId,
    pub plugin_id: PluginId,
    pub plugin_version: PluginVersion,
    pub plugin_package_digest: PluginSha256Digest,
    pub model_id: String,
    pub model_bundle_id: ModelBundleId,
    pub model_bundle_version: Version,
    pub model_bundle_digest: Sha256Digest,
    pub model_variant: String,
    pub model_file_digests: BTreeMap<ModelFileRole, Sha256Digest>,
    pub execution_provider: String,
    pub capability_contract_hash: Sha256Digest,
    pub status: ModelInstanceStatus,
    pub contract_inspection: OnnxContractInspection,
    pub smoke_test_id: Option<String>,
    pub smoke_test_result: Option<SmokeTestResult>,
    pub model_profile_id: ModelProfileId,
    pub model_profile_revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInstanceProfile {
    pub model_profile_id: ModelProfileId,
    pub model_profile_revision: u64,
    pub model_instance_id: ModelInstanceId,
    pub selection_id: String,
    pub display_name: String,
    pub capabilities: BTreeSet<ModelCapability>,
    pub availability: ModelAvailability,
    pub selectable: bool,
}

#[must_use]
pub fn model_instance_selection_id(id: ModelInstanceId) -> String {
    format!("model-instance:{id}")
}

#[must_use]
pub fn parse_model_instance_selection_id(value: &str) -> Option<ModelInstanceId> {
    value.strip_prefix("model-instance:")?.parse().ok()
}

pub struct BindModelInstanceRequest<'a> {
    pub plugin: &'a PluginManifest,
    pub plugin_package_digest: PluginSha256Digest,
    pub runtime_status: PluginRuntimeStatus,
    pub bundle_id: &'a ModelBundleId,
    pub bundle_version: &'a Version,
    pub model_id: &'a str,
    pub target: &'a str,
    pub execution_provider: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelBundleReference {
    pub bundle_id: ModelBundleId,
    pub bundle_version: Version,
    pub bundle_digest: Sha256Digest,
    pub kind: String,
    pub location: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModelBundleGcReport {
    pub removed_bundles: Vec<String>,
    pub removed_staging_entries: usize,
    pub reclaimed_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct ModelBundleRegistryState {
    schema_version: u32,
    catalogs: BTreeMap<String, ModelCatalog>,
    trusted_local_catalogs: BTreeMap<String, TrustedLocalCatalogSource>,
    installations: BTreeMap<String, InstalledModelBundle>,
    model_instances: BTreeMap<String, InstalledModelInstance>,
    references: Vec<ModelBundleReference>,
    license_acceptances: Vec<ModelLicenseAcceptance>,
    events: Vec<ModelBundleEvent>,
}

impl Default for ModelBundleRegistryState {
    fn default() -> Self {
        Self {
            schema_version: MODEL_REGISTRY_SCHEMA_VERSION,
            catalogs: BTreeMap::new(),
            trusted_local_catalogs: BTreeMap::new(),
            installations: BTreeMap::new(),
            model_instances: BTreeMap::new(),
            references: Vec::new(),
            license_acceptances: Vec::new(),
            events: Vec::new(),
        }
    }
}

pub struct ModelBundleRegistry {
    data_root: PathBuf,
    state: ModelBundleRegistryState,
}

impl ModelBundleRegistry {
    pub fn open(data_root: impl Into<PathBuf>) -> Result<Self, ModelCatalogError> {
        let data_root = data_root.into();
        std::fs::create_dir_all(&data_root)?;
        let state_path = data_root.join("model-bundle-registry.json");
        let state = if state_path.is_file() {
            let mut value: ModelBundleRegistryState =
                serde_json::from_slice(&std::fs::read(state_path)?)?;
            if value.schema_version > MODEL_REGISTRY_SCHEMA_VERSION {
                return Err(ModelCatalogError::Provisioning(format!(
                    "unsupported model Bundle Registry schema {}",
                    value.schema_version
                )));
            }
            value.schema_version = MODEL_REGISTRY_SCHEMA_VERSION;
            value
        } else {
            ModelBundleRegistryState::default()
        };
        let mut registry = Self { data_root, state };
        let fixture = crate::build_builtin_fixture_catalog(&registry.data_root)?;
        registry
            .state
            .catalogs
            .insert(fixture.catalog.catalog_id.clone(), fixture.catalog);
        registry.persist()?;
        Ok(registry)
    }

    #[must_use]
    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    #[must_use]
    pub fn catalogs(&self) -> Vec<ModelCatalog> {
        self.state.catalogs.values().cloned().collect()
    }

    pub fn save_catalog(&mut self, catalog: ModelCatalog) -> Result<(), ModelCatalogError> {
        catalog.validate()?;
        self.state
            .trusted_local_catalogs
            .remove(&catalog.catalog_id);
        self.state
            .catalogs
            .insert(catalog.catalog_id.clone(), catalog);
        self.persist()
    }

    #[must_use]
    pub fn trusted_local_catalogs(&self) -> Vec<TrustedLocalCatalogSource> {
        self.state
            .trusted_local_catalogs
            .values()
            .cloned()
            .collect()
    }

    pub fn add_trusted_local_catalog(
        &mut self,
        root: &Path,
    ) -> Result<ModelCatalog, ModelCatalogError> {
        let root = root.canonicalize().map_err(|error| {
            ModelCatalogError::Provisioning(format!(
                "cannot resolve trusted local Catalog {}: {error}",
                root.display()
            ))
        })?;
        let (catalog, source) = load_trusted_local_catalog(&root)?;
        self.state
            .catalogs
            .insert(catalog.catalog_id.clone(), catalog.clone());
        self.state
            .trusted_local_catalogs
            .insert(catalog.catalog_id.clone(), source);
        self.persist()?;
        Ok(catalog)
    }

    pub fn refresh_trusted_local_catalogs(
        &mut self,
    ) -> Result<Vec<ModelCatalog>, ModelCatalogError> {
        let refreshed = self
            .state
            .trusted_local_catalogs
            .values()
            .map(|source| load_trusted_local_catalog(&source.root))
            .collect::<Result<Vec<_>, _>>()?;
        for (catalog, source) in &refreshed {
            self.state
                .catalogs
                .insert(catalog.catalog_id.clone(), catalog.clone());
            self.state
                .trusted_local_catalogs
                .insert(catalog.catalog_id.clone(), source.clone());
        }
        self.persist()?;
        Ok(refreshed.into_iter().map(|(catalog, _)| catalog).collect())
    }

    #[must_use]
    pub fn available(&self) -> Vec<crate::ModelCatalogEntry> {
        self.state
            .catalogs
            .values()
            .flat_map(|catalog| catalog.entries.iter().cloned())
            .collect()
    }

    #[must_use]
    pub fn list(&self) -> Vec<InstalledModelBundle> {
        self.state.installations.values().cloned().collect()
    }

    #[must_use]
    pub fn model_instances(&self) -> Vec<InstalledModelInstance> {
        self.state.model_instances.values().cloned().collect()
    }

    #[must_use]
    pub fn model_instance(&self, id: ModelInstanceId) -> Option<&InstalledModelInstance> {
        self.state.model_instances.get(&id.to_string())
    }

    #[must_use]
    pub fn model_profiles(&self) -> Vec<ModelInstanceProfile> {
        self.state
            .model_instances
            .values()
            .map(|instance| {
                let bundle = self.state.installations.get(&bundle_key(
                    &instance.model_bundle_id,
                    &instance.model_bundle_version,
                ));
                let ready = instance.status == ModelInstanceStatus::Ready;
                let publishable = bundle.is_none_or(|bundle| bundle.manifest.publishable);
                let available = ready && publishable;
                ModelInstanceProfile {
                    model_profile_id: instance.model_profile_id,
                    model_profile_revision: instance.model_profile_revision,
                    model_instance_id: instance.id,
                    selection_id: model_instance_selection_id(instance.id),
                    display_name: bundle.map_or_else(
                        || instance.model_id.clone(),
                        |bundle| bundle.manifest.display_name.clone(),
                    ),
                    capabilities: bundle
                        .map_or_else(BTreeSet::new, |bundle| bundle.manifest.capabilities.clone()),
                    availability: if available {
                        ModelAvailability::Available
                    } else {
                        ModelAvailability::Unknown
                    },
                    selectable: available,
                }
            })
            .collect()
    }

    #[must_use]
    pub fn local_catalog_bundle_path(
        &self,
        catalog_id: &str,
        id: &ModelBundleId,
        version: &Version,
    ) -> Option<PathBuf> {
        let entry = self
            .state
            .catalogs
            .get(catalog_id)?
            .entries
            .iter()
            .find(|entry| entry.bundle_id == *id && entry.bundle_version == *version)?;
        if catalog_id == crate::BUILTIN_FIXTURE_CATALOG_ID {
            if !entry.fixture || entry.publishable {
                return None;
            }
            let path = crate::builtin_catalog_bundle_path(&self.data_root, catalog_id, id, version);
            return path.is_file().then_some(path);
        }
        let source = self.state.trusted_local_catalogs.get(catalog_id)?;
        let filename = catalog_bundle_filename(entry)?;
        let bundles_root = source.root.join("bundles").canonicalize().ok()?;
        let path = bundles_root.join(filename);
        let canonical = path.canonicalize().ok()?;
        canonical.starts_with(bundles_root).then_some(canonical)
    }

    #[must_use]
    pub fn get(&self, id: &ModelBundleId, version: &Version) -> Option<&InstalledModelBundle> {
        self.state.installations.get(&bundle_key(id, version))
    }

    #[must_use]
    pub fn events(&self) -> &[ModelBundleEvent] {
        &self.state.events
    }

    #[must_use]
    pub fn license_acceptances(&self) -> &[ModelLicenseAcceptance] {
        &self.state.license_acceptances
    }

    pub fn accept_license(
        &mut self,
        acceptance: ModelLicenseAcceptance,
    ) -> Result<(), ModelCatalogError> {
        self.state.license_acceptances.retain(|existing| {
            existing.bundle_id != acceptance.bundle_id
                || existing.bundle_version != acceptance.bundle_version
                || existing.license_digest != acceptance.license_digest
        });
        self.state.license_acceptances.push(acceptance);
        self.persist()
    }

    pub fn import_local(
        &mut self,
        package: &Path,
    ) -> Result<InstalledModelBundle, ModelCatalogError> {
        let verified = verify_model_bundle(package).map_err(|error| {
            ModelCatalogError::Provisioning(format!("Bundle verification failed: {error}"))
        })?;
        self.install_verified(verified, ModelBundleInstallSource::LocalImport)
    }

    pub fn install_verified(
        &mut self,
        verified: VerifiedModelBundle,
        source: ModelBundleInstallSource,
    ) -> Result<InstalledModelBundle, ModelCatalogError> {
        let key = bundle_key(&verified.manifest.id, &verified.manifest.version);
        if let Some(existing) = self.state.installations.get(&key) {
            if existing.bundle_digest == verified.bundle_digest {
                return Ok(existing.clone());
            }
            return Err(ModelCatalogError::Provisioning(format!(
                "{key} already maps to a different immutable digest"
            )));
        }
        if verified.manifest.license.requires_acceptance
            && !self.state.license_acceptances.iter().any(|acceptance| {
                acceptance.bundle_id == verified.manifest.id
                    && acceptance.bundle_version == verified.manifest.version
                    && acceptance.license_digest == verified.manifest.license.license_digest
            })
        {
            return Err(ModelCatalogError::Provisioning(
                "the exact model license digest has not been accepted".to_owned(),
            ));
        }

        let digest = verified.bundle_digest.as_str();
        let final_root = self
            .data_root
            .join("models/sha256")
            .join(&digest[..2])
            .join(digest);
        let staging = self
            .data_root
            .join("models/staging")
            .join(uuid::Uuid::new_v4().to_string());
        if let Some(parent) = final_root.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let result: Result<InstalledModelBundle, ModelCatalogError> = (|| {
            verified
                .extract_to(&staging)
                .map_err(|error| ModelCatalogError::Provisioning(error.to_string()))?;
            let verification = ModelBundleVerification {
                verified_at: Utc::now(),
                bundle_digest: verified.bundle_digest.clone(),
                signature: verified.signature,
                file_count: verified.files.len(),
                manifest_valid: true,
                checksums_valid: true,
            };
            std::fs::write(
                staging.join("verification.json"),
                serde_json::to_vec_pretty(&verification)?,
            )?;
            if final_root.exists() {
                std::fs::remove_dir_all(&staging)?;
            } else {
                std::fs::rename(&staging, &final_root)?;
            }
            let now = Utc::now();
            Ok(InstalledModelBundle {
                manifest: verified.manifest,
                bundle_digest: verified.bundle_digest,
                status: ModelBundleStatus::Installed,
                source,
                installed_at: now,
                updated_at: now,
                verification,
                enabled: true,
                content_root: final_root,
            })
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        let installed = result?;
        self.state.installations.insert(key, installed.clone());
        self.state.events.push(ModelBundleEvent {
            bundle_id: installed.manifest.id.clone(),
            bundle_version: installed.manifest.version.clone(),
            event: "installed".to_owned(),
            detail: format!("digest={}", installed.bundle_digest),
            created_at: Utc::now(),
        });
        self.persist()?;
        Ok(installed)
    }

    pub fn bind_model_instance(
        &mut self,
        request: BindModelInstanceRequest<'_>,
    ) -> Result<InstalledModelInstance, ModelCatalogError> {
        let BindModelInstanceRequest {
            plugin,
            plugin_package_digest,
            runtime_status,
            bundle_id,
            bundle_version,
            model_id,
            target,
            execution_provider,
        } = request;
        let bundle = self
            .get(bundle_id, bundle_version)
            .cloned()
            .ok_or_else(|| {
                ModelCatalogError::Provisioning("Model Bundle is not installed".to_owned())
            })?;
        let license_accepted = !bundle.manifest.license.requires_acceptance
            || self.state.license_acceptances.iter().any(|acceptance| {
                acceptance.bundle_id == bundle.manifest.id
                    && acceptance.bundle_version == bundle.manifest.version
                    && acceptance.license_digest == bundle.manifest.license.license_digest
            });
        let compatibility = ModelBundleCompatibilityResolver::resolve(
            Some(plugin),
            runtime_status,
            &bundle.manifest,
            model_id,
            target,
            execution_provider,
            license_accepted,
        );
        if !matches!(compatibility, ModelBundleCompatibility::Compatible { .. }) {
            return Err(ModelCatalogError::Provisioning(format!(
                "Model Bundle is incompatible: {compatibility:?}"
            )));
        }
        let model = plugin
            .models
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| ModelCatalogError::Provisioning("Plugin model is missing".to_owned()))?;
        let contract_inspection = inspect_onnx_contract(&bundle, execution_provider);
        let status = if contract_inspection.valid {
            ModelInstanceStatus::Preparing
        } else {
            ModelInstanceStatus::ContractMismatch
        };
        let identity = format!(
            "{}@{}:{}:{}:{}:{}",
            plugin.id,
            plugin.version,
            plugin_package_digest,
            bundle.bundle_digest,
            model_id,
            execution_provider
        );
        let id = ModelInstanceId::from_identity(&identity);
        let model_profile_id = ModelProfileId(uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!("annotagent-model-profile:{identity}").as_bytes(),
        ));
        let capability_contract_hash = Sha256Digest::of_bytes(
            &serde_json::to_vec(model)
                .map_err(|error| ModelCatalogError::Provisioning(error.to_string()))?,
        );
        let now = Utc::now();
        let instance = InstalledModelInstance {
            id,
            plugin_id: plugin.id.clone(),
            plugin_version: plugin.version.clone(),
            plugin_package_digest,
            model_id: model_id.to_owned(),
            model_bundle_id: bundle.manifest.id.clone(),
            model_bundle_version: bundle.manifest.version.clone(),
            model_bundle_digest: bundle.bundle_digest,
            model_variant: bundle.manifest.variant.clone(),
            model_file_digests: bundle
                .manifest
                .files
                .iter()
                .map(|file| (file.role.clone(), file.sha256.clone()))
                .collect(),
            execution_provider: execution_provider.to_owned(),
            capability_contract_hash,
            status,
            contract_inspection,
            smoke_test_id: None,
            smoke_test_result: None,
            model_profile_id,
            model_profile_revision: 1,
            created_at: now,
            updated_at: now,
        };
        self.state
            .model_instances
            .insert(instance.id.to_string(), instance.clone());
        self.state.events.push(ModelBundleEvent {
            bundle_id: instance.model_bundle_id.clone(),
            bundle_version: instance.model_bundle_version.clone(),
            event: "model_instance_bound".to_owned(),
            detail: format!("instance={} status={:?}", instance.id, instance.status),
            created_at: now,
        });
        self.persist()?;
        Ok(instance)
    }

    pub fn record_model_instance_smoke(
        &mut self,
        id: ModelInstanceId,
        result: SmokeTestResult,
    ) -> Result<InstalledModelInstance, ModelCatalogError> {
        let instance = self
            .state
            .model_instances
            .get_mut(&id.to_string())
            .ok_or_else(|| {
                ModelCatalogError::Provisioning("Model Instance was not found".to_owned())
            })?;
        if instance.status == ModelInstanceStatus::ContractMismatch {
            return Err(ModelCatalogError::Provisioning(
                "a Contract-mismatched Model Instance cannot run a smoke test".to_owned(),
            ));
        }
        instance.smoke_test_id = Some(result.test_id.clone());
        instance.status = if result.status == SmokeTestStatus::Passed {
            ModelInstanceStatus::Ready
        } else {
            ModelInstanceStatus::FailedSmokeTest
        };
        instance.smoke_test_result = Some(result);
        instance.updated_at = Utc::now();
        let instance = instance.clone();
        self.state.events.push(ModelBundleEvent {
            bundle_id: instance.model_bundle_id.clone(),
            bundle_version: instance.model_bundle_version.clone(),
            event: "model_instance_smoke_tested".to_owned(),
            detail: format!("instance={} status={:?}", instance.id, instance.status),
            created_at: instance.updated_at,
        });
        self.persist()?;
        Ok(instance)
    }

    pub fn add_reference(
        &mut self,
        reference: ModelBundleReference,
    ) -> Result<(), ModelCatalogError> {
        let installed = self
            .get(&reference.bundle_id, &reference.bundle_version)
            .ok_or_else(|| {
                ModelCatalogError::Provisioning("Model Bundle is not installed".to_owned())
            })?;
        if installed.bundle_digest != reference.bundle_digest {
            return Err(ModelCatalogError::Provisioning(
                "Model Bundle reference digest does not match installed content".to_owned(),
            ));
        }
        if !self.state.references.iter().any(|existing| {
            existing.bundle_id == reference.bundle_id
                && existing.bundle_version == reference.bundle_version
                && existing.bundle_digest == reference.bundle_digest
                && existing.kind == reference.kind
                && existing.location == reference.location
        }) {
            self.state.references.push(reference);
            self.persist()?;
        }
        Ok(())
    }

    #[must_use]
    pub fn references(&self, id: &ModelBundleId, version: &Version) -> Vec<ModelBundleReference> {
        self.state
            .references
            .iter()
            .filter(|reference| reference.bundle_id == *id && reference.bundle_version == *version)
            .cloned()
            .collect()
    }

    pub fn disable(
        &mut self,
        id: &ModelBundleId,
        version: &Version,
    ) -> Result<(), ModelCatalogError> {
        let key = bundle_key(id, version);
        let installation = self.state.installations.get_mut(&key).ok_or_else(|| {
            ModelCatalogError::Provisioning("Model Bundle is not installed".to_owned())
        })?;
        installation.enabled = false;
        installation.updated_at = Utc::now();
        for instance in self.state.model_instances.values_mut().filter(|instance| {
            instance.model_bundle_id == *id && instance.model_bundle_version == *version
        }) {
            instance.status = ModelInstanceStatus::Disabled;
            instance.updated_at = installation.updated_at;
        }
        self.persist()
    }

    pub fn enable(
        &mut self,
        id: &ModelBundleId,
        version: &Version,
    ) -> Result<(), ModelCatalogError> {
        let key = bundle_key(id, version);
        let installation = self.state.installations.get_mut(&key).ok_or_else(|| {
            ModelCatalogError::Provisioning("Model Bundle is not installed".to_owned())
        })?;
        installation.enabled = true;
        installation.updated_at = Utc::now();
        for instance in self.state.model_instances.values_mut().filter(|instance| {
            instance.model_bundle_id == *id && instance.model_bundle_version == *version
        }) {
            instance.status = match instance
                .smoke_test_result
                .as_ref()
                .map(|result| &result.status)
            {
                Some(SmokeTestStatus::Passed) => ModelInstanceStatus::Ready,
                Some(SmokeTestStatus::Failed | SmokeTestStatus::Crashed) => {
                    ModelInstanceStatus::FailedSmokeTest
                }
                None if instance.contract_inspection.valid => ModelInstanceStatus::Preparing,
                None => ModelInstanceStatus::ContractMismatch,
            };
            instance.updated_at = installation.updated_at;
        }
        self.persist()
    }

    pub fn remove(
        &mut self,
        id: &ModelBundleId,
        version: &Version,
    ) -> Result<(), ModelCatalogError> {
        let references = self.references(id, version);
        if !references.is_empty() {
            return Err(ModelCatalogError::Provisioning(format!(
                "Cannot remove this model bundle. Referenced by: {}",
                references
                    .iter()
                    .map(|reference| format!("{} {}", reference.kind, reference.location))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        let key = bundle_key(id, version);
        let installation = self.state.installations.remove(&key).ok_or_else(|| {
            ModelCatalogError::Provisioning("Model Bundle is not installed".to_owned())
        })?;
        self.state.model_instances.retain(|_, instance| {
            instance.model_bundle_id != *id || instance.model_bundle_version != *version
        });
        let digest_still_used = self
            .state
            .installations
            .values()
            .any(|other| other.bundle_digest == installation.bundle_digest);
        if !digest_still_used {
            let models_root = self.data_root.join("models/sha256");
            if !installation.content_root.starts_with(&models_root) {
                return Err(ModelCatalogError::Provisioning(
                    "refusing to remove an unexpected model content path".to_owned(),
                ));
            }
            if installation.content_root.exists() {
                std::fs::remove_dir_all(&installation.content_root)?;
            }
        }
        self.state.events.push(ModelBundleEvent {
            bundle_id: id.clone(),
            bundle_version: version.clone(),
            event: "removed".to_owned(),
            detail: format!("digest={}", installation.bundle_digest),
            created_at: Utc::now(),
        });
        self.persist()
    }

    pub fn garbage_collect(&mut self) -> Result<ModelBundleGcReport, ModelCatalogError> {
        let removable = self
            .state
            .installations
            .values()
            .filter(|bundle| {
                !bundle.enabled
                    && self
                        .references(&bundle.manifest.id, &bundle.manifest.version)
                        .is_empty()
            })
            .map(|bundle| (bundle.manifest.id.clone(), bundle.manifest.version.clone()))
            .collect::<Vec<_>>();
        let mut report = ModelBundleGcReport::default();
        for (id, version) in removable {
            let key = bundle_key(&id, &version);
            if let Some(bundle) = self.state.installations.get(&key) {
                report.reclaimed_bytes = report.reclaimed_bytes.saturating_add(
                    bundle
                        .manifest
                        .files
                        .iter()
                        .map(|file| file.size_bytes)
                        .sum::<u64>(),
                );
            }
            self.remove(&id, &version)?;
            report.removed_bundles.push(key);
        }
        for directory in [
            self.data_root.join("model-staging"),
            self.data_root.join("model-downloads"),
        ] {
            if !directory.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(&directory)? {
                let entry = entry?;
                let path = entry.path();
                if !path.starts_with(&directory) {
                    continue;
                }
                let metadata = std::fs::symlink_metadata(&path)?;
                if metadata.file_type().is_symlink() {
                    continue;
                }
                if metadata.is_dir() {
                    std::fs::remove_dir_all(path)?;
                } else if metadata.is_file() {
                    std::fs::remove_file(path)?;
                }
                report.removed_staging_entries += 1;
            }
        }
        Ok(report)
    }

    fn persist(&self) -> Result<(), ModelCatalogError> {
        let state_path = self.data_root.join("model-bundle-registry.json");
        let temporary = self.data_root.join(format!(
            ".model-bundle-registry-{}.tmp",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&temporary, serde_json::to_vec_pretty(&self.state)?)?;
        std::fs::rename(temporary, state_path)?;
        Ok(())
    }
}

fn load_trusted_local_catalog(
    root: &Path,
) -> Result<(ModelCatalog, TrustedLocalCatalogSource), ModelCatalogError> {
    let catalog_path = root.join("catalog.json");
    let bytes = std::fs::read(&catalog_path)?;
    if bytes.is_empty() || bytes.len() as u64 > crate::MAX_CATALOG_BYTES {
        return Err(ModelCatalogError::Provisioning(
            "trusted local Catalog is empty or exceeds the size limit".to_owned(),
        ));
    }
    let catalog = ModelCatalog::from_json(&bytes)?;
    let bundles_root = root.join("bundles").canonicalize().map_err(|error| {
        ModelCatalogError::Provisioning(format!(
            "trusted local Catalog bundles directory is unavailable: {error}"
        ))
    })?;
    if !bundles_root.starts_with(root) {
        return Err(ModelCatalogError::Provisioning(
            "trusted local Catalog bundles directory escapes its root".to_owned(),
        ));
    }
    for entry in &catalog.entries {
        let filename = catalog_bundle_filename(entry).ok_or_else(|| {
            ModelCatalogError::Provisioning(format!(
                "local Catalog Bundle {} has no safe release filename",
                entry.bundle_id
            ))
        })?;
        let package = bundles_root
            .join(filename)
            .canonicalize()
            .map_err(|error| {
                ModelCatalogError::Provisioning(format!(
                    "local Catalog Bundle {} is unavailable: {error}",
                    entry.bundle_id
                ))
            })?;
        if !package.starts_with(&bundles_root)
            || std::fs::metadata(&package)?.len() != entry.bundle_size_bytes
        {
            return Err(ModelCatalogError::Provisioning(format!(
                "local Catalog Bundle {} escaped its root or changed size",
                entry.bundle_id
            )));
        }
        let verified = verify_model_bundle(&package).map_err(|error| {
            ModelCatalogError::Provisioning(format!(
                "local Catalog Bundle {} failed verification: {error}",
                entry.bundle_id
            ))
        })?;
        if verified.bundle_digest != entry.bundle_sha256
            || verified.manifest.id != entry.bundle_id
            || verified.manifest.version != entry.bundle_version
            || verified.manifest.fixture != entry.fixture
            || verified.manifest.publishable != entry.publishable
            || verified.manifest.capabilities != entry.capabilities
            || verified.manifest.compatible_plugins != entry.compatible_plugins
            || verified.manifest.license.license_digest != entry.license_summary.license_digest
        {
            return Err(ModelCatalogError::Provisioning(format!(
                "local Catalog Bundle {} metadata does not match catalog.json",
                entry.bundle_id
            )));
        }
    }
    let source = TrustedLocalCatalogSource {
        catalog_id: catalog.catalog_id.clone(),
        root: root.to_path_buf(),
        catalog_sha256: Sha256Digest::of_bytes(&bytes),
    };
    Ok((catalog, source))
}

fn catalog_bundle_filename(entry: &crate::ModelCatalogEntry) -> Option<&str> {
    let filename = entry.bundle_url.path_segments()?.next_back()?;
    (!filename.is_empty()
        && filename.len() <= 240
        && filename
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')))
    .then_some(filename)
}

fn bundle_key(id: &ModelBundleId, version: &Version) -> String {
    format!("{id}@{version}")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use annotagent_core::ModelCapability;
    use annotagent_model_bundle::{
        CommercialUseStatus, ModelBundleFile, ModelContractReference, ModelExportMetadata,
        ModelFileRole, ModelFormat, ModelLicenseMetadata, ModelRuntimeMetadata,
        ModelSourceMetadata, ModelTestSuiteReference, PluginCompatibilityRequirement,
        RedistributionStatus, pack_model_bundle,
    };
    use annotagent_plugin_api::PluginId;

    use super::*;

    #[test]
    fn exact_license_digest_is_required_before_activation() {
        let temp = tempfile::tempdir().expect("temp");
        let mut registry = ModelBundleRegistry::open(temp.path()).expect("registry");
        let id = ModelBundleId::parse("org.annotagent.models.fixture").expect("id");
        let version = Version::new(1, 0, 0);
        let digest = Sha256Digest::of_bytes(b"license-v1");
        registry
            .accept_license(ModelLicenseAcceptance {
                bundle_id: id.clone(),
                bundle_version: version.clone(),
                license_digest: digest.clone(),
                accepted_at: Utc::now(),
                accepted_by: LicenseAcceptanceActor::LocalUser,
            })
            .expect("accept");
        let reopened = ModelBundleRegistry::open(temp.path()).expect("reopen");
        assert_eq!(reopened.license_acceptances().len(), 1);
        assert_eq!(reopened.license_acceptances()[0].bundle_id, id);
        assert_eq!(reopened.license_acceptances()[0].bundle_version, version);
        assert_eq!(reopened.license_acceptances()[0].license_digest, digest);
    }

    #[test]
    fn trusted_local_catalog_is_verified_persisted_and_refreshes_fail_closed() {
        let temp = tempfile::tempdir().expect("temp");
        let fixture_root = temp.path().join("fixture-source");
        let fixture = crate::build_builtin_fixture_catalog(&fixture_root).expect("fixture Catalog");
        let catalog_root = temp.path().join("trusted-catalog");
        let bundles_root = catalog_root.join("bundles");
        std::fs::create_dir_all(&bundles_root).expect("bundle directory");
        let filename = "verified-fixture.annotmodel";
        std::fs::copy(&fixture.package_path, bundles_root.join(filename)).expect("copy Bundle");
        let mut catalog = fixture.catalog;
        catalog.catalog_id = "org.annotagent.catalog.test-local".to_owned();
        catalog.entries[0].bundle_url =
            url::Url::parse(&format!("https://models.example/{filename}")).expect("URL");
        std::fs::write(
            catalog_root.join("catalog.json"),
            serde_json::to_vec_pretty(&catalog).expect("Catalog JSON"),
        )
        .expect("Catalog file");

        let registry_root = temp.path().join("registry");
        let mut registry = ModelBundleRegistry::open(&registry_root).expect("registry");
        let added = registry
            .add_trusted_local_catalog(&catalog_root)
            .expect("add local Catalog");
        assert_eq!(added, catalog);
        assert_eq!(registry.trusted_local_catalogs().len(), 1);
        assert_eq!(
            registry.local_catalog_bundle_path(
                &catalog.catalog_id,
                &catalog.entries[0].bundle_id,
                &catalog.entries[0].bundle_version,
            ),
            Some(bundles_root.join(filename).canonicalize().expect("path"))
        );

        let mut reopened = ModelBundleRegistry::open(&registry_root).expect("reopen");
        assert_eq!(reopened.trusted_local_catalogs().len(), 1);
        assert_eq!(
            reopened
                .refresh_trusted_local_catalogs()
                .expect("refresh")
                .len(),
            1
        );
        std::fs::write(bundles_root.join(filename), b"tampered").expect("tamper");
        assert!(reopened.refresh_trusted_local_catalogs().is_err());
        assert_eq!(
            reopened
                .catalogs()
                .iter()
                .filter(|value| value.catalog_id == catalog.catalog_id)
                .count(),
            1
        );
    }

    #[test]
    fn only_a_passing_smoke_test_makes_the_instance_profile_selectable() {
        let temp = tempfile::tempdir().expect("temp");
        let mut registry = ModelBundleRegistry::open(temp.path()).expect("registry");
        let instance_id = ModelInstanceId::new();
        let now = Utc::now();
        registry.state.model_instances.insert(
            instance_id.to_string(),
            InstalledModelInstance {
                id: instance_id,
                plugin_id: PluginId::parse("org.annotagent.fixture-plugin").expect("plugin"),
                plugin_version: PluginVersion::parse("1.0.0").expect("version"),
                plugin_package_digest: PluginSha256Digest::of_bytes(b"plugin"),
                model_id: "fixture-model".to_owned(),
                model_bundle_id: ModelBundleId::parse("org.annotagent.models.fixture")
                    .expect("bundle"),
                model_bundle_version: Version::new(1, 0, 0),
                model_bundle_digest: Sha256Digest::of_bytes(b"bundle"),
                model_variant: "tiny".to_owned(),
                model_file_digests: BTreeMap::from([(
                    ModelFileRole::parse("model").expect("role"),
                    Sha256Digest::of_bytes(b"model"),
                )]),
                execution_provider: "cpu".to_owned(),
                capability_contract_hash: Sha256Digest::of_bytes(b"contract"),
                status: ModelInstanceStatus::Preparing,
                contract_inspection: OnnxContractInspection {
                    contract_sha256: Sha256Digest::of_bytes(b"contract"),
                    roles: BTreeMap::new(),
                    valid: true,
                    errors: Vec::new(),
                },
                smoke_test_id: None,
                smoke_test_result: None,
                model_profile_id: ModelProfileId::new(),
                model_profile_revision: 1,
                created_at: now,
                updated_at: now,
            },
        );
        assert!(!registry.model_profiles()[0].selectable);
        let result = SmokeTestResult {
            test_id: "fixture-smoke".to_owned(),
            status: SmokeTestStatus::Passed,
            checks: vec![annotagent_model_bundle::SmokeTestCheck {
                name: "sample".to_owned(),
                passed: true,
                detail: "typed output".to_owned(),
            }],
            duration_ms: 1,
            started_at: now,
            finished_at: now,
        };
        let ready = registry
            .record_model_instance_smoke(instance_id, result)
            .expect("record smoke");
        assert_eq!(ready.status, ModelInstanceStatus::Ready);
        assert!(registry.model_profiles()[0].selectable);
        assert_eq!(
            registry.model_profiles()[0].availability,
            ModelAvailability::Available
        );
    }

    #[test]
    fn verified_bundle_installs_once_into_content_addressed_storage() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source");
        let package = temp.path().join("fixture.annotmodel");
        std::fs::create_dir_all(&source).expect("source");
        let manifest = fixture_bundle_source(&source);
        let package_digest = pack_model_bundle(&source, &package).expect("pack");
        let registry_root = temp.path().join("registry");
        let mut registry = ModelBundleRegistry::open(&registry_root).expect("registry");
        assert!(registry.import_local(&package).is_err());
        assert!(registry.list().is_empty());

        registry
            .accept_license(ModelLicenseAcceptance {
                bundle_id: manifest.id.clone(),
                bundle_version: manifest.version.clone(),
                license_digest: manifest.license.license_digest.clone(),
                accepted_at: Utc::now(),
                accepted_by: LicenseAcceptanceActor::LocalUser,
            })
            .expect("accept");
        let installed = registry.import_local(&package).expect("install");
        assert_eq!(installed.bundle_digest, package_digest);
        assert_eq!(installed.status, ModelBundleStatus::Installed);
        assert!(
            installed
                .content_root
                .starts_with(registry_root.join("models/sha256"))
        );
        assert!(installed.content_root.join("files/model.onnx").is_file());
        assert!(installed.content_root.join("verification.json").is_file());
        assert_eq!(
            registry
                .import_local(&package)
                .expect("idempotent")
                .bundle_digest,
            package_digest
        );
        assert_eq!(registry.list().len(), 1);

        let reopened = ModelBundleRegistry::open(&registry_root).expect("reopen");
        assert_eq!(reopened.list().len(), 1);
        assert_eq!(reopened.events().len(), 1);
    }

    #[test]
    fn published_reference_blocks_removal_and_gc_preserves_the_bundle() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source");
        let package = temp.path().join("fixture.annotmodel");
        std::fs::create_dir_all(&source).expect("source");
        let manifest = fixture_bundle_source(&source);
        pack_model_bundle(&source, &package).expect("pack");
        let mut registry =
            ModelBundleRegistry::open(temp.path().join("registry")).expect("registry");
        registry
            .accept_license(ModelLicenseAcceptance {
                bundle_id: manifest.id.clone(),
                bundle_version: manifest.version.clone(),
                license_digest: manifest.license.license_digest.clone(),
                accepted_at: Utc::now(),
                accepted_by: LicenseAcceptanceActor::LocalUser,
            })
            .expect("accept");
        let installed = registry.import_local(&package).expect("install");
        registry
            .add_reference(ModelBundleReference {
                bundle_id: manifest.id.clone(),
                bundle_version: manifest.version.clone(),
                bundle_digest: installed.bundle_digest,
                kind: "published_workflow".to_owned(),
                location: "fixture@v1".to_owned(),
                created_at: Utc::now(),
            })
            .expect("reference");
        registry
            .disable(&manifest.id, &manifest.version)
            .expect("disable");
        assert!(registry.remove(&manifest.id, &manifest.version).is_err());
        let report = registry.garbage_collect().expect("gc");
        assert!(report.removed_bundles.is_empty());
        assert!(registry.get(&manifest.id, &manifest.version).is_some());
    }

    fn fixture_bundle_source(root: &Path) -> ModelBundleManifest {
        let model_bytes = b"fixture model bytes";
        let contract_bytes = b"{}";
        let license_bytes = b"CC0";
        for (path, bytes) in [
            ("files/model.onnx", model_bytes.as_slice()),
            ("contracts/model-contract.json", contract_bytes.as_slice()),
            ("licenses/MODEL-LICENSE", license_bytes.as_slice()),
            ("tests/input.png", b"png".as_slice()),
            ("tests/expected.json", b"{}".as_slice()),
            ("tests/tolerances.json", b"{}".as_slice()),
        ] {
            let target = root.join(path);
            std::fs::create_dir_all(target.parent().expect("parent")).expect("dirs");
            std::fs::write(target, bytes).expect("write");
        }
        let role = ModelFileRole::parse("model").expect("role");
        let manifest = ModelBundleManifest {
            schema_version: "1".to_owned(),
            id: ModelBundleId::parse("org.annotagent.models.fixture").expect("id"),
            version: Version::new(1, 0, 0),
            display_name: "Fixture model".to_owned(),
            description: Some("Lifecycle fixture".to_owned()),
            model_family: "fixture".to_owned(),
            architecture: "identity".to_owned(),
            format: ModelFormat::Onnx,
            variant: "tiny".to_owned(),
            capabilities: BTreeSet::from([ModelCapability::PromptedSegmentation]),
            compatible_plugins: vec![PluginCompatibilityRequirement {
                plugin_id: PluginId::parse("org.annotagent.sam-onnx").expect("plugin"),
                plugin_version: ">=1.0.0,<2.0.0".to_owned(),
                model_id: "sam-vit-b-onnx".to_owned(),
                contract_hash: Sha256Digest::of_bytes(b"plugin-contract"),
                required_file_roles: BTreeSet::from([role.clone()]),
            }],
            files: vec![ModelBundleFile {
                role: role.clone(),
                path: "files/model.onnx".to_owned(),
                sha256: Sha256Digest::of_bytes(model_bytes),
                size_bytes: u64::try_from(model_bytes.len()).expect("size"),
                external_data_files: Vec::new(),
            }],
            contracts: vec![ModelContractReference {
                id: "model".to_owned(),
                path: "contracts/model-contract.json".to_owned(),
                sha256: Sha256Digest::of_bytes(contract_bytes),
                file_roles: BTreeSet::from([role]),
            }],
            transforms: Vec::new(),
            source: ModelSourceMetadata {
                upstream_project: "AnnotAgent fixtures".to_owned(),
                upstream_model_id: "fixture".to_owned(),
                upstream_version: Some("1".to_owned()),
                upstream_checkpoint_sha256: None,
                source_url: None,
            },
            export: ModelExportMetadata {
                exporter_name: "fixture".to_owned(),
                exporter_version: "1".to_owned(),
                exporter_revision: None,
                export_date: None,
                opset: Some(17),
                numerical_validation: None,
            },
            runtime: ModelRuntimeMetadata {
                execution_providers: BTreeSet::from(["cpu".to_owned()]),
                platforms: BTreeSet::from(["macos-aarch64".to_owned()]),
                minimum_memory_mb: 64,
                recommended_memory_mb: 128,
            },
            license: ModelLicenseMetadata {
                name: "CC0-1.0".to_owned(),
                license_url: None,
                license_file: "licenses/MODEL-LICENSE".to_owned(),
                source_notice: None,
                license_digest: Sha256Digest::of_bytes(license_bytes),
                redistribution: RedistributionStatus::Allowed,
                commercial_use: CommercialUseStatus::Allowed,
                requires_acceptance: true,
                usage_notes: vec!["Fixture only".to_owned()],
            },
            test_suite: ModelTestSuiteReference {
                test_id: "fixture".to_owned(),
                input_artifacts: vec!["tests/input.png".to_owned()],
                expected_summary: "tests/expected.json".to_owned(),
                tolerances: "tests/tolerances.json".to_owned(),
            },
            fixture: true,
            publishable: false,
        };
        std::fs::write(
            root.join(annotagent_model_bundle::MODEL_BUNDLE_MANIFEST_FILE),
            manifest.to_toml().expect("manifest"),
        )
        .expect("manifest");
        manifest
    }
}
