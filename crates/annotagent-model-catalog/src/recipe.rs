use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};

use annotagent_model_bundle::{
    MODEL_BUNDLE_MANIFEST_FILE, ModelBundleId, ModelBundleManifest, Sha256Digest,
    pack_model_bundle, verify_model_bundle,
};
use annotagent_plugin_api::PluginModelManifest;
use futures::StreamExt as _;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::io::AsyncWriteExt as _;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{
    MAX_CATALOG_BUNDLE_BYTES, ModelCatalogEntry, ModelCatalogError, ModelLicenseSummary,
    PlatformRequirement, PublisherIdentity, validate_https_public_url, validate_public_ip,
};

const MODEL_RECIPE_SCHEMA_VERSION: &str = "1";
const PLUGIN_CONTRACT_TOKEN: &str = "{{PLUGIN_MODEL_CONTRACT_SHA256}}";
const MAX_RECIPE_REDIRECTS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSupplyRecipe {
    pub schema_version: String,
    pub id: ModelBundleId,
    pub version: Version,
    pub manifest_template: String,
    pub plugin_model_contract: String,
    pub upstream: ModelRecipeUpstream,
    pub downloads: Vec<ModelRecipeDownload>,
    #[serde(default)]
    pub static_files: Vec<ModelRecipeStaticFile>,
    pub catalog: ModelRecipeCatalog,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRecipeUpstream {
    pub project: String,
    pub model: String,
    pub revision: String,
    pub repository_url: Url,
    pub license_url: Url,
    pub license_sha256: Sha256Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRecipeDownload {
    pub name: String,
    pub url: Url,
    pub sha256: Sha256Digest,
    pub size_bytes: u64,
    pub bundle_path: String,
    #[serde(default)]
    pub allowed_redirect_hosts: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRecipeStaticFile {
    pub source: String,
    pub sha256: Sha256Digest,
    pub size_bytes: u64,
    pub bundle_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRecipeCatalog {
    pub bundle_url: Url,
    pub publisher_id: String,
    pub publisher_name: String,
    pub publisher_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRecipeAuditReport {
    pub recipe_path: PathBuf,
    pub bundle_id: ModelBundleId,
    pub bundle_version: Version,
    pub plugin_contract_sha256: Sha256Digest,
    pub download_count: usize,
    pub download_bytes: u64,
    pub static_file_count: usize,
    pub cached_downloads: usize,
    pub publishable: bool,
    pub fixture: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRecipeFetchReport {
    pub bundle_id: ModelBundleId,
    pub bundle_version: Version,
    pub downloaded: usize,
    pub reused: usize,
    pub bytes_verified: u64,
    pub cache_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRecipeBuildReport {
    pub output: PathBuf,
    pub bundle_sha256: Sha256Digest,
    pub bundle_size_bytes: u64,
    pub catalog_entry: ModelCatalogEntry,
}

struct LoadedRecipe {
    path: PathBuf,
    root: PathBuf,
    value: ModelSupplyRecipe,
    manifest: ModelBundleManifest,
    plugin_contract_sha256: Sha256Digest,
}

pub fn audit_model_recipe(path: &Path) -> Result<ModelRecipeAuditReport, ModelCatalogError> {
    let loaded = load_recipe(path)?;
    let cache_root = recipe_cache_root(&loaded);
    let cached_downloads = loaded
        .value
        .downloads
        .iter()
        .filter(|download| {
            verify_file(
                &cache_root.join(download.sha256.as_str()),
                download.size_bytes,
                &download.sha256,
            )
            .is_ok()
        })
        .count();
    Ok(ModelRecipeAuditReport {
        recipe_path: loaded.path,
        bundle_id: loaded.value.id,
        bundle_version: loaded.value.version,
        plugin_contract_sha256: loaded.plugin_contract_sha256,
        download_count: loaded.value.downloads.len(),
        download_bytes: loaded
            .value
            .downloads
            .iter()
            .map(|download| download.size_bytes)
            .sum(),
        static_file_count: loaded.value.static_files.len(),
        cached_downloads,
        publishable: loaded.manifest.publishable,
        fixture: loaded.manifest.fixture,
    })
}

pub async fn fetch_model_recipe(
    path: &Path,
    cancellation: &CancellationToken,
) -> Result<ModelRecipeFetchReport, ModelCatalogError> {
    let loaded = load_recipe(path)?;
    let cache_root = recipe_cache_root(&loaded);
    tokio::fs::create_dir_all(&cache_root).await?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(30 * 60))
        .user_agent("AnnotAgent-ModelRecipe/1")
        .build()?;
    let mut downloaded = 0;
    let mut reused = 0;
    let mut bytes_verified = 0_u64;
    for download in &loaded.value.downloads {
        let destination = cache_root.join(download.sha256.as_str());
        if verify_file(&destination, download.size_bytes, &download.sha256).is_ok() {
            reused += 1;
        } else {
            if destination.exists() {
                return recipe_error(format!(
                    "cached asset {} exists but does not match its immutable declaration",
                    destination.display()
                ));
            }
            fetch_download(&client, download, &destination, cancellation).await?;
            downloaded += 1;
        }
        bytes_verified = bytes_verified.saturating_add(download.size_bytes);
    }
    Ok(ModelRecipeFetchReport {
        bundle_id: loaded.value.id,
        bundle_version: loaded.value.version,
        downloaded,
        reused,
        bytes_verified,
        cache_root,
    })
}

pub fn verify_model_recipe(path: &Path) -> Result<ModelRecipeAuditReport, ModelCatalogError> {
    let report = audit_model_recipe(path)?;
    if report.cached_downloads != report.download_count {
        return recipe_error(format!(
            "{} of {} immutable downloads are present; run recipe fetch first",
            report.cached_downloads, report.download_count
        ));
    }
    Ok(report)
}

pub fn build_model_recipe(
    path: &Path,
    output: &Path,
) -> Result<ModelRecipeBuildReport, ModelCatalogError> {
    verify_model_recipe(path)?;
    let loaded = load_recipe(path)?;
    let cache_root = recipe_cache_root(&loaded);
    let staging = cache_root.join(format!("build-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&staging)?;
    let temporary_output =
        output.with_extension(format!("annotmodel.building-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        for download in &loaded.value.downloads {
            copy_payload(
                &cache_root.join(download.sha256.as_str()),
                &staging,
                &download.bundle_path,
            )?;
        }
        for file in &loaded.value.static_files {
            copy_payload(
                &resolve_recipe_file(&loaded.root, &file.source)?,
                &staging,
                &file.bundle_path,
            )?;
        }
        std::fs::write(
            staging.join(MODEL_BUNDLE_MANIFEST_FILE),
            loaded.manifest.to_toml().map_err(|error| {
                ModelCatalogError::Recipe(format!("manifest serialization failed: {error}"))
            })?,
        )?;
        let digest = pack_model_bundle(&staging, &temporary_output).map_err(|error| {
            ModelCatalogError::Recipe(format!("Bundle packing failed: {error}"))
        })?;
        let verified = verify_model_bundle(&temporary_output).map_err(|error| {
            ModelCatalogError::Recipe(format!("built Bundle verification failed: {error}"))
        })?;
        if verified.manifest.id != loaded.value.id
            || verified.manifest.version != loaded.value.version
            || verified.manifest.fixture
            || !verified.manifest.publishable
        {
            return recipe_error("built Bundle identity or publication policy changed".to_owned());
        }
        let size = std::fs::metadata(&temporary_output)?.len();
        let installed_size = verified
            .files
            .values()
            .try_fold(0_u64, |total, file| total.checked_add(file.size_bytes))
            .ok_or_else(|| ModelCatalogError::Recipe("installed size overflow".to_owned()))?;
        if size > MAX_CATALOG_BUNDLE_BYTES {
            return recipe_error("built Bundle exceeds the Catalog size limit".to_owned());
        }
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if output.is_file() {
            let current = file_sha256(output)?;
            if current != digest || std::fs::metadata(output)?.len() != size {
                return recipe_error(format!(
                    "refusing to overwrite different existing Bundle {}",
                    output.display()
                ));
            }
            std::fs::remove_file(&temporary_output)?;
        } else {
            std::fs::rename(&temporary_output, output)?;
        }
        let catalog_entry = catalog_entry(&loaded, digest.clone(), size, installed_size)?;
        Ok(ModelRecipeBuildReport {
            output: output.to_path_buf(),
            bundle_sha256: digest,
            bundle_size_bytes: size,
            catalog_entry,
        })
    })();
    let _ = std::fs::remove_dir_all(&staging);
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary_output);
    }
    result
}

fn load_recipe(path: &Path) -> Result<LoadedRecipe, ModelCatalogError> {
    let path = if path.is_dir() {
        path.join("recipe.toml")
    } else {
        path.to_path_buf()
    };
    let path = path.canonicalize().map_err(|error| {
        ModelCatalogError::Recipe(format!("cannot resolve {}: {error}", path.display()))
    })?;
    let root = path
        .parent()
        .ok_or_else(|| ModelCatalogError::Recipe("recipe has no parent directory".to_owned()))?
        .to_path_buf();
    let value: ModelSupplyRecipe =
        toml::from_str(&std::fs::read_to_string(&path)?).map_err(|error| {
            ModelCatalogError::Recipe(format!("{} cannot be parsed: {error}", path.display()))
        })?;
    validate_recipe_declarations(&root, &value)?;
    let plugin_contract_path = resolve_recipe_file(&root, &value.plugin_model_contract)?;
    let plugin_contract: PluginModelManifest =
        serde_json::from_slice(&std::fs::read(&plugin_contract_path)?).map_err(|error| {
            ModelCatalogError::Recipe(format!("Plugin model Contract cannot be parsed: {error}"))
        })?;
    plugin_contract
        .validate()
        .map_err(|error| ModelCatalogError::Recipe(error.to_string()))?;
    let plugin_contract_sha256 = Sha256Digest::of_bytes(
        &serde_json::to_vec(&plugin_contract)
            .map_err(|error| ModelCatalogError::Recipe(error.to_string()))?,
    );
    let template = std::fs::read_to_string(resolve_recipe_file(&root, &value.manifest_template)?)?;
    if template.matches(PLUGIN_CONTRACT_TOKEN).count() != 1 {
        return recipe_error("manifest template must contain one Plugin Contract token".to_owned());
    }
    let manifest = ModelBundleManifest::from_toml(
        &template.replace(PLUGIN_CONTRACT_TOKEN, plugin_contract_sha256.as_str()),
    )
    .map_err(|error| ModelCatalogError::Recipe(format!("manifest template is invalid: {error}")))?;
    validate_recipe_manifest(&value, &manifest, &plugin_contract, &plugin_contract_sha256)?;
    validate_payload_set(&root, &value, &manifest)?;
    Ok(LoadedRecipe {
        path,
        root,
        value,
        manifest,
        plugin_contract_sha256,
    })
}

fn validate_recipe_declarations(
    root: &Path,
    recipe: &ModelSupplyRecipe,
) -> Result<(), ModelCatalogError> {
    if recipe.schema_version != MODEL_RECIPE_SCHEMA_VERSION
        || recipe.downloads.is_empty()
        || recipe.id.as_str().is_empty()
    {
        return recipe_error("Recipe must use schema 1 and declare immutable downloads".to_owned());
    }
    validate_https_public_url(&recipe.catalog.bundle_url)?;
    validate_https_public_url(&recipe.upstream.repository_url)?;
    validate_https_public_url(&recipe.upstream.license_url)?;
    validate_name(&recipe.upstream.project)?;
    validate_name(&recipe.upstream.model)?;
    validate_name(&recipe.upstream.revision)?;
    let mut bundle_paths = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut total = 0_u64;
    for download in &recipe.downloads {
        validate_https_public_url(&download.url)?;
        validate_name(&download.name)?;
        validate_bundle_path(&download.bundle_path)?;
        if download.size_bytes == 0
            || !names.insert(download.name.as_str())
            || !bundle_paths.insert(download.bundle_path.as_str())
        {
            return recipe_error(
                "download names, sizes and Bundle paths must be unique".to_owned(),
            );
        }
        total = total.saturating_add(download.size_bytes);
        for host in &download.allowed_redirect_hosts {
            validate_host_name(host)?;
        }
    }
    for file in &recipe.static_files {
        validate_relative_path(&file.source)?;
        validate_bundle_path(&file.bundle_path)?;
        if file.size_bytes == 0 || !bundle_paths.insert(file.bundle_path.as_str()) {
            return recipe_error("static file sizes and Bundle paths must be unique".to_owned());
        }
        let source = resolve_recipe_file(root, &file.source)?;
        verify_file(&source, file.size_bytes, &file.sha256)?;
        total = total.saturating_add(file.size_bytes);
    }
    if total > MAX_CATALOG_BUNDLE_BYTES {
        return recipe_error("Recipe payload exceeds the Bundle size limit".to_owned());
    }
    validate_relative_path(&recipe.manifest_template)?;
    validate_relative_path(&recipe.plugin_model_contract)?;
    validate_name(&recipe.catalog.publisher_id)?;
    validate_name(&recipe.catalog.publisher_name)?;
    Ok(())
}

fn validate_recipe_manifest(
    recipe: &ModelSupplyRecipe,
    manifest: &ModelBundleManifest,
    plugin_contract: &PluginModelManifest,
    plugin_contract_sha256: &Sha256Digest,
) -> Result<(), ModelCatalogError> {
    if manifest.id != recipe.id || manifest.version != recipe.version {
        return recipe_error("Recipe and Bundle Manifest identities differ".to_owned());
    }
    if manifest.fixture || !manifest.publishable {
        return recipe_error("real-model Recipe must be non-Fixture and publishable".to_owned());
    }
    if manifest.compatible_plugins.len() != 1 {
        return recipe_error(
            "first delivery Recipe must bind one exact Plugin Contract".to_owned(),
        );
    }
    let requirement = &manifest.compatible_plugins[0];
    if requirement.model_id != plugin_contract.id
        || requirement.contract_hash != *plugin_contract_sha256
        || manifest.capabilities.iter().any(|capability| {
            !plugin_contract
                .capabilities
                .iter()
                .any(|value| value == capability)
        })
    {
        return recipe_error(
            "Bundle compatibility does not match the canonical Plugin model Contract".to_owned(),
        );
    }
    Ok(())
}

fn validate_payload_set(
    root: &Path,
    recipe: &ModelSupplyRecipe,
    manifest: &ModelBundleManifest,
) -> Result<(), ModelCatalogError> {
    let declared = recipe
        .downloads
        .iter()
        .map(|item| item.bundle_path.clone())
        .chain(
            recipe
                .static_files
                .iter()
                .map(|item| item.bundle_path.clone()),
        )
        .collect::<BTreeSet<_>>();
    let mut expected = BTreeSet::new();
    for file in &manifest.files {
        expected.insert(file.path.clone());
        expected.extend(file.external_data_files.iter().cloned());
        let source = recipe
            .downloads
            .iter()
            .find(|download| download.bundle_path == file.path)
            .ok_or_else(|| {
                ModelCatalogError::Recipe(format!(
                    "model file {} is not an immutable download",
                    file.path
                ))
            })?;
        if source.sha256 != file.sha256 || source.size_bytes != file.size_bytes {
            return recipe_error(format!(
                "model file {} does not match its Recipe declaration",
                file.path
            ));
        }
    }
    expected.extend(manifest.contracts.iter().map(|item| item.path.clone()));
    expected.extend(manifest.transforms.iter().map(|item| item.path.clone()));
    expected.insert(manifest.license.license_file.clone());
    expected.extend(manifest.license.source_notice.iter().cloned());
    expected.extend(manifest.test_suite.input_artifacts.iter().cloned());
    expected.insert(manifest.test_suite.expected_summary.clone());
    expected.insert(manifest.test_suite.tolerances.clone());
    if declared != expected {
        return recipe_error(format!(
            "Recipe payload differs from Manifest; missing={:?}, unknown={:?}",
            expected.difference(&declared).collect::<Vec<_>>(),
            declared.difference(&expected).collect::<Vec<_>>()
        ));
    }
    let license = recipe
        .downloads
        .iter()
        .find(|item| item.bundle_path == manifest.license.license_file)
        .map(|item| &item.sha256)
        .or_else(|| {
            recipe
                .static_files
                .iter()
                .find(|item| item.bundle_path == manifest.license.license_file)
                .map(|item| &item.sha256)
        });
    if license != Some(&manifest.license.license_digest) {
        return recipe_error(
            "Manifest license digest differs from the exact license bytes".to_owned(),
        );
    }
    if recipe.upstream.license_sha256 != manifest.license.license_digest {
        return recipe_error(
            "Recipe upstream license digest differs from the Bundle Manifest".to_owned(),
        );
    }
    for file in &recipe.static_files {
        verify_file(
            &resolve_recipe_file(root, &file.source)?,
            file.size_bytes,
            &file.sha256,
        )?;
    }
    Ok(())
}

async fn fetch_download(
    client: &reqwest::Client,
    download: &ModelRecipeDownload,
    destination: &Path,
    cancellation: &CancellationToken,
) -> Result<(), ModelCatalogError> {
    let mut url = download.url.clone();
    let mut redirect_count = 0_usize;
    let response = loop {
        validate_network_destination(&url).await?;
        let response = tokio::select! {
            () = cancellation.cancelled() => return Err(ModelCatalogError::Cancelled),
            response = client.get(url.clone()).send() => response?,
        };
        if !response.status().is_redirection() {
            break response.error_for_status()?;
        }
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ModelCatalogError::Recipe("redirect is missing Location".to_owned()))?;
        let next = url
            .join(location)
            .map_err(|error| ModelCatalogError::Recipe(format!("invalid redirect: {error}")))?;
        let host = next
            .host_str()
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| ModelCatalogError::Recipe("redirect host is missing".to_owned()))?;
        if !download.allowed_redirect_hosts.contains(&host) {
            return recipe_error(format!("redirect host {host} is not explicitly allowed"));
        }
        redirect_count += 1;
        if redirect_count > MAX_RECIPE_REDIRECTS {
            return recipe_error("too many model asset redirects".to_owned());
        }
        url = next;
    };
    if response
        .content_length()
        .is_some_and(|size| size != download.size_bytes)
    {
        return Err(ModelCatalogError::DownloadSize);
    }
    let temporary = destination.with_extension(format!("partial-{}", uuid::Uuid::new_v4()));
    let result = async {
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .await?;
        let mut stream = response.bytes_stream();
        let mut digest = Sha256::new();
        let mut size = 0_u64;
        while let Some(chunk) = tokio::select! {
            () = cancellation.cancelled() => return Err(ModelCatalogError::Cancelled),
            value = stream.next() => value,
        } {
            let chunk = chunk?;
            size = size.saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
            if size > download.size_bytes {
                return Err(ModelCatalogError::DownloadSize);
            }
            digest.update(&chunk);
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        file.sync_all().await?;
        let actual = Sha256Digest::parse(format!("{:x}", digest.finalize()))
            .map_err(|error| ModelCatalogError::Recipe(error.to_string()))?;
        if size != download.size_bytes || actual != download.sha256 {
            return if size == download.size_bytes {
                Err(ModelCatalogError::DownloadChecksum)
            } else {
                Err(ModelCatalogError::DownloadSize)
            };
        }
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temporary).await;
    }
    result?;
    tokio::fs::rename(&temporary, destination).await?;
    Ok(())
}

async fn validate_network_destination(url: &Url) -> Result<(), ModelCatalogError> {
    validate_https_public_url(url)?;
    let host = url
        .host_str()
        .ok_or_else(|| ModelCatalogError::UnsafeUrl("URL host is missing".to_owned()))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let mut found = false;
    for address in tokio::net::lookup_host((host, port)).await? {
        found = true;
        validate_public_ip(address.ip())?;
    }
    if !found {
        return Err(ModelCatalogError::UnsafeUrl(
            "host did not resolve to an address".to_owned(),
        ));
    }
    Ok(())
}

fn catalog_entry(
    loaded: &LoadedRecipe,
    bundle_sha256: Sha256Digest,
    bundle_size_bytes: u64,
    installed_size_bytes: u64,
) -> Result<ModelCatalogEntry, ModelCatalogError> {
    let manifest = &loaded.manifest;
    let entry = ModelCatalogEntry {
        bundle_id: manifest.id.clone(),
        bundle_version: manifest.version.clone(),
        display_name: manifest.display_name.clone(),
        description: manifest
            .description
            .clone()
            .unwrap_or_else(|| manifest.model_family.clone()),
        model_family: Some(manifest.model_family.clone()),
        capabilities: manifest.capabilities.clone(),
        compatible_plugins: manifest.compatible_plugins.clone(),
        platform_requirements: manifest
            .runtime
            .platforms
            .iter()
            .map(|target| PlatformRequirement {
                target: target.clone(),
                execution_providers: manifest.runtime.execution_providers.clone(),
                minimum_memory_mb: manifest.runtime.minimum_memory_mb,
                minimum_disk_bytes: installed_size_bytes,
            })
            .collect(),
        bundle_url: loaded.value.catalog.bundle_url.clone(),
        bundle_sha256,
        bundle_size_bytes,
        installed_size_bytes: Some(installed_size_bytes),
        license_summary: ModelLicenseSummary {
            name: manifest.license.name.clone(),
            license_url: manifest.license.license_url.clone(),
            license_digest: manifest.license.license_digest.clone(),
            redistribution: manifest.license.redistribution,
            commercial_use: manifest.license.commercial_use,
            requires_acceptance: manifest.license.requires_acceptance,
        },
        publisher: PublisherIdentity {
            id: loaded.value.catalog.publisher_id.clone(),
            display_name: loaded.value.catalog.publisher_name.clone(),
            verified: loaded.value.catalog.publisher_verified,
        },
        fixture: manifest.fixture,
        publishable: manifest.publishable,
    };
    entry.validate()?;
    Ok(entry)
}

fn recipe_cache_root(loaded: &LoadedRecipe) -> PathBuf {
    loaded.root.join(".cache")
}

fn resolve_recipe_file(root: &Path, relative: &str) -> Result<PathBuf, ModelCatalogError> {
    validate_relative_path(relative)?;
    let path = root.join(relative).canonicalize().map_err(|error| {
        ModelCatalogError::Recipe(format!("cannot resolve Recipe file {relative}: {error}"))
    })?;
    if !path.starts_with(root) || !path.is_file() {
        return recipe_error(format!(
            "Recipe file {relative} escapes its root or is not a file"
        ));
    }
    Ok(path)
}

fn validate_relative_path(value: &str) -> Result<(), ModelCatalogError> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || value.contains([':', '\\'])
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return recipe_error(format!("unsafe Recipe path {value}"));
    }
    Ok(())
}

fn validate_bundle_path(value: &str) -> Result<(), ModelCatalogError> {
    validate_relative_path(value)?;
    if value == MODEL_BUNDLE_MANIFEST_FILE
        || !["files/", "contracts/", "transforms/", "licenses/", "tests/"]
            .iter()
            .any(|prefix| value.starts_with(prefix))
    {
        return recipe_error(format!("unsupported Bundle payload path {value}"));
    }
    Ok(())
}

fn validate_name(value: &str) -> Result<(), ModelCatalogError> {
    if value.trim().is_empty() || value.len() > 240 || value.contains(['\r', '\n']) {
        return recipe_error("Recipe name is empty, multiline or too long".to_owned());
    }
    Ok(())
}

fn validate_host_name(value: &str) -> Result<(), ModelCatalogError> {
    if value.is_empty() || value != value.to_ascii_lowercase() || value.contains(['/', '@', ':']) {
        return recipe_error(format!("invalid redirect host {value}"));
    }
    Ok(())
}

fn copy_payload(source: &Path, staging: &Path, relative: &str) -> Result<(), ModelCatalogError> {
    validate_bundle_path(relative)?;
    let destination = staging.join(relative);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, destination)?;
    Ok(())
}

