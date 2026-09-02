use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use annotagent_core::{ModelAvailability, ModelCapability, ModelProfileId};
use annotagent_model_bundle::{
    ModelBundleId, ModelBundleManifest, ModelBundleSignatureState, ModelBundleStatus,
    ModelFileRole, ModelInstanceId, ModelInstanceStatus, Sha256Digest, VerifiedModelBundle,
    verify_model_bundle,
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

const MODEL_REGISTRY_SCHEMA_VERSION: u32 = 2;

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
    pub smoke_test_result: Option<serde_json::Value>,
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
    pub display_name: String,
    pub capabilities: BTreeSet<ModelCapability>,
    pub availability: ModelAvailability,
    pub selectable: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct ModelBundleRegistryState {
    schema_version: u32,
    catalogs: BTreeMap<String, ModelCatalog>,
    installations: BTreeMap<String, InstalledModelBundle>,
    model_instances: BTreeMap<String, InstalledModelInstance>,
    license_acceptances: Vec<ModelLicenseAcceptance>,
    events: Vec<ModelBundleEvent>,
}

impl Default for ModelBundleRegistryState {
    fn default() -> Self {
        Self {
            schema_version: MODEL_REGISTRY_SCHEMA_VERSION,
            catalogs: BTreeMap::new(),
            installations: BTreeMap::new(),
            model_instances: BTreeMap::new(),
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
        Ok(Self { data_root, state })
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
            .catalogs
            .insert(catalog.catalog_id.clone(), catalog);
        self.persist()
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
    pub fn model_profiles(&self) -> Vec<ModelInstanceProfile> {
        self.state
            .model_instances
            .values()
            .map(|instance| {
                let bundle = self.state.installations.get(&bundle_key(
                    &instance.model_bundle_id,
                    &instance.model_bundle_version,
                ));
                let available = instance.status == ModelInstanceStatus::Ready;
                ModelInstanceProfile {
                    model_profile_id: instance.model_profile_id,
                    model_profile_revision: instance.model_profile_revision,
                    model_instance_id: instance.id,
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
