//! Stable package, lifecycle and process protocol types for expert model plugins.

use std::{collections::BTreeMap, fmt, str::FromStr};

use annotagent_core::{
    ArtifactContract, GeometrySemantics, ModelCapability, PipelineInferenceRequest,
    PipelineInferenceResponse, RuntimeRequirements, ScoreSemantics,
};
use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const PLUGIN_MANIFEST_SCHEMA_VERSION: &str = "1";
pub const PLUGIN_API_VERSION: &str = "1";
pub const PLUGIN_PROTOCOL_VERSION: &str = "1";
pub const PLUGIN_MANIFEST_FILE: &str = "annotagent-plugin.toml";
pub const PLUGIN_CHECKSUM_FILE: &str = "checksums.json";
pub const PLUGIN_PACKAGE_EXTENSION: &str = "annotplugin";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PluginApiError {
    #[error("invalid plugin id: {0}")]
    InvalidPluginId(String),
    #[error("invalid plugin version: {0}")]
    InvalidPluginVersion(String),
    #[error("invalid plugin manifest: {0}")]
    InvalidManifest(String),
    #[error("invalid plugin contract: {0}")]
    InvalidContract(String),
    #[error("invalid digest: {0}")]
    InvalidDigest(String),
    #[error("manifest serialization failed: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginId(String);

impl PluginId {
    pub fn parse(value: impl Into<String>) -> Result<Self, PluginApiError> {
        let value = value.into();
        let segments = value.split('.').collect::<Vec<_>>();
        let valid = value.len() <= 160
            && segments.len() >= 3
            && segments.iter().all(|segment| {
                !segment.is_empty()
                    && segment.len() <= 63
                    && segment.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                    })
                    && !segment.starts_with('-')
                    && !segment.ends_with('-')
            });
        if !valid {
            return Err(PluginApiError::InvalidPluginId(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for PluginId {
    type Err = PluginApiError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginVersion(Version);

impl PluginVersion {
    pub fn parse(value: &str) -> Result<Self, PluginApiError> {
        Version::parse(value)
            .map(Self)
            .map_err(|error| PluginApiError::InvalidPluginVersion(error.to_string()))
    }

    #[must_use]
    pub const fn version(&self) -> &Version {
        &self.0
    }
}

impl fmt::Display for PluginVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for PluginVersion {
    type Err = PluginApiError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    pub fn parse(value: impl Into<String>) -> Result<Self, PluginApiError> {
        let value = value.into().to_ascii_lowercase();
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(PluginApiError::InvalidDigest(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes)))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginRuntimeKind {
    NativeRustProcess,
}

/// Truthful implementation state shipped by this exact package version.
///
/// `LiveConditional` is executable Rust code whose real-model smoke still depends on an external
/// legal checkpoint. `Unsupported` is a protocol/catalog package that must never be promoted to
/// Ready until a later package version provides a Rust-callable model runtime.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginImplementationStatus {
    #[default]
    Runnable,
    LiveConditional,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginRuntimeManifest {
    pub kind: PluginRuntimeKind,
    pub entrypoint: String,
    pub protocol: String,
    pub startup_timeout_seconds: u64,
    pub shutdown_timeout_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCompatibility {
    pub annotagent: String,
    pub targets: Vec<String>,
    #[serde(default)]
    pub accelerators: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginNetworkPermission {
    None,
    LoopbackOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginPermissions {
    pub network: PluginNetworkPermission,
    #[serde(default)]
    pub provider_secrets: bool,
    #[serde(default)]
    pub project_files: bool,
    #[serde(default)]
    pub temporary_images: bool,
    #[serde(default)]
    pub plugin_cache: bool,
    #[serde(default)]
    pub subprocesses: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginResourceLimits {
    pub minimum_memory_mb: u64,
    pub recommended_memory_mb: u64,
    pub minimum_vram_mb: u64,
    pub recommended_vram_mb: u64,
    pub maximum_response_mb: u64,
    #[serde(default = "default_concurrency")]
    pub maximum_concurrency: u32,
    #[serde(default = "default_request_count")]
    pub maximum_requests_per_process: u64,
}

const fn default_concurrency() -> u32 {
    1
}

const fn default_request_count() -> u64 {
    10_000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightProvisioning {
    None,
    LocalPath,
    LocalPathOrFixedRecipe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginWeightsManifest {
    pub bundled: bool,
    pub required: bool,
    pub provisioning: WeightProvisioning,
    #[serde(default = "default_true")]
    pub checkpoint_sha256_required: bool,
    /// Named files required by each model. An empty list preserves the single-file v1 contract.
    #[serde(default)]
    pub components: Vec<WeightComponentManifest>,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightComponentManifest {
    pub id: String,
    pub model_id: String,
    pub filename: String,
    #[serde(default)]
    pub sha256: Option<Sha256Digest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightRecipe {
    pub id: String,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub component_id: Option<String>,
    pub url: String,
    pub sha256: Sha256Digest,
    pub license_url: String,
    pub filename: String,
    pub maximum_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommercialUseDeclaration {
    Allowed,
    Restricted,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginLicenseManifest {
    pub code: String,
    pub weights: String,
    pub commercial_use: CommercialUseDeclaration,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginModelManifest {
    pub id: String,
    pub display_name: String,
    pub capabilities: Vec<ModelCapability>,
    pub input_contracts: Vec<ArtifactContract>,
    pub output_contracts: Vec<ArtifactContract>,
    pub score_semantics: ScoreSemantics,
    pub geometry_semantics: GeometrySemantics,
    #[serde(default)]
    pub runtime_requirements: RuntimeRequirements,
}

impl PluginModelManifest {
    pub fn validate(&self) -> Result<(), PluginApiError> {
        validate_identity("model id", &self.id, 160)?;
        validate_identity("model display name", &self.display_name, 160)?;
        if self.capabilities.is_empty() {
            return Err(PluginApiError::InvalidContract(format!(
                "model {} declares no capabilities",
                self.id
            )));
        }
        if self.input_contracts.is_empty() || self.output_contracts.is_empty() {
            return Err(PluginApiError::InvalidContract(format!(
                "model {} requires input and output contracts",
                self.id
            )));
        }
        let mut capabilities = std::collections::BTreeSet::new();
        for capability in &self.capabilities {
            if !capabilities.insert(*capability) {
                return Err(PluginApiError::InvalidContract(format!(
                    "model {} declares duplicate capabilities",
                    self.id
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub schema_version: String,
    pub id: PluginId,
    pub version: PluginVersion,
    pub display_name: String,
    pub description: String,
    pub publisher: String,
    pub plugin_api: String,
    #[serde(default)]
    pub implementation_status: PluginImplementationStatus,
    pub runtime: PluginRuntimeManifest,
    pub compatibility: PluginCompatibility,
    pub permissions: PluginPermissions,
    pub resources: PluginResourceLimits,
    pub models: Vec<PluginModelManifest>,
    pub weights: PluginWeightsManifest,
    #[serde(default)]
    pub weight_recipes: Vec<WeightRecipe>,
    pub license: PluginLicenseManifest,
}

impl PluginManifest {
    pub fn from_toml(source: &str) -> Result<Self, PluginApiError> {
        let manifest = toml::from_str::<Self>(source)
            .map_err(|error| PluginApiError::Serialization(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn to_toml(&self) -> Result<String, PluginApiError> {
        self.validate()?;
        toml::to_string_pretty(self)
            .map_err(|error| PluginApiError::Serialization(error.to_string()))
    }

    pub fn validate(&self) -> Result<(), PluginApiError> {
        if self.schema_version != PLUGIN_MANIFEST_SCHEMA_VERSION {
            return Err(PluginApiError::InvalidManifest(format!(
                "unsupported manifest schema {}",
                self.schema_version
            )));
        }
        if self.plugin_api != PLUGIN_API_VERSION {
            return Err(PluginApiError::InvalidManifest(format!(
                "unsupported plugin api {}",
                self.plugin_api
            )));
        }
        validate_identity("display name", &self.display_name, 160)?;
        validate_identity("description", &self.description, 2_000)?;
        validate_identity("publisher", &self.publisher, 160)?;
        validate_relative_entrypoint(&self.runtime.entrypoint)?;
        if self.runtime.kind != PluginRuntimeKind::NativeRustProcess
            || self.runtime.protocol != "http-vision-v1"
        {
            return Err(PluginApiError::InvalidManifest(
                "runtime must use the native process and HTTP Vision v1 boundary".to_owned(),
            ));
        }
        if self.runtime.startup_timeout_seconds == 0
            || self.runtime.shutdown_timeout_seconds == 0
            || self.resources.maximum_response_mb == 0
            || self.resources.maximum_concurrency == 0
            || self.resources.maximum_requests_per_process == 0
        {
            return Err(PluginApiError::InvalidManifest(
                "timeouts and resource limits must be greater than zero".to_owned(),
            ));
        }
        if self.permissions.network != PluginNetworkPermission::LoopbackOnly
            || self.permissions.provider_secrets
            || self.permissions.project_files
            || self.permissions.subprocesses
        {
            return Err(PluginApiError::InvalidManifest(
                "official process plugins require least-privilege permissions".to_owned(),
            ));
        }
        let requirement = VersionReq::parse(&self.compatibility.annotagent)
            .map_err(|error| PluginApiError::InvalidManifest(error.to_string()))?;
        if requirement.to_string().is_empty() || self.compatibility.targets.is_empty() {
            return Err(PluginApiError::InvalidManifest(
                "compatibility requires an application version range and targets".to_owned(),
            ));
        }
        if self.models.is_empty() {
            return Err(PluginApiError::InvalidManifest(
                "plugin must declare at least one model".to_owned(),
            ));
        }
        let mut models = std::collections::BTreeSet::new();
        for model in &self.models {
            model.validate()?;
            if !models.insert(model.id.as_str()) {
                return Err(PluginApiError::InvalidManifest(
                    "plugin model ids must be unique".to_owned(),
                ));
            }
        }
        if self.weights.required && self.weights.provisioning == WeightProvisioning::None {
            return Err(PluginApiError::InvalidManifest(
                "required weights need a provisioning mode".to_owned(),
            ));
        }
        let mut weight_components = std::collections::BTreeSet::new();
        for component in &self.weights.components {
            validate_identity("weight component id", &component.id, 160)?;
            validate_filename(&component.filename)?;
            if !models.contains(component.model_id.as_str()) {
                return Err(PluginApiError::InvalidManifest(format!(
                    "weight component {} references unknown model {}",
                    component.id, component.model_id
                )));
            }
            if !weight_components.insert((component.model_id.as_str(), component.id.as_str())) {
                return Err(PluginApiError::InvalidManifest(
                    "weight component ids must be unique within a model".to_owned(),
                ));
            }
        }
        if self.weights.required
            && !self.weights.components.is_empty()
            && self.models.iter().any(|model| {
                !self
                    .weights
                    .components
                    .iter()
                    .any(|component| component.model_id == model.id)
            })
        {
            return Err(PluginApiError::InvalidManifest(
                "every weighted model requires at least one declared component".to_owned(),
            ));
        }
        for recipe in &self.weight_recipes {
            validate_identity("weight recipe id", &recipe.id, 160)?;
            if !recipe.url.starts_with("https://")
                || !recipe.license_url.starts_with("https://")
                || recipe.maximum_bytes == 0
            {
                return Err(PluginApiError::InvalidManifest(
                    "weight recipes require bounded HTTPS resources".to_owned(),
                ));
            }
            validate_filename(&recipe.filename)?;
            match (&recipe.model_id, &recipe.component_id) {
                (Some(model_id), Some(component_id)) => {
                    if !weight_components.contains(&(model_id.as_str(), component_id.as_str())) {
                        return Err(PluginApiError::InvalidManifest(format!(
                            "weight recipe {} references an unknown component",
                            recipe.id
                        )));
                    }
                }
                (None, None) if self.weights.components.is_empty() => {}
                _ => {
                    return Err(PluginApiError::InvalidManifest(format!(
                        "weight recipe {} must identify both model_id and component_id",
                        recipe.id
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<Sha256Digest, PluginApiError> {
        Ok(Sha256Digest::of_bytes(self.to_toml()?.as_bytes()))
    }
}

fn validate_identity(name: &str, value: &str, maximum: usize) -> Result<(), PluginApiError> {
    if value.trim().is_empty() || value.len() > maximum || value.contains(['\r', '\n']) {
        return Err(PluginApiError::InvalidManifest(format!(
            "{name} must be non-empty, single-line and at most {maximum} bytes"
        )));
    }
    Ok(())
}

fn validate_relative_entrypoint(value: &str) -> Result<(), PluginApiError> {
    if value.is_empty()
        || value.starts_with(['/', '\\'])
        || value
            .split(['/', '\\'])
            .any(|part| part == ".." || part.is_empty())
    {
        return Err(PluginApiError::InvalidManifest(
            "runtime entrypoint must be a safe relative package path".to_owned(),
        ));
    }
    Ok(())
}

fn validate_filename(value: &str) -> Result<(), PluginApiError> {
    if value.is_empty() || value.contains(['/', '\\']) || matches!(value, "." | "..") {
        return Err(PluginApiError::InvalidManifest(
            "weight filename must be a single safe path component".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageChecksums {
    pub schema_version: String,
    pub files: BTreeMap<String, Sha256Digest>,
}

impl PackageChecksums {
    pub fn validate(&self) -> Result<(), PluginApiError> {
        if self.schema_version != "1" || self.files.is_empty() {
            return Err(PluginApiError::InvalidManifest(
                "checksum manifest must use schema 1 and contain files".to_owned(),
            ));
        }
        for path in self.files.keys() {
            validate_relative_entrypoint(path)?;
            if path == PLUGIN_CHECKSUM_FILE {
                return Err(PluginApiError::InvalidManifest(
                    "checksum manifest cannot checksum itself".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginStatus {
    Discovered,
    Installing,
    Installed,
    NeedsWeights,
    UnsupportedPlatform,
    Disabled,
    Starting,
    Ready,
    Unhealthy,
    Crashed,
    IncompatibleApi,
    InvalidManifest,
    InvalidContract,
    FailedSmokeTest,
    UpdateAvailable,
}

/// Runtime-only lifecycle status. Model asset and Model Instance readiness are independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginRuntimeStatus {
    NotInstalled,
    Installed,
    Disabled,
    Starting,
    Ready,
    Unhealthy,
    Crashed,
    Incompatible,
}

impl PluginStatus {
    #[must_use]
    pub const fn selectable(self) -> bool {
        matches!(self, Self::Ready)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginModelReference {
    pub plugin_id: PluginId,
    pub plugin_version: PluginVersion,
    pub package_digest: Sha256Digest,
    pub plugin_api_version: String,
    pub protocol_version: String,
    pub model_id: String,
    pub model_profile_revision: u64,
    pub checkpoint_sha256: Option<Sha256Digest>,
    pub capability_contract_hash: Sha256Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginReadyHandshake {
    pub status: String,
    pub plugin_api: String,
    pub protocol_version: String,
    pub listen: String,
    pub plugin_id: PluginId,
    pub session_nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginHealth {
    pub status: String,
    pub plugin_id: PluginId,
    pub plugin_version: PluginVersion,
    pub protocol_version: String,
    pub loaded_models: Vec<String>,
    pub device: String,
    pub uptime_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginRuntimeDescriptor {
    pub plugin_id: PluginId,
    pub plugin_version: PluginVersion,
    pub plugin_api: String,
    pub protocol_version: String,
    pub capabilities: Vec<ModelCapability>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRuntimeDescriptor {
    pub model: PluginModelManifest,
    pub loaded: bool,
    pub checkpoint_sha256: Option<Sha256Digest>,
    pub device: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginContracts {
    pub plugin_id: PluginId,
    pub plugin_version: PluginVersion,
    pub models: Vec<PluginModelManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarmupRequest {
    pub request_id: String,
    pub model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarmupResponse {
    pub request_id: String,
    pub model_id: String,
    pub ready: bool,
    pub duration_ms: u64,
    pub error: Option<PluginErrorBody>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelRequest {
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelResponse {
    pub request_id: String,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShutdownRequest {
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShutdownResponse {
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginErrorBody {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticatedInferenceRequest {
    pub request: PipelineInferenceRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticatedInferenceResponse {
    pub response: PipelineInferenceResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginTestReport {
    pub plugin_id: PluginId,
    pub plugin_version: PluginVersion,
    pub passed: bool,
    pub checks: Vec<PluginTestCheck>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginTestCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use annotagent_core::{ArtifactKind, ContractDataType, LicenseMetadata};

    use super::*;

    fn manifest() -> PluginManifest {
        PluginManifest {
            schema_version: "1".to_owned(),
            id: PluginId::parse("org.annotagent.dummy-detector").expect("plugin id"),
            version: PluginVersion::parse("1.0.0").expect("version"),
            display_name: "Dummy Detector".to_owned(),
            description: "Deterministic protocol conformance detector".to_owned(),
            publisher: "AnnotAgent".to_owned(),
            plugin_api: "1".to_owned(),
            implementation_status: PluginImplementationStatus::Runnable,
            runtime: PluginRuntimeManifest {
                kind: PluginRuntimeKind::NativeRustProcess,
                entrypoint: "bin/{target}/annotagent-plugin-dummy-detector".to_owned(),
                protocol: "http-vision-v1".to_owned(),
                startup_timeout_seconds: 10,
                shutdown_timeout_seconds: 5,
            },
            compatibility: PluginCompatibility {
                annotagent: ">=0.1.0,<0.2.0".to_owned(),
                targets: vec!["macos-aarch64".to_owned()],
                accelerators: vec!["cpu".to_owned()],
            },
            permissions: PluginPermissions {
                network: PluginNetworkPermission::LoopbackOnly,
                provider_secrets: false,
                project_files: false,
                temporary_images: true,
                plugin_cache: true,
                subprocesses: false,
            },
            resources: PluginResourceLimits {
                minimum_memory_mb: 64,
                recommended_memory_mb: 128,
                minimum_vram_mb: 0,
                recommended_vram_mb: 0,
                maximum_response_mb: 8,
                maximum_concurrency: 2,
                maximum_requests_per_process: 100,
            },
            models: vec![PluginModelManifest {
                id: "dummy-detector-v1".to_owned(),
                display_name: "Dummy Detector v1".to_owned(),
                capabilities: vec![ModelCapability::ObjectDetection],
                input_contracts: vec![ArtifactContract::artifact(
                    "image",
                    ArtifactKind::Image,
                    true,
                    false,
                )],
                output_contracts: vec![ArtifactContract::artifact(
                    "detections",
                    ArtifactKind::DetectionSet,
                    true,
                    false,
                )],
                score_semantics: ScoreSemantics::DetectionConfidence,
                geometry_semantics: GeometrySemantics::PredictedGeometry,
                runtime_requirements: RuntimeRequirements::default(),
            }],
            weights: PluginWeightsManifest {
                bundled: false,
                required: false,
                provisioning: WeightProvisioning::None,
                checkpoint_sha256_required: false,
                components: Vec::new(),
            },
            weight_recipes: Vec::new(),
            license: PluginLicenseManifest {
                code: "MIT".to_owned(),
                weights: "not_applicable".to_owned(),
                commercial_use: CommercialUseDeclaration::Allowed,
            },
        }
    }

    #[test]
    fn manifest_round_trip_is_stable_and_digest_changes_with_semantics() {
        let manifest = manifest();
        let encoded = manifest.to_toml().expect("toml");
        let decoded = PluginManifest::from_toml(&encoded).expect("manifest");
        assert_eq!(decoded, manifest);
        assert_eq!(
            decoded.digest().expect("digest"),
            manifest.digest().expect("digest")
        );

        let mut changed = manifest;
        changed.models[0].geometry_semantics = GeometrySemantics::RefinedGeometry;
        assert_ne!(
            changed.digest().expect("digest"),
            decoded.digest().expect("digest")
        );
    }

    #[test]
    fn manifest_rejects_unsafe_entrypoint_permissions_and_contracts() {
        let mut value = manifest();
        value.runtime.entrypoint = "../escape".to_owned();
        assert!(value.validate().is_err());

        let mut value = manifest();
        value.permissions.provider_secrets = true;
        assert!(value.validate().is_err());

        let mut value = manifest();
        value.models[0].output_contracts.clear();
        assert!(value.validate().is_err());
    }

    #[test]
    fn ids_digests_and_checksum_paths_are_strict() {
        assert!(PluginId::parse("sam").is_err());
        assert!(PluginId::parse("org.AnnotAgent.model").is_err());
        assert!(PluginVersion::parse("v1").is_err());
        assert!(Sha256Digest::parse("abcd").is_err());

        let checksums = PackageChecksums {
            schema_version: "1".to_owned(),
            files: BTreeMap::from([(
                "bin/macos-aarch64/plugin".to_owned(),
                Sha256Digest::of_bytes(b"binary"),
            )]),
        };
        checksums.validate().expect("checksums");

        let invalid = PackageChecksums {
            files: BTreeMap::from([("../escape".to_owned(), Sha256Digest::of_bytes(b"bad"))]),
            ..checksums
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn model_contract_serializes_core_types_without_brand_specific_variants() {
        let model = &manifest().models[0];
        let json = serde_json::to_value(model).expect("json");
        assert_eq!(json["capabilities"][0], "object_detection");
        assert_eq!(
            json["output_contracts"][0]["data_type"],
            serde_json::json!({"artifact":"detection_set"})
        );
        assert!(matches!(
            model.output_contracts[0].data_type,
            ContractDataType::Artifact(ArtifactKind::DetectionSet)
        ));
        let _ = LicenseMetadata::default();
    }
}