fn verify_file(
    path: &Path,
    expected_size: u64,
    expected_sha256: &Sha256Digest,
) -> Result<(), ModelCatalogError> {
    if std::fs::metadata(path)?.len() != expected_size || file_sha256(path)? != *expected_sha256 {
        return recipe_error(format!(
            "{} failed size or SHA-256 verification",
            path.display()
        ));
    }
    Ok(())
}

fn file_sha256(path: &Path) -> Result<Sha256Digest, ModelCatalogError> {
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest)
        .map_err(|error| ModelCatalogError::Recipe(format!("hash failed: {error}")))?;
    Sha256Digest::parse(format!("{:x}", digest.finalize()))
        .map_err(|error| ModelCatalogError::Recipe(error.to_string()))
}

fn recipe_error<T>(message: String) -> Result<T, ModelCatalogError> {
    Err(ModelCatalogError::Recipe(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_redirect_hosts_fail_closed() {
        for path in ["", "/tmp/model", "../model", "files/../model", "C:/model"] {
            assert!(validate_relative_path(path).is_err(), "{path}");
        }
        assert!(validate_bundle_path("model.onnx").is_err());
        assert!(validate_bundle_path("annotagent-model.toml").is_err());
        assert!(validate_bundle_path("files/model.onnx").is_ok());
        assert!(validate_host_name("cdn.example.com").is_ok());
        assert!(validate_host_name("CDN.example.com").is_err());
        assert!(validate_host_name("user@cdn.example.com").is_err());
    }

    #[test]
    fn recipe_schema_rejects_commands_and_unknown_fields() {
        let source = r#"
schema_version = "1"
id = "org.annotagent.models.example"
version = "1.0.0"
manifest_template = "bundle.toml"
plugin_model_contract = "plugin.json"
command = "python export.py"
downloads = []

[catalog]
bundle_url = "https://models.example/model.annotmodel"
publisher_id = "example"
publisher_name = "Example"
publisher_verified = false
"#;
        assert!(toml::from_str::<ModelSupplyRecipe>(source).is_err());
    }

    #[test]
    fn repository_efficientsam_recipe_is_real_publishable_and_audited() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("repository root");
        let report = audit_model_recipe(&repository.join("model-recipes/efficientsam-ti"))
            .expect("audited Recipe");
        assert_eq!(
            report.bundle_id.as_str(),
            "org.annotagent.models.efficientsam-ti-onnx"
        );
        assert!(report.publishable);
        assert!(!report.fixture);
        assert_eq!(report.download_count, 3);
        assert_eq!(report.download_bytes, 41_814_211);
        assert_eq!(
            report.plugin_contract_sha256.as_str(),
            "ad3f23abcadb04561dcced33bae9cbfccbce4c13910a715fc964f1281c8f56ee"
        );
    }
}
