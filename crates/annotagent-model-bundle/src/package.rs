use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use walkdir::WalkDir;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    MODEL_BUNDLE_CHECKSUM_FILE, MODEL_BUNDLE_MANIFEST_FILE, ModelBundleError, ModelBundleManifest,
    Sha256Digest,
};

pub const MODEL_BUNDLE_SIGNATURE_FILE: &str = "signatures/bundle.ed25519.sig";
pub const MAX_MODEL_BUNDLE_BYTES: u64 = 32 * 1024 * 1024 * 1024;
pub const MAX_MODEL_BUNDLE_FILE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
pub const MAX_MODEL_BUNDLE_FILE_COUNT: usize = 4_096;

#[derive(Debug, Error)]
pub enum ModelBundlePackageError {
    #[error("model bundle io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("model bundle archive is invalid: {0}")]
    Archive(#[from] zip::result::ZipError),
    #[error("model bundle manifest is invalid: {0}")]
    Manifest(#[from] ModelBundleError),
    #[error("model bundle is unsafe: {0}")]
    Unsafe(String),
    #[error("model bundle checksum mismatch: {0}")]
    Checksum(String),
    #[error("model bundle signature is invalid: {0}")]
    Signature(String),
    #[error("model bundle serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelBundleChecksums {
    pub schema_version: String,
    pub files: BTreeMap<String, Sha256Digest>,
}

impl ModelBundleChecksums {
    pub fn validate(&self) -> Result<(), ModelBundlePackageError> {
        if self.schema_version != "1" || self.files.is_empty() {
            return Err(ModelBundlePackageError::Unsafe(
                "checksums must use schema 1 and contain files".to_owned(),
            ));
        }
        for path in self.files.keys() {
            normalize_relative(Path::new(path))?;
            if path == MODEL_BUNDLE_CHECKSUM_FILE || path.starts_with("signatures/") {
                return Err(ModelBundlePackageError::Unsafe(
                    "checksums cannot include themselves or signatures".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelBundleSignatureState {
    Unsigned,
    PresentUnverified,
    Verified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBundleFile {
    pub path: String,
    pub sha256: Sha256Digest,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct VerifiedModelBundle {
    pub source: PathBuf,
    pub manifest: ModelBundleManifest,
    pub bundle_digest: Sha256Digest,
    pub signature: ModelBundleSignatureState,
    pub files: BTreeMap<String, VerifiedBundleFile>,
}

impl VerifiedModelBundle {
    /// Extracts only the verified exact file set and removes partial output after an error.
    pub fn extract_to(&self, destination: &Path) -> Result<(), ModelBundlePackageError> {
        if destination.exists() {
            return Err(ModelBundlePackageError::Unsafe(format!(
                "destination {} already exists",
                destination.display()
            )));
        }
        std::fs::create_dir_all(destination)?;
        let result = self.extract_verified(destination);
        if result.is_err() {
            let _ = std::fs::remove_dir_all(destination);
        }
        result
    }

    fn extract_verified(&self, destination: &Path) -> Result<(), ModelBundlePackageError> {
        let mut archive = ZipArchive::new(File::open(&self.source)?)?;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            if entry.is_dir() {
                continue;
            }
            let name = normalize_archive_name(entry.name())?;
            let expected = self.files.get(&name).ok_or_else(|| {
                ModelBundlePackageError::Unsafe(format!("unverified archive entry {name}"))
            })?;
            let path = destination.join(&name);
            ensure_descendant(destination, &path)?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut output = File::create(&path)?;
            let (digest, written) = copy_and_hash(&mut entry, &mut output)?;
            if digest != expected.sha256 || written != expected.size_bytes {
                return Err(ModelBundlePackageError::Checksum(name));
            }
        }
        Ok(())
    }
}

/// Packages already-prepared assets. It never converts or downloads a model.
pub fn pack_model_bundle(
    source: &Path,
    output: &Path,
) -> Result<Sha256Digest, ModelBundlePackageError> {
    let source = source.canonicalize()?;
    let manifest = ModelBundleManifest::from_toml(&std::fs::read_to_string(
        source.join(MODEL_BUNDLE_MANIFEST_FILE),
    )?)?;
    let expected = referenced_payload_paths(&manifest);
    let mut actual = BTreeSet::new();
    let mut total = 0_u64;
    for entry in WalkDir::new(&source).follow_links(false) {
        let entry = entry.map_err(|error| ModelBundlePackageError::Unsafe(error.to_string()))?;
        if entry.file_type().is_symlink() {
            return Err(ModelBundlePackageError::Unsafe(format!(
                "symbolic links are not allowed: {}",
                entry.path().display()
            )));
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = normalize_relative(entry.path().strip_prefix(&source).map_err(|_| {
            ModelBundlePackageError::Unsafe("source path escaped package root".to_owned())
        })?)?;
        if relative == MODEL_BUNDLE_CHECKSUM_FILE
            || relative == MODEL_BUNDLE_SIGNATURE_FILE
            || entry.path() == output
        {
            continue;
        }
        let size = entry
            .metadata()
            .map_err(|error| std::io::Error::other(error.to_string()))?
            .len();
        check_size(&relative, size, &mut total)?;
        actual.insert(relative);
    }
    if actual != expected || actual.len() > MAX_MODEL_BUNDLE_FILE_COUNT {
        return Err(ModelBundlePackageError::Unsafe(format!(
            "package files do not match manifest; missing={:?}, unknown={:?}",
            expected.difference(&actual).collect::<Vec<_>>(),
            actual.difference(&expected).collect::<Vec<_>>()
        )));
    }

    let checksums = ModelBundleChecksums {
        schema_version: "1".to_owned(),
        files: actual
            .iter()
            .map(|relative| Ok((relative.clone(), hash_path(&source.join(relative))?)))
            .collect::<Result<_, ModelBundlePackageError>>()?,
    };
    checksums.validate()?;
    validate_manifest_payload(&manifest, &source, &checksums.files)?;
    let checksum_bytes = serde_json::to_vec_pretty(&checksums)?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = output.with_extension(format!("annotmodel.partial-{}", uuid::Uuid::new_v4()));
    if let Err(error) = write_deterministic_archive(&source, &actual, &checksum_bytes, &temporary) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if std::fs::metadata(&temporary)?.len() > MAX_MODEL_BUNDLE_BYTES {
        let _ = std::fs::remove_file(&temporary);
        return Err(ModelBundlePackageError::Unsafe(
            "compressed package exceeds the maximum size".to_owned(),
        ));
    }
    let digest = hash_path(&temporary)?;
    std::fs::rename(temporary, output)?;
    Ok(digest)
}

pub fn verify_model_bundle(path: &Path) -> Result<VerifiedModelBundle, ModelBundlePackageError> {
    verify_model_bundle_with_key(path, None, false)
}

pub fn verify_model_bundle_with_key(
    path: &Path,
    trusted_key: Option<&VerifyingKey>,
    signature_required: bool,
) -> Result<VerifiedModelBundle, ModelBundlePackageError> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_MODEL_BUNDLE_BYTES {
        return Err(ModelBundlePackageError::Unsafe(
            "bundle must be a bounded regular file".to_owned(),
        ));
    }
    let bundle_digest = hash_path(path)?;
    let mut archive = ZipArchive::new(File::open(path)?)?;
    if archive.is_empty() || archive.len() > MAX_MODEL_BUNDLE_FILE_COUNT + 2 {
        return Err(ModelBundlePackageError::Unsafe(
            "archive file count is invalid".to_owned(),
        ));
    }

    let mut files = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut casefolded = BTreeSet::new();
    let mut total = 0_u64;
    let mut manifest_bytes = None;
    let mut checksum_bytes = None;
    let mut signature_bytes = None;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = normalize_archive_name(entry.name())?;
        if !names.insert(name.clone()) || !casefolded.insert(name.to_ascii_lowercase()) {
            return Err(ModelBundlePackageError::Unsafe(format!(
                "duplicate or case-conflicting archive path {name}"
            )));
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170_000 == 0o120_000)
        {
            return Err(ModelBundlePackageError::Unsafe(
                "archive links are not allowed".to_owned(),
            ));
        }
        check_size(&name, entry.size(), &mut total)?;
        let capture = matches!(
            name.as_str(),
            MODEL_BUNDLE_MANIFEST_FILE | MODEL_BUNDLE_CHECKSUM_FILE | MODEL_BUNDLE_SIGNATURE_FILE
        );
        let (digest, size, captured) = hash_entry(&mut entry, capture)?;
        match name.as_str() {
            MODEL_BUNDLE_MANIFEST_FILE => manifest_bytes = captured,
            MODEL_BUNDLE_CHECKSUM_FILE => checksum_bytes = captured,
            MODEL_BUNDLE_SIGNATURE_FILE => signature_bytes = captured,
            _ => {}
        }
        files.insert(
            name.clone(),
            VerifiedBundleFile {
                path: name,
                sha256: digest,
                size_bytes: size,
            },
        );
    }

    let manifest_bytes = manifest_bytes
        .ok_or_else(|| ModelBundlePackageError::Unsafe("manifest is missing".to_owned()))?;
    let manifest = ModelBundleManifest::from_toml(
        std::str::from_utf8(&manifest_bytes)
            .map_err(|_| ModelBundlePackageError::Unsafe("manifest is not UTF-8".to_owned()))?,
    )?;
    let checksum_bytes = checksum_bytes
        .ok_or_else(|| ModelBundlePackageError::Unsafe("checksums are missing".to_owned()))?;
    let checksums: ModelBundleChecksums = serde_json::from_slice(&checksum_bytes)?;
    checksums.validate()?;

    let expected_paths = referenced_payload_paths(&manifest);
    let checksum_paths = checksums.files.keys().cloned().collect::<BTreeSet<_>>();
    let archive_paths = files
        .keys()
        .filter(|path| {
            path.as_str() != MODEL_BUNDLE_CHECKSUM_FILE && !path.starts_with("signatures/")
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    if checksum_paths != expected_paths || archive_paths != expected_paths {
        return Err(ModelBundlePackageError::Checksum(
            "archive and checksums must exactly match manifest references".to_owned(),
        ));
    }
    for (name, expected) in &checksums.files {
        if files.get(name).map(|value| &value.sha256) != Some(expected) {
            return Err(ModelBundlePackageError::Checksum(name.clone()));
        }
    }
    validate_verified_manifest_payload(&manifest, &files)?;

    let signature = match (signature_bytes, trusted_key) {
        (Some(bytes), Some(key)) => {
            let signature = parse_signature(&bytes)?;
            key.verify(
                &signature_payload(&manifest_bytes, &checksum_bytes),
                &signature,
            )
            .map_err(|error| ModelBundlePackageError::Signature(error.to_string()))?;
            ModelBundleSignatureState::Verified
        }
        (Some(_), None) => ModelBundleSignatureState::PresentUnverified,
        (None, _) if signature_required => {
            return Err(ModelBundlePackageError::Signature(
                "a trusted signature is required".to_owned(),
            ));
        }
        (None, _) => ModelBundleSignatureState::Unsigned,
    };
    Ok(VerifiedModelBundle {
        source: path.to_owned(),
        manifest,
        bundle_digest,
        signature,
        files,
    })
}

#[must_use]
pub fn signature_payload(manifest: &[u8], checksums: &[u8]) -> Vec<u8> {
    let mut payload = b"annotagent-model-bundle-signature-v1\0".to_vec();
    payload.extend_from_slice(manifest);
    payload.push(0);
    payload.extend_from_slice(checksums);
    payload
}

fn parse_signature(bytes: &[u8]) -> Result<Signature, ModelBundlePackageError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ModelBundlePackageError::Signature("signature is not UTF-8".to_owned()))?;
    let decoded = BASE64
        .decode(text.trim())
        .map_err(|error| ModelBundlePackageError::Signature(error.to_string()))?;
    Signature::from_slice(&decoded)
        .map_err(|error| ModelBundlePackageError::Signature(error.to_string()))
}

fn referenced_payload_paths(manifest: &ModelBundleManifest) -> BTreeSet<String> {
    let mut paths = BTreeSet::from([MODEL_BUNDLE_MANIFEST_FILE.to_owned()]);
    for file in &manifest.files {
        paths.insert(file.path.clone());
        paths.extend(file.external_data_files.iter().cloned());
    }
    paths.extend(manifest.contracts.iter().map(|item| item.path.clone()));
    paths.extend(manifest.transforms.iter().map(|item| item.path.clone()));
    paths.insert(manifest.license.license_file.clone());
    paths.extend(manifest.license.source_notice.iter().cloned());
    paths.extend(manifest.test_suite.input_artifacts.iter().cloned());
    paths.insert(manifest.test_suite.expected_summary.clone());
    paths.insert(manifest.test_suite.tolerances.clone());
    paths
}

fn validate_manifest_payload(
    manifest: &ModelBundleManifest,
    source: &Path,
    checksums: &BTreeMap<String, Sha256Digest>,
) -> Result<(), ModelBundlePackageError> {
    for model in &manifest.files {
        if std::fs::metadata(source.join(&model.path))?.len() != model.size_bytes
            || checksums.get(&model.path) != Some(&model.sha256)
        {
            return Err(ModelBundlePackageError::Checksum(model.path.clone()));
        }
    }
    validate_declared_hashes(manifest, |path| checksums.get(path).cloned())
}

fn validate_verified_manifest_payload(
    manifest: &ModelBundleManifest,
    files: &BTreeMap<String, VerifiedBundleFile>,
) -> Result<(), ModelBundlePackageError> {
    for model in &manifest.files {
        let actual = files
            .get(&model.path)
            .ok_or_else(|| ModelBundlePackageError::Checksum(model.path.clone()))?;
        if actual.size_bytes != model.size_bytes || actual.sha256 != model.sha256 {
            return Err(ModelBundlePackageError::Checksum(model.path.clone()));
        }
    }
    validate_declared_hashes(manifest, |path| {
        files.get(path).map(|file| file.sha256.clone())
    })
}

fn validate_declared_hashes(
    manifest: &ModelBundleManifest,
    digest: impl Fn(&str) -> Option<Sha256Digest>,
) -> Result<(), ModelBundlePackageError> {
    for contract in &manifest.contracts {
        if digest(&contract.path) != Some(contract.sha256.clone()) {
            return Err(ModelBundlePackageError::Checksum(contract.path.clone()));
        }
    }
    for transform in &manifest.transforms {
        if digest(&transform.path) != Some(transform.sha256.clone()) {
            return Err(ModelBundlePackageError::Checksum(transform.path.clone()));
        }
    }
    if digest(&manifest.license.license_file) != Some(manifest.license.license_digest.clone()) {
        return Err(ModelBundlePackageError::Checksum(
            manifest.license.license_file.clone(),
        ));
    }
    Ok(())
}

fn write_deterministic_archive(
    source: &Path,
    paths: &BTreeSet<String>,
    checksums: &[u8],
    output: &Path,
) -> Result<(), ModelBundlePackageError> {
    let mut writer = ZipWriter::new(File::create(output)?);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    for path in paths {
        writer.start_file(path, options)?;
        std::io::copy(&mut File::open(source.join(path))?, &mut writer)?;
    }
    writer.start_file(MODEL_BUNDLE_CHECKSUM_FILE, options)?;
    writer.write_all(checksums)?;
    writer.finish()?.sync_all()?;
    Ok(())
}

fn check_size(name: &str, size: u64, total: &mut u64) -> Result<(), ModelBundlePackageError> {
    if size == 0 || size > MAX_MODEL_BUNDLE_FILE_BYTES {
        return Err(ModelBundlePackageError::Unsafe(format!(
            "archive entry {name} has an invalid size"
        )));
    }
    *total = total.saturating_add(size);
    if *total > MAX_MODEL_BUNDLE_BYTES {
        return Err(ModelBundlePackageError::Unsafe(
            "expanded archive exceeds the maximum size".to_owned(),
        ));
    }
    Ok(())
}

fn hash_entry<R: Read>(
    reader: &mut R,
    capture: bool,
) -> Result<(Sha256Digest, u64, Option<Vec<u8>>), ModelBundlePackageError> {
    use sha2::Digest as _;
    let mut digest = sha2::Sha256::new();
    let mut total = 0_u64;
    let mut captured = capture.then(Vec::new);
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        if let Some(bytes) = &mut captured {
            bytes.extend_from_slice(&buffer[..read]);
        }
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
    }
    Ok((
        Sha256Digest::parse(format!("{:x}", digest.finalize()))?,
        total,
        captured,
    ))
}

fn copy_and_hash<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
) -> Result<(Sha256Digest, u64), ModelBundlePackageError> {
    use sha2::Digest as _;
    let mut digest = sha2::Sha256::new();
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        digest.update(&buffer[..read]);
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
    }
    Ok((
        Sha256Digest::parse(format!("{:x}", digest.finalize()))?,
        total,
    ))
}

fn hash_path(path: &Path) -> Result<Sha256Digest, ModelBundlePackageError> {
    let (digest, _, _) = hash_entry(&mut File::open(path)?, false)?;
    Ok(digest)
}

fn normalize_archive_name(value: &str) -> Result<String, ModelBundlePackageError> {
    if value.contains('\\') {
        return Err(ModelBundlePackageError::Unsafe(
            "archive paths must use forward slashes".to_owned(),
        ));
    }
    normalize_relative(Path::new(value))
}

fn normalize_relative(path: &Path) -> Result<String, ModelBundlePackageError> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ModelBundlePackageError::Unsafe(format!(
            "unsafe package path {}",
            path.display()
        )));
    }
    let value = path
        .to_str()
        .ok_or_else(|| ModelBundlePackageError::Unsafe("path is not UTF-8".to_owned()))?
        .replace(std::path::MAIN_SEPARATOR, "/");
    if value.is_empty() {
        return Err(ModelBundlePackageError::Unsafe(
            "package path cannot be empty".to_owned(),
        ));
    }
    Ok(value)
}

fn ensure_descendant(root: &Path, path: &Path) -> Result<(), ModelBundlePackageError> {
    if !path.starts_with(root) || path == root {
        return Err(ModelBundlePackageError::Unsafe(
            "package path escapes installation root".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use annotagent_core::ModelCapability;
    use annotagent_plugin_api::PluginId;
    use ed25519_dalek::{Signer as _, SigningKey};
    use rand::rngs::OsRng;
    use semver::Version;
    use zip::write::SimpleFileOptions;

    use crate::{
        CommercialUseStatus, ModelBundleFile, ModelBundleId, ModelContractReference,
        ModelExportMetadata, ModelFileRole, ModelFormat, ModelLicenseMetadata,
        ModelRuntimeMetadata, ModelSourceMetadata, ModelTestSuiteReference,
        ModelTransformReference, PluginCompatibilityRequirement, RedistributionStatus,
    };

    use super::*;

    fn source(root: &Path) -> ModelBundleManifest {
        let payloads = BTreeMap::from([
            ("files/model.onnx", b"fixture ONNX bytes".as_slice()),
            ("contracts/model-contract.json", b"{}".as_slice()),
            ("transforms/preprocessing.json", b"{}".as_slice()),
            ("transforms/postprocessing.json", b"{}".as_slice()),
            ("tests/input-image.png", b"png fixture".as_slice()),
            ("tests/prompts.json", b"{}".as_slice()),
            ("tests/expected-summary.json", b"{}".as_slice()),
            ("tests/tolerances.json", b"{}".as_slice()),
            ("licenses/MODEL-LICENSE", b"CC0".as_slice()),
            ("licenses/SOURCE-NOTICE", b"generated fixture".as_slice()),
        ]);
        for (path, bytes) in &payloads {
            let target = root.join(path);
            std::fs::create_dir_all(target.parent().expect("parent")).expect("dirs");
            std::fs::write(target, bytes).expect("write");
        }
        let role = ModelFileRole::parse("model").expect("role");
        let manifest = ModelBundleManifest {
            schema_version: "1".to_owned(),
            id: ModelBundleId::parse("org.annotagent.models.fixture-prompted-segmentation")
                .expect("id"),
            version: Version::new(1, 0, 0),
            display_name: "Fixture prompted segmentation".to_owned(),
            description: Some("Fixture".to_owned()),
            model_family: "fixture".to_owned(),
            architecture: "identity-mask".to_owned(),
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
                sha256: Sha256Digest::of_bytes(payloads["files/model.onnx"]),
                size_bytes: payloads["files/model.onnx"].len() as u64,
                external_data_files: Vec::new(),
            }],
            contracts: vec![ModelContractReference {
                id: "contract".to_owned(),
                path: "contracts/model-contract.json".to_owned(),
                sha256: Sha256Digest::of_bytes(payloads["contracts/model-contract.json"]),
                file_roles: BTreeSet::from([role]),
            }],
            transforms: vec![
                ModelTransformReference {
                    kind: "preprocessing".to_owned(),
                    path: "transforms/preprocessing.json".to_owned(),
                    sha256: Sha256Digest::of_bytes(payloads["transforms/preprocessing.json"]),
                },
                ModelTransformReference {
                    kind: "postprocessing".to_owned(),
                    path: "transforms/postprocessing.json".to_owned(),
                    sha256: Sha256Digest::of_bytes(payloads["transforms/postprocessing.json"]),
                },
            ],
            source: ModelSourceMetadata {
                upstream_project: "AnnotAgent".to_owned(),
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
                source_notice: Some("licenses/SOURCE-NOTICE".to_owned()),
                license_digest: Sha256Digest::of_bytes(payloads["licenses/MODEL-LICENSE"]),
                redistribution: RedistributionStatus::Allowed,
                commercial_use: CommercialUseStatus::Allowed,
                requires_acceptance: true,
                usage_notes: vec!["Fixture only".to_owned()],
            },
            test_suite: ModelTestSuiteReference {
                test_id: "fixture".to_owned(),
                input_artifacts: vec![
                    "tests/input-image.png".to_owned(),
                    "tests/prompts.json".to_owned(),
                ],
                expected_summary: "tests/expected-summary.json".to_owned(),
                tolerances: "tests/tolerances.json".to_owned(),
            },
            fixture: true,
            publishable: false,
        };
        std::fs::write(
            root.join(MODEL_BUNDLE_MANIFEST_FILE),
            manifest.to_toml().expect("manifest"),
        )
        .expect("manifest");
        manifest
    }

    #[test]
    fn deterministic_pack_verify_and_extract_round_trip() {
        let temp = tempfile::tempdir().expect("temp");
        let directory = temp.path().join("source");
        std::fs::create_dir_all(&directory).expect("source");
        let manifest = source(&directory);
        let first = temp.path().join("first.annotmodel");
        let second = temp.path().join("second.annotmodel");
        let digest = pack_model_bundle(&directory, &first).expect("pack");
        assert_eq!(
            digest,
            pack_model_bundle(&directory, &second).expect("pack")
        );
        let verified = verify_model_bundle(&first).expect("verify");
        assert_eq!(verified.bundle_digest, digest);
        assert_eq!(verified.manifest, manifest);
        assert_eq!(verified.signature, ModelBundleSignatureState::Unsigned);
        let extracted = temp.path().join("extracted");
        verified.extract_to(&extracted).expect("extract");
        assert_eq!(
            std::fs::read(extracted.join("files/model.onnx")).expect("model"),
            b"fixture ONNX bytes"
        );
    }

    #[test]
    fn pack_rejects_unknown_files_and_manifest_hash_mismatch() {
        let temp = tempfile::tempdir().expect("temp");
        let directory = temp.path().join("source");
        std::fs::create_dir_all(&directory).expect("source");
        source(&directory);
        std::fs::write(directory.join("unknown.bin"), b"unknown").expect("unknown");
        assert!(pack_model_bundle(&directory, &temp.path().join("bad.annotmodel")).is_err());
        std::fs::remove_file(directory.join("unknown.bin")).expect("remove");
        std::fs::write(directory.join("files/model.onnx"), b"tampered").expect("tamper");
        assert!(pack_model_bundle(&directory, &temp.path().join("bad.annotmodel")).is_err());
    }

    #[test]
    fn verifier_rejects_traversal_duplicate_case_conflict_and_symlink() {
        for (name, entries) in [
            ("traversal", vec![("../escape", None)]),
            ("duplicate", vec![("same", None), ("same", None)]),
            ("case", vec![("Files/model", None), ("files/model", None)]),
            ("symlink", vec![("files/link", Some(0o120_777))]),
        ] {
            let temp = tempfile::tempdir().expect("temp");
            let package = temp.path().join(format!("{name}.annotmodel"));
            let mut writer = ZipWriter::new(File::create(&package).expect("file"));
            let mut writer_rejected = false;
            for (path, mode) in entries {
                let options = SimpleFileOptions::default().unix_permissions(mode.unwrap_or(0o644));
                if writer.start_file(path, options).is_err() {
                    writer_rejected = true;
                    break;
                }
                writer.write_all(b"x").expect("bytes");
            }
            if writer_rejected {
                continue;
            }
            writer.finish().expect("finish");
            assert!(verify_model_bundle(&package).is_err(), "{name}");
        }
    }

    #[test]
    fn trusted_signature_is_verified_and_wrong_key_is_rejected() {
        let temp = tempfile::tempdir().expect("temp");
        let directory = temp.path().join("source");
        std::fs::create_dir_all(&directory).expect("source");
        source(&directory);
        let unsigned = temp.path().join("unsigned.annotmodel");
        pack_model_bundle(&directory, &unsigned).expect("pack");
        let (manifest, checksums) = signature_inputs(&unsigned);
        let signing = SigningKey::generate(&mut OsRng);
        let signature = signing.sign(&signature_payload(&manifest, &checksums));
        let signed = temp.path().join("signed.annotmodel");
        append_signature(&unsigned, &signed, &BASE64.encode(signature.to_bytes()));
        assert_eq!(
            verify_model_bundle_with_key(&signed, Some(&signing.verifying_key()), true)
                .expect("verified")
                .signature,
            ModelBundleSignatureState::Verified
        );
        let other = SigningKey::generate(&mut OsRng);
        assert!(verify_model_bundle_with_key(&signed, Some(&other.verifying_key()), true).is_err());
        assert!(verify_model_bundle_with_key(&unsigned, None, true).is_err());
    }

    fn signature_inputs(package: &Path) -> (Vec<u8>, Vec<u8>) {
        let mut archive = ZipArchive::new(File::open(package).expect("open")).expect("archive");
        let mut manifest = Vec::new();
        archive
            .by_name(MODEL_BUNDLE_MANIFEST_FILE)
            .expect("manifest")
            .read_to_end(&mut manifest)
            .expect("read");
        let mut checksums = Vec::new();
        archive
            .by_name(MODEL_BUNDLE_CHECKSUM_FILE)
            .expect("checksums")
            .read_to_end(&mut checksums)
            .expect("read");
        (manifest, checksums)
    }

    fn append_signature(source: &Path, destination: &Path, signature: &str) {
        let mut input = ZipArchive::new(File::open(source).expect("open")).expect("archive");
        let mut writer = ZipWriter::new(File::create(destination).expect("destination"));
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(0o644);
        for index in 0..input.len() {
            let mut entry = input.by_index(index).expect("entry");
            writer.start_file(entry.name(), options).expect("entry");
            std::io::copy(&mut entry, &mut writer).expect("copy");
        }
        writer
            .start_file(MODEL_BUNDLE_SIGNATURE_FILE, options)
            .expect("signature");
        writer.write_all(signature.as_bytes()).expect("signature");
        writer.finish().expect("finish");
    }
}
