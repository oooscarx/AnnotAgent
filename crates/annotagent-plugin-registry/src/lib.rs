//! Durable plugin installation, version, weight and reference registry.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read as _,
    path::{Path, PathBuf},
};

use annotagent_core::{
    ArtifactKind, ArtifactRef, BoxPrompt, BoxPromptSetArtifact, CheckpointIdentity,
    ExpertModelManifest, ImageId, LicenseMetadata, LicensePermission, ModelAvailability,
    ModelAvailabilityEvidence, ModelCapability, ModelConnection, NormalizedRect,
    PIPELINE_VISION_PROTOCOL_VERSION, PipelineArtifact, PipelineInferenceRequest,
    PipelineInferenceResponse, PromptContract, PromptKind, RunId, VisionCapability,
};
use annotagent_plugin_api::{
    CommercialUseDeclaration, PluginId, PluginImplementationStatus, PluginManifest,
    PluginModelReference, PluginRuntimeStatus, PluginStatus, PluginTestReport, PluginVersion,
    Sha256Digest,
};
use annotagent_plugin_host::{
    HostedPlugin, PackageSignatureState, PluginHostError, PluginPackageError, PluginProcessConfig,
    current_target, process_directories, verify_package,
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

fn conformance_sample_request(
    installation: &PluginInstallation,
) -> Result<PipelineInferenceRequest, PluginRegistryError> {
    let model = installation.manifest.models.first().ok_or_else(|| {
        PluginRegistryError::InvalidTransition("plugin declares no model".to_owned())
    })?;
    let capability = *model.capabilities.first().ok_or_else(|| {
        PluginRegistryError::InvalidTransition("plugin model declares no capability".to_owned())
    })?;
    let operation = match capability {
        ModelCapability::VisionLanguage => VisionCapability::VisionLanguage,
        ModelCapability::ImageClassification => VisionCapability::Classification,
        ModelCapability::ObjectDetection => VisionCapability::ObjectDetection,
        ModelCapability::OpenVocabularyDetection => VisionCapability::OpenVocabularyDetection,
        ModelCapability::PhraseGrounding => VisionCapability::PhraseGrounding,
        ModelCapability::SemanticSegmentation => VisionCapability::SemanticSegmentation,
        ModelCapability::PromptedSegmentation => VisionCapability::PromptedSegmentation,
        ModelCapability::InstanceSegmentation => VisionCapability::InstanceSegmentation,
        ModelCapability::KeypointDetection => VisionCapability::KeypointDetection,
        ModelCapability::TextGeneration => {
            return Err(PluginRegistryError::InvalidTransition(
                "text generation is not an expert vision operation".to_owned(),
            ));
        }
    };
    let image_id = ImageId::new();
    let input_artifacts = if operation == VisionCapability::PromptedSegmentation {
        let source = ArtifactRef {
            artifact_id: "conformance-detections".to_owned(),
            source_node: "plugin_conformance".to_owned(),
            port: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            item_id: None,
        };
        vec![PipelineArtifact::BoxPromptSet(BoxPromptSetArtifact {
            reference: ArtifactRef {
                artifact_id: "conformance-prompts".to_owned(),
                source_node: "plugin_conformance".to_owned(),
                port: "box_prompts".to_owned(),
                artifact_type: ArtifactKind::BoxPromptSet,
                item_id: None,
            },
            image_id,
            source_detections: source.clone(),
            prompts: vec![BoxPrompt {
                id: "conformance-box".to_owned(),
                subject: source.item("conformance-object"),
                bbox: NormalizedRect::new(0.2, 0.2, 0.6, 0.6)
                    .expect("static conformance rectangle is valid"),
                attributes: BTreeMap::new(),
            }],
        })]
    } else {
        Vec::new()
    };
    Ok(PipelineInferenceRequest {
        protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
        request_id: uuid::Uuid::new_v4().to_string(),
        run_id: RunId::new(),
        image_id,
        node_id: "plugin_conformance".to_owned(),
        model_id: model.id.clone(),
        operation,
        image: Some(annotagent_core::ModelImage {
            id: "conformance-image".to_owned(),
            mime_type: "image/png".to_owned(),
            data_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
        }),
        input_artifacts,
        parameters: BTreeMap::new(),
        timeout_ms: Some(30_000),
    })
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
    #[error("plugin process test failed: {0}")]
    Host(#[from] PluginHostError),
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

    /// Returns executable-runtime health only. Missing model assets deliberately do not make an
    /// installed Rust package disappear or overload this status with Bundle readiness.
    #[must_use]
    pub const fn runtime_status(&self) -> PluginRuntimeStatus {
        match self.status {
            PluginStatus::Discovered | PluginStatus::Installing => {
                PluginRuntimeStatus::NotInstalled
            }
            PluginStatus::Disabled => PluginRuntimeStatus::Disabled,
            PluginStatus::Starting => PluginRuntimeStatus::Starting,
            PluginStatus::Ready => PluginRuntimeStatus::Ready,
            PluginStatus::Unhealthy | PluginStatus::FailedSmokeTest => {
                PluginRuntimeStatus::Unhealthy
            }
            PluginStatus::Crashed => PluginRuntimeStatus::Crashed,
            PluginStatus::IncompatibleApi
            | PluginStatus::InvalidManifest
            | PluginStatus::InvalidContract
            | PluginStatus::UnsupportedPlatform => PluginRuntimeStatus::Incompatible,
            PluginStatus::Installed
            | PluginStatus::NeedsWeights
            | PluginStatus::UpdateAvailable => PluginRuntimeStatus::Installed,
        }
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

#[derive(Debug, Clone)]
pub struct PluginModelSmokeReport {
    pub conformance: PluginTestReport,
    pub response: PipelineInferenceResponse,
}

#[must_use]
pub fn plugin_model_selection_id(reference: &PluginModelReference) -> String {
    format!(
        "plugin:{}@{}:{}",
        reference.plugin_id, reference.plugin_version, reference.model_id
    )
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
        let data_root = std::fs::canonicalize(data_root)?;
        let state_path = data_root.join("plugin-registry.json");
        let mut state = if state_path.is_file() {
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
        for installation in state.installations.values_mut() {
            installation.installation_root = installation_root(
                &data_root,
                &installation.manifest.id,
                &installation.manifest.version,
            );
        }
        for weights in &mut state.weight_sets {
            let filename = weights.stored_path.file_name().map_or_else(
                || std::ffi::OsString::from(&weights.original_filename),
                std::ffi::OsStr::to_owned,
            );
            weights.stored_path = data_root
                .join("model-cache")
                .join(weights.plugin_id.as_str())
                .join(weights.plugin_version.to_string())
                .join(&weights.model_id)
                .join(weights.checkpoint_sha256.as_str())
                .join(filename);
        }
        let registry = Self { data_root, state };
        registry.persist()?;
        Ok(registry)
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

    /// Constructs the same restricted process sandbox used by CLI, Server and runtime callers.
    /// The child receives only its installation, state, cache, temporary and weight roots.
    pub fn process_config(
        &self,
        installation: &PluginInstallation,
    ) -> Result<PluginProcessConfig, PluginRegistryError> {
        let executable =
            self.executable(&installation.manifest.id, &installation.manifest.version)?;
        let process_root = self
            .data_root
            .join("plugin-state")
            .join(installation.manifest.id.as_str())
            .join(installation.manifest.version.to_string());
        let (state_dir, cache_dir, temporary_dir) = process_directories(&process_root);
        let weights_dir =
            self.weights_root(&installation.manifest.id, &installation.manifest.version)?;
        let maximum_response_bytes = installation
            .manifest
            .resources
            .maximum_response_mb
            .saturating_mul(1024 * 1024);
        Ok(PluginProcessConfig {
            executable,
            installation_root: installation.installation_root.clone(),
            state_dir,
            weights_dir,
            model_files: BTreeMap::new(),
            cache_dir,
            temporary_dir,
            max_request_bytes: 64 * 1024 * 1024,
            max_response_bytes: usize::try_from(maximum_response_bytes)
                .unwrap_or(256 * 1024 * 1024),
        })
    }

    pub fn process_config_for_model_files(
        &self,
        installation: &PluginInstallation,
        weights_dir: &Path,
        model_files: BTreeMap<String, PathBuf>,
    ) -> Result<PluginProcessConfig, PluginRegistryError> {
        if !weights_dir.is_absolute() || !weights_dir.is_dir() {
            return Err(PluginRegistryError::InvalidWeight(
                "verified Bundle content root must be an absolute directory".to_owned(),
            ));
        }
        for (role, path) in &model_files {
            if !installation
                .manifest
                .models
                .iter()
                .any(|model| model.required_file_roles.contains(role))
                || !path.starts_with(weights_dir)
                || !path.is_file()
            {
                return Err(PluginRegistryError::InvalidWeight(format!(
                    "model file role {role:?} is not a verified file for this Plugin"
                )));
            }
        }
        let mut config = self.process_config(installation)?;
        config.weights_dir = weights_dir.to_path_buf();
        config.model_files = model_files;
        Ok(config)
    }

    /// Runs package conformance plus exactly one Bundle-provided inference request against the
    /// exact role-bound files. Readiness is recorded by the Model Bundle Registry, not by the
    /// legacy raw-weight Plugin status.
    pub async fn test_model_instance(
        &self,
        installation: &PluginInstallation,
        weights_dir: &Path,
        model_files: BTreeMap<String, PathBuf>,
        sample: &PipelineInferenceRequest,
    ) -> Result<PluginModelSmokeReport, PluginRegistryError> {
        if installation.manifest.implementation_status == PluginImplementationStatus::Unsupported {
            return Err(PluginRegistryError::InvalidTransition(
                "unsupported plugin versions cannot run a Model Instance smoke test".to_owned(),
            ));
        }
        run_model_instance_smoke(
            installation.manifest.clone(),
            self.process_config_for_model_files(installation, weights_dir, model_files)?,
            sample,
        )
        .await
    }

    /// Runs the authenticated HTTP Vision conformance and one typed sample inference in an
    /// isolated child process. The caller records the returned report as a separate transaction.
    pub async fn test_installation(
        &self,
        installation: &PluginInstallation,
    ) -> Result<PluginTestReport, PluginRegistryError> {
        if installation.status == PluginStatus::NeedsWeights {
            return Err(PluginRegistryError::InvalidTransition(
                "plugin requires checkpoint provisioning before process testing".to_owned(),
            ));
        }
        if installation.manifest.implementation_status == PluginImplementationStatus::Unsupported {
            return Err(PluginRegistryError::InvalidTransition(
                "unsupported plugin versions cannot run a readiness smoke test".to_owned(),
            ));
        }
        let host = HostedPlugin::start(
            installation.manifest.clone(),
            self.process_config(installation)?,
        )
        .await?;
        let sample = conformance_sample_request(installation)?;
        let report = host.test(Some(&sample)).await;
        let stop_result = host.stop().await;
        match (report, stop_result) {
            (Ok(report), Ok(())) => Ok(report),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error.into()),
        }
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
        if !self.state.references.iter().any(|existing| {
            existing.plugin_id == reference.plugin_id
                && existing.plugin_version == reference.plugin_version
                && existing.kind == reference.kind
                && existing.location == reference.location
        }) {
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

    /// Returns credential-free expert manifests for Agent discovery. Ready models carry the full
    /// availability evidence required for selection; setup-only models remain visible but cannot
    /// be published.
    #[must_use]
    pub fn expert_model_manifests(&self) -> Vec<ExpertModelManifest> {
        let profiles = self
            .ready_models()
            .into_iter()
            .map(|profile| {
                (
                    format!(
                        "{}@{}:{}",
                        profile.reference.plugin_id,
                        profile.reference.plugin_version,
                        profile.reference.model_id
                    ),
                    profile,
                )
            })
            .collect::<BTreeMap<_, _>>();
        self.state
            .installations
            .values()
            .flat_map(|installation| {
                installation.manifest.models.iter().filter_map(|model| {
                    let key = format!(
                        "{}@{}:{}",
                        installation.manifest.id, installation.manifest.version, model.id
                    );
                    let profile = profiles.get(&key)?;
                    let test_passed = installation
                        .last_test
                        .as_ref()
                        .is_some_and(|report| report.passed);
                    let check_passed = |name: &str| {
                        installation.last_test.as_ref().is_some_and(|report| {
                            report
                                .checks
                                .iter()
                                .any(|check| check.name == name && check.passed)
                        })
                    };
                    let available = profile.availability == ModelAvailability::Available;
                    let checked_at = installation
                        .last_test
                        .as_ref()
                        .map(|report| report.finished_at);
                    let prompt_contracts = model
                        .input_contracts
                        .iter()
                        .filter_map(|contract| match contract.data_type {
                            annotagent_core::ContractDataType::Artifact(
                                ArtifactKind::BoxPromptSet,
                            ) => Some(PromptContract {
                                kind: PromptKind::Box,
                                required: contract.required,
                                multiple: contract.multiple,
                            }),
                            annotagent_core::ContractDataType::Artifact(
                                ArtifactKind::PointPromptSet,
                            ) => Some(PromptContract {
                                kind: PromptKind::Point,
                                required: contract.required,
                                multiple: contract.multiple,
                            }),
                            _ => None,
                        })
                        .collect();
                    let checkpoint = profile.reference.checkpoint_sha256.as_ref().map(|digest| {
                        CheckpointIdentity {
                            sha256: digest.to_string(),
                            source: Some("AnnotAgent local plugin model cache".to_owned()),
                            training_dataset_version: None,
                        }
                    });
                    let availability_evidence = ModelAvailabilityEvidence {
                        health_passed: available && check_passed("health"),
                        protocol_compatible: available
                            && check_passed("capability declaration")
                            && check_passed("model discovery"),
                        contracts_validated: available && check_passed("contract discovery"),
                        sample_conversion_passed: available && check_passed("sample inference"),
                        weights_ready: profile.reference.checkpoint_sha256.is_some()
                            || !installation.manifest.weights.required,
                        checked_at,
                        detail: Some(if available && test_passed {
                            "Installed Rust plugin passed authenticated process conformance and typed sample inference"
                                .to_owned()
                        } else {
                            format!("Plugin lifecycle status is {:?}", profile.plugin_status)
                        }),
                    };
                    let commercial_use = match installation.manifest.license.commercial_use {
                        CommercialUseDeclaration::Allowed => LicensePermission::Allowed,
                        CommercialUseDeclaration::Restricted => LicensePermission::Restricted,
                        CommercialUseDeclaration::Unknown => LicensePermission::Unknown,
                    };
                    let manifest = ExpertModelManifest {
                        schema_version:
                            annotagent_core::EXPERT_MODEL_MANIFEST_SCHEMA_VERSION.to_string(),
                        model_id: plugin_model_selection_id(&profile.reference),
                        display_name: format!(
                            "{} · {} {}",
                            model.display_name,
                            installation.manifest.display_name,
                            installation.manifest.version
                        ),
                        architecture: Some(installation.manifest.id.to_string()),
                        model_version: installation.manifest.version.to_string(),
                        connection: ModelConnection::VisionWorkerModel {
                            worker_id: format!(
                                "plugin:{}@{}",
                                installation.manifest.id, installation.manifest.version
                            ),
                            worker_model_id: model.id.clone(),
                        },
                        capabilities: model.capabilities.iter().copied().collect(),
                        input_contracts: model.input_contracts.clone(),
                        output_contracts: model.output_contracts.clone(),
                        prompt_contracts,
                        score_semantics: model.score_semantics,
                        geometry_semantics: model.geometry_semantics,
                        label_space: None,
                        checkpoint,
                        runtime_requirements: model.runtime_requirements.clone(),
                        license: LicenseMetadata {
                            code_license: Some(installation.manifest.license.code.clone()),
                            weight_license: Some(installation.manifest.license.weights.clone()),
                            commercial_use,
                            usage_notes: vec![format!(
                                "Published by {} as plugin {}@{}",
                                installation.manifest.publisher,
                                installation.manifest.id,
                                installation.manifest.version
                            )],
                            ..LicenseMetadata::default()
                        },
                        availability: profile.availability,
                        availability_evidence,
                        metadata: BTreeMap::from([
                            ("plugin_id".to_owned(), serde_json::json!(installation.manifest.id)),
                            (
                                "plugin_version".to_owned(),
                                serde_json::json!(installation.manifest.version),
                            ),
                            (
                                "plugin_package_sha256".to_owned(),
                                serde_json::json!(installation.package_digest),
                            ),
                            (
                                "plugin_api_version".to_owned(),
                                serde_json::json!(installation.manifest.plugin_api),
                            ),
                            (
                                "worker_protocol_version".to_owned(),
                                serde_json::json!(profile.reference.protocol_version),
                            ),
                            (
                                "capability_contract_sha256".to_owned(),
                                serde_json::json!(profile.reference.capability_contract_hash),
                            ),
                        ]),
                    };
                    manifest.validate().ok().map(|()| manifest)
                })
            })
            .collect()
    }

    #[must_use]
    pub fn weight_sets(
        &self,
        plugin_id: &PluginId,
        version: &PluginVersion,
    ) -> Vec<PluginWeightSet> {
        self.state
            .weight_sets
            .iter()
            .filter(|weights| weights.plugin_id == *plugin_id && weights.plugin_version == *version)
            .cloned()
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

pub async fn run_model_instance_smoke(
    manifest: PluginManifest,
    config: PluginProcessConfig,
    sample: &PipelineInferenceRequest,
) -> Result<PluginModelSmokeReport, PluginRegistryError> {
    let host = HostedPlugin::start(manifest, config).await?;
    let conformance = host.test(None).await;
    let response = host.infer(sample).await;
    let stop_result = host.stop().await;
    let mut report = conformance?;
    let response = response?;
    stop_result?;
    let passed = response.request_id.as_deref() == Some(sample.request_id.as_str())
        && response.error.is_none()
        && !response.artifacts.is_empty()
        && response
            .artifacts
            .iter()
            .all(|artifact| artifact.validate().is_ok());
    report.checks.push(annotagent_plugin_api::PluginTestCheck {
        name: "bundle sample inference".to_owned(),
        passed,
        detail: "fixed Bundle sample returns scoped, typed and valid artifacts".to_owned(),
    });
    report.finished_at = Utc::now();
    report.passed = report.checks.iter().all(|check| check.passed);
    Ok(PluginModelSmokeReport {
        conformance: report,
        response,
    })
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
        manifest.models[0].required_file_roles = if weights_required {
            std::collections::BTreeSet::from(["model".to_owned()])
        } else {
            std::collections::BTreeSet::new()
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
    fn relative_data_root_is_persisted_as_an_absolute_process_root() {
        let current = std::env::current_dir()
            .expect("current directory")
            .canonicalize()
            .expect("canonical current directory");
        let temp = tempfile::Builder::new()
            .prefix("plugin-registry-relative-")
            .tempdir_in(&current)
            .expect("temp");
        let relative = temp
            .path()
            .strip_prefix(&current)
            .expect("relative temp")
            .join("data");
        let registry = PluginRegistry::open(relative).expect("registry");
        assert!(registry.data_root().is_absolute());
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

    #[test]
    fn expert_manifests_are_discoverable_but_only_selectable_after_conformance() {
        let temp = tempfile::tempdir().expect("temp");
        let (package, manifest) = package(&temp, "3.0.0", false);
        let mut registry = PluginRegistry::open(temp.path().join("data")).expect("registry");
        registry.install(&package, &approval()).expect("install");

        let discovered = registry.expert_model_manifests();
        assert_eq!(discovered.len(), 1);
        assert_eq!(
            discovered[0].model_id,
            format!(
                "plugin:{}@{}:{}",
                manifest.id, manifest.version, manifest.models[0].id
            )
        );
        assert_ne!(discovered[0].availability, ModelAvailability::Available);
        assert!(!discovered[0].availability_evidence.available());
        discovered[0].validate().expect("setup-only manifest");

        let now = Utc::now();
        registry
            .record_test(PluginTestReport {
                plugin_id: manifest.id,
                plugin_version: manifest.version,
                passed: true,
                checks: [
                    "health",
                    "capability declaration",
                    "model discovery",
                    "contract discovery",
                    "sample inference",
                ]
                .into_iter()
                .map(|name| annotagent_plugin_api::PluginTestCheck {
                    name: name.to_owned(),
                    passed: true,
                    detail: "typed fixture".to_owned(),
                })
                .collect(),
                started_at: now,
                finished_at: now,
            })
            .expect("record conformance");

        let ready = registry.expert_model_manifests();
        assert_eq!(ready[0].availability, ModelAvailability::Available);
        assert!(ready[0].availability_evidence.available());
        ready[0].validate().expect("ready manifest");
    }
}
