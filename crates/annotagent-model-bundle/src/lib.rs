//! Versioned, model-neutral asset manifests for installable `.annotmodel` packages.
//!
//! A bundle contains data only. Executable loading, transforms and inference stay in a separately
//! versioned Rust `.annotplugin`; an installed and verified pairing becomes a Model Instance.

use std::{collections::BTreeSet, fmt, path::Path, str::FromStr};

use annotagent_core::ModelCapability;
use annotagent_plugin_api::{PluginId, PluginVersion};
use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use url::Url;

pub const MODEL_BUNDLE_MANIFEST_SCHEMA_VERSION: &str = "1";
pub const MODEL_BUNDLE_MANIFEST_FILE: &str = "annotagent-model.toml";
pub const MODEL_BUNDLE_CHECKSUM_FILE: &str = "checksums.json";
pub const MODEL_BUNDLE_EXTENSION: &str = "annotmodel";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModelBundleError {
    #[error("invalid model bundle id: {0}")]
    InvalidBundleId(String),
    #[error("invalid model file role: {0}")]
    InvalidFileRole(String),
    #[error("invalid SHA-256 digest: {0}")]
    InvalidDigest(String),
    #[error("invalid model bundle manifest: {0}")]
    InvalidManifest(String),
    #[error("model bundle serialization failed: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelBundleId(String);

impl ModelBundleId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ModelBundleError> {
        let value = value.into();
        let segments = value.split('.').collect::<Vec<_>>();
        let valid = value.len() <= 180
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
            return Err(ModelBundleError::InvalidBundleId(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelBundleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ModelBundleId {
    type Err = ModelBundleError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// A plugin-defined semantic file role such as `image_encoder` or `mask_decoder`.
///
/// Roles intentionally are not a Core enum. New model families can introduce roles without an
/// `AnnotAgent` release, while this type still rejects ambiguous or unsafe identifiers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelFileRole(String);

impl ModelFileRole {
    pub fn parse(value: impl Into<String>) -> Result<Self, ModelBundleError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 80
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte)
            })
            && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            && value
                .as_bytes()
                .last()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
        if !valid {
            return Err(ModelBundleError::InvalidFileRole(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelFileRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ModelFileRole {
    type Err = ModelBundleError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    pub fn parse(value: impl Into<String>) -> Result<Self, ModelBundleError> {
        let value = value.into().to_ascii_lowercase();
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ModelBundleError::InvalidDigest(value));
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

impl FromStr for Sha256Digest {
    type Err = ModelBundleError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    Onnx,
    Safetensors,
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelBundleStatus {
    NotInstalled,
    AvailableInCatalog,
    LicenseAcceptanceRequired,
    Downloading,
    Importing,
    Verifying,
    Installed,
    IncompatiblePlugin,
    InvalidManifest,
    InvalidChecksum,
    InvalidContract,
    UnsupportedPlatform,
    Corrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelInstanceStatus {
    Unresolved,
    MissingPlugin,
    MissingModelBundle,
    Preparing,
    SmokeTesting,
    Ready,
    FailedSmokeTest,
    PluginUnavailable,
    ContractMismatch,
    Disabled,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCompatibilityRequirement {
    pub plugin_id: PluginId,
    pub plugin_version: String,
    pub model_id: String,
    pub contract_hash: Sha256Digest,
    #[serde(default)]
    pub required_file_roles: BTreeSet<ModelFileRole>,
}

impl PluginCompatibilityRequirement {
    pub fn version_requirement(&self) -> Result<VersionReq, ModelBundleError> {
        VersionReq::parse(&self.plugin_version).map_err(|error| {
            ModelBundleError::InvalidManifest(format!(
                "plugin {} has invalid version requirement: {error}",
                self.plugin_id
            ))
        })
    }

    #[must_use]
    pub fn accepts(&self, plugin_id: &PluginId, version: &PluginVersion, model_id: &str) -> bool {
        self.plugin_id == *plugin_id
            && self.model_id == model_id
            && self
                .version_requirement()
                .is_ok_and(|requirement| requirement.matches(version.version()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelBundleFile {
    pub role: ModelFileRole,
    pub path: String,
    pub sha256: Sha256Digest,
    pub size_bytes: u64,
    #[serde(default)]
    pub external_data_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelContractReference {
    pub id: String,
    pub path: String,
    pub sha256: Sha256Digest,
    pub file_roles: BTreeSet<ModelFileRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSourceMetadata {
    pub upstream_project: String,
    pub upstream_model_id: String,
    pub upstream_version: Option<String>,
    pub upstream_checkpoint_sha256: Option<Sha256Digest>,
    pub source_url: Option<Url>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumericalValidationSummary {
    pub sample_count: u64,
    pub maximum_absolute_error: f64,
    pub maximum_relative_error: f64,
    pub reference_runtime: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelExportMetadata {
    pub exporter_name: String,
    pub exporter_version: String,
    pub exporter_revision: Option<String>,
    pub export_date: Option<DateTime<Utc>>,
    pub opset: Option<u32>,
    pub numerical_validation: Option<NumericalValidationSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRuntimeMetadata {
    pub execution_providers: BTreeSet<String>,
    pub platforms: BTreeSet<String>,
    pub minimum_memory_mb: u64,
    pub recommended_memory_mb: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedistributionStatus {
    Allowed,
    Restricted,
    Prohibited,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommercialUseStatus {
    Allowed,
    Restricted,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelLicenseMetadata {
    pub name: String,
    pub license_url: Option<Url>,
    pub license_file: String,
    pub license_digest: Sha256Digest,
    pub redistribution: RedistributionStatus,
    pub commercial_use: CommercialUseStatus,
    pub requires_acceptance: bool,
    pub usage_notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelTestSuiteReference {
    pub test_id: String,
    pub input_artifacts: Vec<String>,
    pub expected_summary: String,
    pub tolerances: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelBundleManifest {
    pub schema_version: String,
    pub id: ModelBundleId,
    pub version: Version,
    pub display_name: String,
    pub description: Option<String>,
    pub model_family: String,
    pub architecture: String,
    pub format: ModelFormat,
    pub variant: String,
    pub capabilities: BTreeSet<ModelCapability>,
    pub compatible_plugins: Vec<PluginCompatibilityRequirement>,
    pub files: Vec<ModelBundleFile>,
    pub contracts: Vec<ModelContractReference>,
    pub source: ModelSourceMetadata,
    pub export: ModelExportMetadata,
    pub runtime: ModelRuntimeMetadata,
    pub license: ModelLicenseMetadata,
    pub test_suite: ModelTestSuiteReference,
    #[serde(default)]
    pub fixture: bool,
    #[serde(default = "default_true")]
    pub publishable: bool,
}

const fn default_true() -> bool {
    true
}

impl ModelBundleManifest {
    pub fn from_toml(source: &str) -> Result<Self, ModelBundleError> {
        let manifest = toml::from_str::<Self>(source)
            .map_err(|error| ModelBundleError::Serialization(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn to_toml(&self) -> Result<String, ModelBundleError> {
        self.validate()?;
        toml::to_string_pretty(self)
            .map_err(|error| ModelBundleError::Serialization(error.to_string()))
    }

    pub fn validate(&self) -> Result<(), ModelBundleError> {
        if self.schema_version != MODEL_BUNDLE_MANIFEST_SCHEMA_VERSION {
            return invalid(format!(
                "unsupported schema version {}",
                self.schema_version
            ));
        }
        if self.version == Version::new(0, 0, 0) {
            return invalid("bundle version 0.0.0 is not installable".to_owned());
        }
        for (field, value, maximum) in [
            ("display name", self.display_name.as_str(), 160),
            ("model family", self.model_family.as_str(), 160),
            ("architecture", self.architecture.as_str(), 160),
            ("variant", self.variant.as_str(), 160),
        ] {
            validate_text(field, value, maximum)?;
        }
        if self.capabilities.is_empty() {
            return invalid("at least one capability is required".to_owned());
        }
        if self.compatible_plugins.is_empty() {
            return invalid("at least one compatible plugin is required".to_owned());
        }
        if self.files.is_empty() || self.contracts.is_empty() {
            return invalid("model files and contracts are required".to_owned());
        }
        if self.fixture == self.publishable {
            return invalid(
                "fixture bundles must be non-publishable and release bundles must not be fixtures"
                    .to_owned(),
            );
        }

        let mut roles = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for file in &self.files {
            validate_bundle_path(&file.path, "files/")?;
            if file.size_bytes == 0 {
                return invalid(format!("model file {} cannot be empty", file.path));
            }
            if !roles.insert(file.role.clone()) {
                return invalid(format!("duplicate model file role {}", file.role));
            }
            if !paths.insert(file.path.as_str()) {
                return invalid(format!("duplicate model file path {}", file.path));
            }
            for external in &file.external_data_files {
                validate_bundle_path(external, "files/")?;
                if !paths.insert(external.as_str()) {
                    return invalid(format!("duplicate model file path {external}"));
                }
            }
        }

        let mut contract_ids = BTreeSet::new();
        for contract in &self.contracts {
            validate_text("contract id", &contract.id, 160)?;
            validate_bundle_path(&contract.path, "contracts/")?;
            if !contract_ids.insert(contract.id.as_str()) {
                return invalid(format!("duplicate contract id {}", contract.id));
            }
            if contract.file_roles.is_empty()
                || !contract.file_roles.iter().all(|role| roles.contains(role))
            {
                return invalid(format!(
                    "contract {} references no roles or an undeclared role",
                    contract.id
                ));
            }
        }

        for requirement in &self.compatible_plugins {
            requirement.version_requirement()?;
            validate_text("plugin model id", &requirement.model_id, 160)?;
            if requirement.required_file_roles.is_empty()
                || !requirement
                    .required_file_roles
                    .iter()
                    .all(|role| roles.contains(role))
            {
                return invalid(format!(
                    "plugin {} requires no roles or an undeclared role",
                    requirement.plugin_id
                ));
            }
        }

        validate_text("upstream project", &self.source.upstream_project, 240)?;
        validate_text("upstream model id", &self.source.upstream_model_id, 240)?;
        validate_text("exporter name", &self.export.exporter_name, 160)?;
        validate_text("exporter version", &self.export.exporter_version, 160)?;
        if self.format == ModelFormat::Onnx && self.export.opset.is_none() {
            return invalid("ONNX bundles must declare an opset".to_owned());
        }
        if self.runtime.execution_providers.is_empty() || self.runtime.platforms.is_empty() {
            return invalid("runtime providers and platforms are required".to_owned());
        }
        if self.runtime.minimum_memory_mb == 0
            || self.runtime.recommended_memory_mb < self.runtime.minimum_memory_mb
        {
            return invalid("runtime memory requirements are invalid".to_owned());
        }
        validate_text("license name", &self.license.name, 240)?;
        validate_bundle_path(&self.license.license_file, "licenses/")?;
        if self.license.redistribution == RedistributionStatus::Prohibited && self.publishable {
            return invalid("a redistribution-prohibited bundle cannot be publishable".to_owned());
        }
        validate_text("test id", &self.test_suite.test_id, 160)?;
        if self.test_suite.input_artifacts.is_empty() {
            return invalid("a smoke test requires input artifacts".to_owned());
        }
        for path in &self.test_suite.input_artifacts {
            validate_bundle_path(path, "tests/")?;
        }
        validate_bundle_path(&self.test_suite.expected_summary, "tests/")?;
        validate_bundle_path(&self.test_suite.tolerances, "tests/")?;
        Ok(())
    }

    pub fn digest(&self) -> Result<Sha256Digest, ModelBundleError> {
        Ok(Sha256Digest::of_bytes(self.to_toml()?.as_bytes()))
    }
}

fn invalid<T>(message: String) -> Result<T, ModelBundleError> {
    Err(ModelBundleError::InvalidManifest(message))
}

fn validate_text(field: &str, value: &str, maximum: usize) -> Result<(), ModelBundleError> {
    if value.trim().is_empty() || value.len() > maximum || value.contains(['\r', '\n']) {
        return invalid(format!(
            "{field} must be non-empty, single-line and at most {maximum} bytes"
        ));
    }
    Ok(())
}

pub fn validate_bundle_path(path: &str, required_prefix: &str) -> Result<(), ModelBundleError> {
    let parsed = Path::new(path);
    if !path.starts_with(required_prefix)
        || path.contains('\\')
        || parsed.is_absolute()
        || parsed
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return invalid(format!(
            "bundle path {path} must be a safe relative path under {required_prefix}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use annotagent_core::ModelCapability;
    use annotagent_plugin_api::PluginId;

    use super::*;

    fn digest(value: &str) -> Sha256Digest {
        Sha256Digest::of_bytes(value.as_bytes())
    }

    fn manifest() -> ModelBundleManifest {
        let encoder = ModelFileRole::parse("image_encoder").expect("role");
        let decoder = ModelFileRole::parse("mask_decoder").expect("role");
        ModelBundleManifest {
            schema_version: "1".to_owned(),
            id: ModelBundleId::parse("org.annotagent.models.fixture-prompted-segmentation")
                .expect("id"),
            version: Version::new(1, 0, 0),
            display_name: "Fixture prompted segmentation".to_owned(),
            description: Some("Offline lifecycle fixture".to_owned()),
            model_family: "fixture".to_owned(),
            architecture: "identity-mask".to_owned(),
            format: ModelFormat::Onnx,
            variant: "tiny".to_owned(),
            capabilities: BTreeSet::from([ModelCapability::PromptedSegmentation]),
            compatible_plugins: vec![PluginCompatibilityRequirement {
                plugin_id: PluginId::parse("org.annotagent.sam-onnx").expect("plugin"),
                plugin_version: ">=1.0.0,<2.0.0".to_owned(),
                model_id: "sam-vit-b-onnx".to_owned(),
                contract_hash: digest("plugin-contract"),
                required_file_roles: BTreeSet::from([encoder.clone(), decoder.clone()]),
            }],
            files: vec![
                ModelBundleFile {
                    role: encoder.clone(),
                    path: "files/image_encoder.onnx".to_owned(),
                    sha256: digest("encoder"),
                    size_bytes: 7,
                    external_data_files: Vec::new(),
                },
                ModelBundleFile {
                    role: decoder.clone(),
                    path: "files/mask_decoder.onnx".to_owned(),
                    sha256: digest("decoder"),
                    size_bytes: 7,
                    external_data_files: Vec::new(),
                },
            ],
            contracts: vec![ModelContractReference {
                id: "fixture-contract".to_owned(),
                path: "contracts/model-contract.json".to_owned(),
                sha256: digest("contract"),
                file_roles: BTreeSet::from([encoder, decoder]),
            }],
            source: ModelSourceMetadata {
                upstream_project: "AnnotAgent fixtures".to_owned(),
                upstream_model_id: "identity-mask".to_owned(),
                upstream_version: Some("1".to_owned()),
                upstream_checkpoint_sha256: None,
                source_url: None,
            },
            export: ModelExportMetadata {
                exporter_name: "fixture generator".to_owned(),
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
                license_digest: digest("CC0"),
                redistribution: RedistributionStatus::Allowed,
                commercial_use: CommercialUseStatus::Allowed,
                requires_acceptance: true,
                usage_notes: vec!["Fixture only".to_owned()],
            },
            test_suite: ModelTestSuiteReference {
                test_id: "fixture-box-prompt".to_owned(),
                input_artifacts: vec![
                    "tests/input-image.png".to_owned(),
                    "tests/prompts.json".to_owned(),
                ],
                expected_summary: "tests/expected-summary.json".to_owned(),
                tolerances: "tests/tolerances.json".to_owned(),
            },
            fixture: true,
            publishable: false,
        }
    }

    #[test]
    fn manifest_round_trips_with_generic_multi_file_roles() {
        let manifest = manifest();
        manifest.validate().expect("valid");
        let encoded = manifest.to_toml().expect("serialize");
        let decoded = ModelBundleManifest::from_toml(&encoded).expect("deserialize");
        assert_eq!(decoded, manifest);
        assert_eq!(decoded.files[0].role.as_str(), "image_encoder");
        assert!(decoded.compatible_plugins[0].accepts(
            &PluginId::parse("org.annotagent.sam-onnx").expect("plugin"),
            &PluginVersion::parse("1.2.3").expect("version"),
            "sam-vit-b-onnx"
        ));
    }

    #[test]
    fn role_is_not_a_brand_specific_enum_but_still_rejects_unsafe_values() {
        assert!(ModelFileRole::parse("depth_auxiliary_2").is_ok());
        assert!(ModelFileRole::parse("SamImageEncoder").is_err());
        assert!(ModelFileRole::parse("../model").is_err());
        assert!(ModelFileRole::parse("_encoder").is_err());
    }

    #[test]
    fn validation_rejects_duplicate_or_missing_roles() {
        let mut duplicate = manifest();
        duplicate.files[1].role = duplicate.files[0].role.clone();
        assert!(duplicate.validate().is_err());

        let mut missing = manifest();
        missing.compatible_plugins[0]
            .required_file_roles
            .insert(ModelFileRole::parse("new_auxiliary").expect("role"));
        assert!(missing.validate().is_err());
    }

    #[test]
    fn validation_rejects_unsafe_paths_and_invalid_fixture_claims() {
        let mut unsafe_path = manifest();
        unsafe_path.files[0].path = "files/../escape.onnx".to_owned();
        assert!(unsafe_path.validate().is_err());

        let mut false_release = manifest();
        false_release.publishable = true;
        assert!(false_release.validate().is_err());
    }

    #[test]
    fn unknown_manifest_fields_are_rejected() {
        let source = format!("{}\nunknown = true\n", manifest().to_toml().expect("toml"));
        assert!(ModelBundleManifest::from_toml(&source).is_err());
    }
}
