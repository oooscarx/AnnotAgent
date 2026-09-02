use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
};

use annotagent_plugin_api::{
    PLUGIN_CHECKSUM_FILE, PLUGIN_MANIFEST_FILE, PackageChecksums, PluginManifest, Sha256Digest,
};
use thiserror::Error;
use walkdir::WalkDir;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

const MAX_PACKAGE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_FILE_COUNT: usize = 10_000;

#[derive(Debug, Error)]
pub enum PluginPackageError {
    #[error("plugin package io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("plugin archive is invalid: {0}")]
    Archive(#[from] zip::result::ZipError),
    #[error("plugin manifest is invalid: {0}")]
    Manifest(#[from] annotagent_plugin_api::PluginApiError),
    #[error("plugin package is unsafe: {0}")]
    Unsafe(String),
    #[error("plugin package checksum mismatch: {0}")]
    Checksum(String),
    #[error("plugin package is incompatible: {0}")]
    Incompatible(String),
    #[error("plugin package serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSignatureState {
    Unsigned,
    PresentUnverified,
}

#[derive(Debug, Clone)]
pub struct VerifiedPluginPackage {
    pub source: PathBuf,
    pub manifest: PluginManifest,
    pub package_digest: Sha256Digest,
    pub signature: PackageSignatureState,
    files: BTreeMap<String, Vec<u8>>,
}

impl VerifiedPluginPackage {
    pub fn extract_to(&self, destination: &Path) -> Result<(), PluginPackageError> {
        if destination.exists() {
            return Err(PluginPackageError::Unsafe(format!(
                "destination {} already exists",
                destination.display()
            )));
        }
        std::fs::create_dir_all(destination)?;
        for (relative, bytes) in &self.files {
            let path = destination.join(relative);
            ensure_descendant(destination, &path)?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, bytes)?;
            if relative.starts_with("bin/") {
                make_executable(&path)?;
            }
        }
        Ok(())
    }

    pub fn executable_path(&self, installation_root: &Path) -> Result<PathBuf, PluginPackageError> {
        let relative = self
            .manifest
            .runtime
            .entrypoint
            .replace("{target}", &current_target());
        let path = installation_root.join(relative);
        ensure_descendant(installation_root, &path)?;
        if !path.is_file() {
            return Err(PluginPackageError::Incompatible(format!(
                "package has no executable for target {}",
                current_target()
            )));
        }
        Ok(path)
    }
}

pub fn pack_directory(source: &Path, output: &Path) -> Result<Sha256Digest, PluginPackageError> {
    let source = source.canonicalize()?;
    let manifest_path = source.join(PLUGIN_MANIFEST_FILE);
    let manifest = PluginManifest::from_toml(&std::fs::read_to_string(&manifest_path)?)?;
    validate_current_target(&manifest)?;

    let mut files = BTreeMap::new();
    for entry in WalkDir::new(&source).follow_links(false) {
        let entry = entry.map_err(|error| PluginPackageError::Unsafe(error.to_string()))?;
        if entry.file_type().is_symlink() {
            return Err(PluginPackageError::Unsafe(format!(
                "symbolic links are not allowed: {}",
                entry.path().display()
            )));
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(&source)
            .map_err(|_| PluginPackageError::Unsafe("path escaped source".to_owned()))?;
        let relative = normalize_relative(relative)?;
        if relative == PLUGIN_CHECKSUM_FILE || entry.path() == output {
            continue;
        }
        files.insert(relative, std::fs::read(entry.path())?);
    }
    if files.len() > MAX_FILE_COUNT || !files.contains_key(PLUGIN_MANIFEST_FILE) {
        return Err(PluginPackageError::Unsafe(
            "package file count or manifest is invalid".to_owned(),
        ));
    }
    let checksums = PackageChecksums {
        schema_version: "1".to_owned(),
        files: files
            .iter()
            .filter(|(path, _)| !path.starts_with("signatures/"))
            .map(|(path, bytes)| (path.clone(), Sha256Digest::of_bytes(bytes)))
            .collect(),
    };
    checksums.validate()?;
    files.insert(
        PLUGIN_CHECKSUM_FILE.to_owned(),
        serde_json::to_vec_pretty(&checksums)?,
    );

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    for (path, bytes) in &files {
        let file_options = if path.starts_with("bin/") {
            options.unix_permissions(0o755)
        } else {
            options
        };
        writer.start_file(path, file_options)?;
        writer.write_all(bytes)?;
    }
    let bytes = writer.finish()?.into_inner();
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_PACKAGE_BYTES {
        return Err(PluginPackageError::Unsafe(
            "package exceeds the maximum size".to_owned(),
        ));
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = output.with_extension("annotplugin.partial");
    std::fs::write(&temporary, &bytes)?;
    std::fs::rename(temporary, output)?;
    Ok(Sha256Digest::of_bytes(&bytes))
}

pub fn verify_package(path: &Path) -> Result<VerifiedPluginPackage, PluginPackageError> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_PACKAGE_BYTES {
        return Err(PluginPackageError::Unsafe(
            "package must be a bounded regular file".to_owned(),
        ));
    }
    let package_bytes = std::fs::read(path)?;
    let package_digest = Sha256Digest::of_bytes(&package_bytes);
    let mut archive = ZipArchive::new(Cursor::new(&package_bytes))?;
    if archive.is_empty() || archive.len() > MAX_FILE_COUNT {
        return Err(PluginPackageError::Unsafe(
            "archive file count is invalid".to_owned(),
        ));
    }
    let mut files = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = normalize_archive_name(entry.name())?;
        if !names.insert(name.clone()) {
            return Err(PluginPackageError::Unsafe(format!(
                "duplicate archive path {name}"
            )));
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170_000 == 0o120_000)
        {
            return Err(PluginPackageError::Unsafe(
                "archive links are not allowed".to_owned(),
            ));
        }
        total = total.saturating_add(entry.size());
        if total > MAX_PACKAGE_BYTES {
            return Err(PluginPackageError::Unsafe(
                "expanded archive exceeds the maximum size".to_owned(),
            ));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or(0));
        entry.read_to_end(&mut bytes)?;
        files.insert(name, bytes);
    }
    let manifest_bytes = files
        .get(PLUGIN_MANIFEST_FILE)
        .ok_or_else(|| PluginPackageError::Unsafe("manifest is missing".to_owned()))?;
    let manifest_source = std::str::from_utf8(manifest_bytes)
        .map_err(|_| PluginPackageError::Unsafe("manifest is not UTF-8".to_owned()))?;
    let manifest = PluginManifest::from_toml(manifest_source)?;
    validate_current_target(&manifest)?;
    let checksum_bytes = files
        .get(PLUGIN_CHECKSUM_FILE)
        .ok_or_else(|| PluginPackageError::Unsafe("checksums are missing".to_owned()))?;
    let checksums: PackageChecksums = serde_json::from_slice(checksum_bytes)?;
    checksums.validate()?;
    let expected_paths = files
        .keys()
        .filter(|path| *path != PLUGIN_CHECKSUM_FILE && !path.starts_with("signatures/"))
        .cloned()
        .collect::<BTreeSet<_>>();
    let checksum_paths = checksums.files.keys().cloned().collect::<BTreeSet<_>>();
    if expected_paths != checksum_paths {
        return Err(PluginPackageError::Checksum(
            "checksum file list does not exactly match package files".to_owned(),
        ));
    }
    for (name, expected) in &checksums.files {
        let bytes = files
            .get(name)
            .ok_or_else(|| PluginPackageError::Checksum(name.clone()))?;
        if Sha256Digest::of_bytes(bytes) != *expected {
            return Err(PluginPackageError::Checksum(name.clone()));
        }
    }
    let executable = manifest
        .runtime
        .entrypoint
        .replace("{target}", &current_target());
    if files.get(&executable).is_none_or(Vec::is_empty) {
        return Err(PluginPackageError::Incompatible(format!(
            "target executable {executable} is missing or empty"
        )));
    }
    let signature = if files.keys().any(|path| path.starts_with("signatures/")) {
        PackageSignatureState::PresentUnverified
    } else {
        PackageSignatureState::Unsigned
    };
    Ok(VerifiedPluginPackage {
        source: path.to_owned(),
        manifest,
        package_digest,
        signature,
        files,
    })
}

fn validate_current_target(manifest: &PluginManifest) -> Result<(), PluginPackageError> {
    let target = current_target();
    if !manifest
        .compatibility
        .targets
        .iter()
        .any(|item| item == &target)
    {
        return Err(PluginPackageError::Incompatible(format!(
            "plugin does not support {target}"
        )));
    }
    Ok(())
}

#[must_use]
pub fn current_target() -> String {
    let os = match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        _ => "linux",
    };
    let architecture = std::env::consts::ARCH;
    format!("{os}-{architecture}")
}

fn normalize_archive_name(value: &str) -> Result<String, PluginPackageError> {
    if value.contains('\\') {
        return Err(PluginPackageError::Unsafe(
            "archive paths must use forward slashes".to_owned(),
        ));
    }
    normalize_relative(Path::new(value))
}

fn normalize_relative(path: &Path) -> Result<String, PluginPackageError> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(PluginPackageError::Unsafe(format!(
            "unsafe package path {}",
            path.display()
        )));
    }
    let value = path
        .to_str()
        .ok_or_else(|| PluginPackageError::Unsafe("package path is not UTF-8".to_owned()))?
        .replace(std::path::MAIN_SEPARATOR, "/");
    if value.is_empty() {
        return Err(PluginPackageError::Unsafe(
            "package path cannot be empty".to_owned(),
        ));
    }
    Ok(value)
}

fn ensure_descendant(root: &Path, path: &Path) -> Result<(), PluginPackageError> {
    if !path.starts_with(root) || path == root {
        return Err(PluginPackageError::Unsafe(
            "package path escapes installation root".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_package_round_trip_and_tamper_detection() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source");
        let binary = source
            .join("bin")
            .join(current_target())
            .join("annotagent-plugin-dummy-detector");
        std::fs::create_dir_all(binary.parent().expect("parent")).expect("dirs");
        let manifest = include_str!("../../../plugins/dummy-detector/annotagent-plugin.toml");
        std::fs::write(source.join(PLUGIN_MANIFEST_FILE), manifest).expect("manifest");
        std::fs::write(&binary, b"fixture-binary").expect("binary");
        let first = temp.path().join("first.annotplugin");
        let second = temp.path().join("second.annotplugin");
        let digest = pack_directory(&source, &first).expect("pack");
        assert_eq!(digest, pack_directory(&source, &second).expect("pack"));
        let verified = verify_package(&first).expect("verify");
        assert_eq!(verified.package_digest, digest);
        let install = temp.path().join("install");
        verified.extract_to(&install).expect("extract");
        assert!(
            verified
                .executable_path(&install)
                .expect("executable")
                .is_file()
        );

        let mut bytes = std::fs::read(&first).expect("package");
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        let tampered = temp.path().join("tampered.annotplugin");
        std::fs::write(&tampered, bytes).expect("tampered");
        assert!(verify_package(&tampered).is_err());
    }

    #[test]
    fn archive_path_validation_rejects_escape_and_mixed_separators() {
        assert!(normalize_archive_name("../escape").is_err());
        assert!(normalize_archive_name("/absolute").is_err());
        assert!(normalize_archive_name("bin\\escape").is_err());
        assert!(normalize_archive_name("bin/target/plugin").is_ok());
    }
}
