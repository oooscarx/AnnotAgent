use std::path::Path;

use annotagent_model_bundle::{
    ModelBundleId, ModelInstanceId, SmokeTestCheck, SmokeTestResult, SmokeTestStatus,
    pack_model_bundle, verify_model_bundle,
};
use annotagent_model_catalog::{
    BindModelInstanceRequest, LicenseAcceptanceActor, ModelBundleInstallSource,
    ModelBundleRegistry, ModelCatalog, ModelCatalogClient, ModelCatalogEntry,
    ModelLicenseAcceptance, audit_model_recipe, build_model_recipe, evaluate_bundle_smoke_response,
    fetch_model_recipe, prepare_bundle_smoke_test, verify_model_recipe,
};
use annotagent_plugin_registry::{PluginRegistry, PluginRegistryError, run_model_instance_smoke};
use anyhow::{Context, Result, bail};
use chrono::Utc;
use semver::Version;
use tokio_util::sync::CancellationToken;

use crate::{ModelBundleCommand, ModelCatalogCommand, ModelRecipeCommand, ModelsCommand};

pub async fn run(workspace: &Path, command: ModelsCommand) -> Result<()> {
    match command {
        ModelsCommand::Bundle { command } => bundle(command),
        ModelsCommand::Catalog { command: None } => list_catalogs(workspace),
        ModelsCommand::Catalog {
            command: Some(command),
        } => catalog(workspace, command),
        ModelsCommand::Search { query } => search(workspace, &query),
        ModelsCommand::Show { bundle_id } => show(workspace, &bundle_id),
        ModelsCommand::Install { bundle, accept } => install(workspace, &bundle, accept).await,
        ModelsCommand::Import { package, accept } => import(workspace, &package, accept),
        ModelsCommand::List => list(workspace),
        ModelsCommand::Test { model_instance_id } => {
            test_instance(workspace, parse_instance_id(&model_instance_id)?).await
        }
        ModelsCommand::Disable { model_instance_id } => {
            set_enabled(workspace, parse_instance_id(&model_instance_id)?, false)
        }
        ModelsCommand::Enable { model_instance_id } => {
            set_enabled(workspace, parse_instance_id(&model_instance_id)?, true)
        }
        ModelsCommand::Remove { bundle } => remove(workspace, &bundle),
        ModelsCommand::References { bundle } => references(workspace, &bundle),
        ModelsCommand::Doctor { model_instance_id } => {
            doctor(workspace, parse_instance_id(&model_instance_id)?)
        }
        ModelsCommand::Gc => garbage_collect(workspace),
        ModelsCommand::Recipe { command } => recipe(command).await,
    }
}

fn bundle(command: ModelBundleCommand) -> Result<()> {
    match command {
        ModelBundleCommand::Pack { directory, output } => {
            let digest = pack_model_bundle(&directory, &output)?;
            println!("packed {}", output.display());
            println!("bundle sha256: {digest}");
        }
        ModelBundleCommand::Inspect { package } => {
            let verified = verify_model_bundle(&package)?;
            println!("{}", verified.manifest.to_toml()?);
            println!("bundle sha256: {}", verified.bundle_digest);
            println!("signature: {:?}", verified.signature);
            println!("files: {}", verified.files.len());
        }
        ModelBundleCommand::Verify { package } => {
            let verified = verify_model_bundle(&package)?;
            println!(
                "verified {}@{} ({})",
                verified.manifest.id, verified.manifest.version, verified.bundle_digest
            );
            println!("signature: {:?}", verified.signature);
        }
    }
    Ok(())
}

fn catalog(workspace: &Path, command: ModelCatalogCommand) -> Result<()> {
    match command {
        ModelCatalogCommand::List => list_catalogs(workspace),
        ModelCatalogCommand::AddLocal { directory } => {
            let catalog = registry(workspace)?.add_trusted_local_catalog(&directory)?;
            println!(
                "added trusted local Catalog {} ({} entries) from {}",
                catalog.catalog_id,
                catalog.entries.len(),
                directory.display()
            );
            Ok(())
        }
        ModelCatalogCommand::Refresh => {
            let catalogs = registry(workspace)?.refresh_trusted_local_catalogs()?;
            println!("refreshed {} trusted local Catalogs", catalogs.len());
            for catalog in catalogs {
                println!("{} · {} bundles", catalog.catalog_id, catalog.entries.len());
            }
            Ok(())
        }
        ModelCatalogCommand::Build {
            directory,
            output,
            catalog_id,
        } => {
            let mut entries = Vec::<ModelCatalogEntry>::new();
            for entry in std::fs::read_dir(&directory)
                .with_context(|| format!("cannot read Catalog source {}", directory.display()))?
            {
                let path = entry?.path();
                if path
                    .extension()
                    .is_some_and(|extension| extension == "json")
                    && path.file_name().is_some_and(|name| name != "catalog.json")
                {
                    entries
                        .push(serde_json::from_slice(&std::fs::read(&path)?).with_context(
                            || format!("invalid Catalog entry {}", path.display()),
                        )?);
                }
            }
            if entries.is_empty() {
                bail!("Catalog source must contain one or more ModelCatalogEntry JSON files");
            }
            entries.sort_by(|left, right| {
                (&left.bundle_id, &left.bundle_version)
                    .cmp(&(&right.bundle_id, &right.bundle_version))
            });
            let catalog = ModelCatalog {
                schema_version: annotagent_model_catalog::MODEL_CATALOG_SCHEMA_VERSION.to_owned(),
                catalog_id: catalog_id.unwrap_or_else(|| {
                    format!(
                        "local.{}",
                        directory
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("models")
                    )
                }),
                generated_at: Utc::now(),
                entries,
                signature: None,
            };
            catalog.validate()?;
            let bytes = serde_json::to_vec_pretty(&catalog)?;
            if let Some(output) = output {
                std::fs::write(&output, bytes)?;
                println!("built {}", output.display());
            } else {
                println!("{}", String::from_utf8(bytes)?);
            }
            Ok(())
        }
        ModelCatalogCommand::Verify { catalog_file } => {
            let value = ModelCatalog::from_json(&std::fs::read(&catalog_file)?)?;
            println!(
                "verified {} ({} entries)",
                value.catalog_id,
                value.entries.len()
            );
            Ok(())
        }
    }
}

async fn recipe(command: ModelRecipeCommand) -> Result<()> {
    match command {
        ModelRecipeCommand::Audit { recipe } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&audit_model_recipe(&recipe)?)?
            );
        }
        ModelRecipeCommand::Fetch { recipe } => {
            let report = fetch_model_recipe(&recipe, &CancellationToken::new()).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        ModelRecipeCommand::Build {
            recipe,
            output,
            catalog_entry,
            verification_report,
        } => {
            let report = build_model_recipe(&recipe, &output)?;
            if let Some(path) = catalog_entry {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, serde_json::to_vec_pretty(&report.catalog_entry)?)?;
                println!("catalog entry: {}", path.display());
            }
            if let Some(path) = verification_report {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
                println!("verification report: {}", path.display());
            }
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        ModelRecipeCommand::Verify { recipe } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&verify_model_recipe(&recipe)?)?
            );
        }
    }
    Ok(())
}

fn registry(workspace: &Path) -> Result<ModelBundleRegistry> {
    std::fs::create_dir_all(workspace)?;
    Ok(ModelBundleRegistry::open(workspace.join(".annotagent"))?)
}

fn plugins(workspace: &Path) -> Result<PluginRegistry> {
    Ok(PluginRegistry::open(workspace.join(".annotagent/plugins"))?)
}

fn list_catalogs(workspace: &Path) -> Result<()> {
    let registry = registry(workspace)?;
    let local = registry
        .trusted_local_catalogs()
        .into_iter()
        .map(|source| (source.catalog_id, source.root))
        .collect::<std::collections::BTreeMap<_, _>>();
    for catalog in registry.catalogs() {
        let source = if catalog.catalog_id == annotagent_model_catalog::BUILTIN_FIXTURE_CATALOG_ID {
            "builtin_fixture".to_owned()
        } else if let Some(root) = local.get(&catalog.catalog_id) {
            format!("trusted_local_catalog:{}", root.display())
        } else {
            "curated_remote".to_owned()
        };
        println!(
            "{} · {} bundles · {} · {}",
            catalog.catalog_id,
            catalog.entries.len(),
            catalog.generated_at,
            source
        );
    }
    Ok(())
}

fn normalized(value: &str) -> String {
    value.to_ascii_lowercase().replace(['-', '_'], " ")
}

fn search(workspace: &Path, query: &str) -> Result<()> {
    let query = normalized(query);
    let mut matched = 0_usize;
    for catalog in registry(workspace)?.catalogs() {
        for entry in catalog.entries {
            let haystack = normalized(&format!(
                "{} {} {} {:?}",
                entry.bundle_id, entry.display_name, entry.description, entry.capabilities
            ));
            if haystack.contains(&query) {
                matched += 1;
                println!(
                    "{}@{} · {} · catalog={} · fixture={} · publishable={}",
                    entry.bundle_id,
                    entry.bundle_version,
                    entry.display_name,
                    catalog.catalog_id,
                    entry.fixture,
                    entry.publishable
                );
            }
        }
    }
    if matched == 0 {
        println!("No compatible Catalog Bundle matched {query:?}.");
    }
    Ok(())
}

fn show(workspace: &Path, requested: &str) -> Result<()> {
    let (id, requested_version) = parse_optional_bundle(requested)?;
    let entries = registry(workspace)?
        .catalogs()
        .into_iter()
        .flat_map(|catalog| {
            let catalog_id = catalog.catalog_id;
            catalog
                .entries
                .into_iter()
                .map(move |entry| (catalog_id.clone(), entry))
        })
        .filter(|(_, entry)| {
            entry.bundle_id == id
                && requested_version
                    .as_ref()
                    .is_none_or(|version| entry.bundle_version == *version)
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        bail!("Model Bundle {requested} was not found in configured Catalogs");
    }
    for (catalog_id, entry) in entries {
        println!("catalog: {catalog_id}");
        println!("{}", serde_json::to_string_pretty(&entry)?);
    }
    Ok(())
}

async fn install(workspace: &Path, requested: &str, accept: bool) -> Result<()> {
    let (id, version) = parse_bundle(requested)?;
    let mut registry = registry(workspace)?;
    let (catalog_id, entry) = registry
        .catalogs()
        .into_iter()
        .find_map(|catalog| {
            catalog
                .entries
                .into_iter()
                .find(|entry| entry.bundle_id == id && entry.bundle_version == version)
                .map(|entry| (catalog.catalog_id, entry))
        })
        .with_context(|| {
            format!("Model Bundle {id}@{version} was not found in configured Catalogs")
        })?;
    println!("Bundle: {} {}", entry.display_name, entry.bundle_version);
    println!(
        "License: {} ({})",
        entry.license_summary.name, entry.license_summary.license_digest
    );
    println!(
        "Publisher: {} · verified={}",
        entry.publisher.display_name, entry.publisher.verified
    );
    if entry.license_summary.requires_acceptance {
        let already_accepted = registry.license_acceptances().iter().any(|value| {
            value.bundle_id == id
                && value.bundle_version == version
                && value.license_digest == entry.license_summary.license_digest
        });
        if !already_accepted && !accept {
            bail!("review the exact license digest and repeat with --accept to install it");
        }
        if !already_accepted {
            registry.accept_license(ModelLicenseAcceptance {
                bundle_id: id.clone(),
                bundle_version: version.clone(),
                license_digest: entry.license_summary.license_digest.clone(),
                accepted_at: Utc::now(),
                accepted_by: LicenseAcceptanceActor::LocalUser,
            })?;
        }
    }
    let download_root = registry.data_root().join("model-downloads");
    std::fs::create_dir_all(&download_root)?;
    let package = download_root.join(format!(
        "{}-{}.annotmodel",
        id.as_str().replace('.', "_"),
        uuid::Uuid::new_v4()
    ));
    if let Some(source) = registry.local_catalog_bundle_path(&catalog_id, &id, &version) {
        std::fs::copy(source, &package)?;
    } else {
        ModelCatalogClient::new()?
            .download_bundle(&entry, &package, &CancellationToken::new(), None)
            .await?;
    }
    let install_result = (|| -> Result<_> {
        let verified = verify_model_bundle(&package)?;
        if verified.manifest.id != id
            || verified.manifest.version != version
            || verified.bundle_digest != entry.bundle_sha256
            || verified.manifest.license.license_digest != entry.license_summary.license_digest
        {
            bail!("downloaded Bundle identity, digest or license does not match the Catalog");
        }
        Ok(registry.install_verified(
            verified,
            ModelBundleInstallSource::CuratedCatalog { catalog_id },
        )?)
    })();
    let _ = std::fs::remove_file(&package);
    let installed = install_result?;
    let instances = bind_instances(workspace, &mut registry, &installed)?;
    println!(
        "installed {}@{} · {}",
        installed.manifest.id, installed.manifest.version, installed.bundle_digest
    );
    if instances.is_empty() {
        println!(
            "setup required: install and enable a compatible Rust Expert Model Plugin, then reinstall or use the GUI binding action"
        );
    } else {
        for instance in instances {
            println!("model instance: {} · {:?}", instance.id, instance.status);
        }
    }
    Ok(())
}

fn import(workspace: &Path, package: &Path, accept: bool) -> Result<()> {
    let verified = verify_model_bundle(package)?;
    println!(
        "Bundle: {} {}",
        verified.manifest.display_name, verified.manifest.version
    );
    println!(
        "License: {} ({})",
        verified.manifest.license.name, verified.manifest.license.license_digest
    );
    if verified.manifest.license.requires_acceptance && !accept {
        bail!("review the exact Bundle license and repeat with --accept to import it");
    }
    let mut registry = registry(workspace)?;
    if verified.manifest.license.requires_acceptance {
        registry.accept_license(ModelLicenseAcceptance {
            bundle_id: verified.manifest.id.clone(),
            bundle_version: verified.manifest.version.clone(),
            license_digest: verified.manifest.license.license_digest.clone(),
            accepted_at: Utc::now(),
            accepted_by: LicenseAcceptanceActor::LocalUser,
        })?;
    }
    let installed = registry.import_local(package)?;
    let instances = bind_instances(workspace, &mut registry, &installed)?;
    println!(
        "imported {}@{} · {} · {} Model Instances",
        installed.manifest.id,
        installed.manifest.version,
        installed.bundle_digest,
        instances.len()
    );
    Ok(())
}

fn bind_instances(
    workspace: &Path,
    registry: &mut ModelBundleRegistry,
    installed: &annotagent_model_catalog::InstalledModelBundle,
) -> Result<Vec<annotagent_model_catalog::InstalledModelInstance>> {
    let plugin_registry = plugins(workspace)?;
    let installed_plugins = plugin_registry.list();
    let mut instances = Vec::new();
    for requirement in &installed.manifest.compatible_plugins {
        for plugin in installed_plugins.iter().filter(|plugin| {
            requirement.accepts(
                &plugin.manifest.id,
                &plugin.manifest.version,
                &requirement.model_id,
            )
        }) {
            let execution_provider = installed
                .manifest
                .runtime
                .execution_providers
                .iter()
                .find(|provider| {
                    plugin
                        .manifest
                        .models
                        .iter()
                        .find(|model| model.id == requirement.model_id)
                        .is_some_and(|model| model.runtime_requirements.devices.contains(provider))
                })
                .cloned()
                .unwrap_or_else(|| "cpu".to_owned());
            instances.push(registry.bind_model_instance(BindModelInstanceRequest {
                plugin: &plugin.manifest,
                plugin_package_digest: plugin.package_digest.clone(),
                runtime_status: plugin.runtime_status(),
                bundle_id: &installed.manifest.id,
                bundle_version: &installed.manifest.version,
                model_id: &requirement.model_id,
                target: &annotagent_plugin_host::current_target(),
                execution_provider: &execution_provider,
            })?);
        }
    }
    Ok(instances)
}

fn list(workspace: &Path) -> Result<()> {
    let registry = registry(workspace)?;
    if registry.list().is_empty() {
        println!("No Model Bundles are installed.");
    }
    for bundle in registry.list() {
        println!(
            "{}@{} · {:?} · enabled={} · {}",
            bundle.manifest.id,
            bundle.manifest.version,
            bundle.status,
            bundle.enabled,
            bundle.bundle_digest
        );
    }
    for instance in registry.model_instances() {
        println!(
            "instance {} · {} · {:?} · {}",
            instance.id, instance.model_id, instance.status, instance.execution_provider
        );
    }
    Ok(())
}

async fn test_instance(workspace: &Path, id: ModelInstanceId) -> Result<()> {
    let mut model_registry = registry(workspace)?;
    let instance = model_registry
        .model_instance(id)
        .cloned()
        .context("Model Instance was not found")?;
    let bundle = model_registry
        .get(&instance.model_bundle_id, &instance.model_bundle_version)
        .cloned()
        .context("Model Bundle was not found")?;
    if !bundle.enabled {
        bail!("Model Bundle is disabled");
    }
    let prepared = prepare_bundle_smoke_test(&bundle, &instance.model_id)?;
    let plugin_registry = plugins(workspace)?;
    let installation = plugin_registry
        .get(&instance.plugin_id, &instance.plugin_version)?
        .clone();
    if installation.package_digest != instance.plugin_package_digest {
        bail!("installed Plugin package no longer matches the Model Instance");
    }
    let model_files = bundle
        .manifest
        .files
        .iter()
        .map(|file| {
            (
                file.role.as_str().to_owned(),
                bundle.content_root.join(&file.path),
            )
        })
        .collect();
    let config = plugin_registry.process_config_for_model_files(
        &installation,
        &bundle.content_root,
        model_files,
    )?;
    let started_at = Utc::now();
    let started = std::time::Instant::now();
    let result =
        match run_model_instance_smoke(installation.manifest, config, &prepared.request).await {
            Ok(report) => {
                let mut result = evaluate_bundle_smoke_response(
                    &prepared.definition,
                    &prepared.request,
                    &report.response,
                    elapsed_ms(started),
                    started_at,
                );
                result.checks.push(SmokeTestCheck {
                    name: "plugin conformance".to_owned(),
                    passed: report.conformance.passed,
                    detail: "Rust Plugin health, capability, model and Contract discovery passed"
                        .to_owned(),
                });
                if !report.conformance.passed {
                    result.status = SmokeTestStatus::Failed;
                }
                result
            }
            Err(error) => SmokeTestResult {
                test_id: prepared.definition.test_id,
                status: SmokeTestStatus::Crashed,
                checks: vec![SmokeTestCheck {
                    name: "plugin process".to_owned(),
                    passed: false,
                    detail: format!(
                        "Rust Plugin smoke process failed ({})",
                        plugin_error_kind(&error)
                    ),
                }],
                duration_ms: elapsed_ms(started),
                started_at,
                finished_at: Utc::now(),
            },
        };
    let instance = model_registry.record_model_instance_smoke(id, result)?;
    println!("{}", serde_json::to_string_pretty(&instance)?);
    if instance.status != annotagent_model_bundle::ModelInstanceStatus::Ready {
        bail!("Model Instance smoke test did not pass");
    }
    Ok(())
}

fn elapsed_ms(started: std::time::Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn plugin_error_kind(error: &PluginRegistryError) -> &'static str {
    match error {
        PluginRegistryError::Host(_) => "plugin_host",
        PluginRegistryError::InvalidWeight(_) => "model_files",
        _ => "plugin_registry",
    }
}

fn set_enabled(workspace: &Path, id: ModelInstanceId, enabled: bool) -> Result<()> {
    let mut registry = registry(workspace)?;
    let instance = registry
        .model_instance(id)
        .cloned()
        .context("Model Instance was not found")?;
    if enabled {
        registry.enable(&instance.model_bundle_id, &instance.model_bundle_version)?;
    } else {
        registry.disable(&instance.model_bundle_id, &instance.model_bundle_version)?;
    }
    println!(
        "{} {}@{}",
        if enabled { "enabled" } else { "disabled" },
        instance.model_bundle_id,
        instance.model_bundle_version
    );
    Ok(())
}

fn remove(workspace: &Path, requested: &str) -> Result<()> {
    let (id, version) = parse_bundle(requested)?;
    registry(workspace)?.remove(&id, &version)?;
    println!("removed {id}@{version}");
    Ok(())
}

fn references(workspace: &Path, requested: &str) -> Result<()> {
    let (id, version) = parse_bundle(requested)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&registry(workspace)?.references(&id, &version))?
    );
    Ok(())
}

fn doctor(workspace: &Path, id: ModelInstanceId) -> Result<()> {
    let registry = registry(workspace)?;
    let instance = registry
        .model_instance(id)
        .context("Model Instance was not found")?;
    let bundle = registry
        .get(&instance.model_bundle_id, &instance.model_bundle_version)
        .context("Model Bundle was not found")?;
    let report = serde_json::json!({
        "instance": instance,
        "bundle_enabled": bundle.enabled,
        "bundle_publishable": bundle.manifest.publishable,
        "bundle_fixture": bundle.manifest.fixture,
        "license_accepted": !bundle.manifest.license.requires_acceptance || registry.license_acceptances().iter().any(|value| value.bundle_id == bundle.manifest.id && value.bundle_version == bundle.manifest.version && value.license_digest == bundle.manifest.license.license_digest),
        "references": registry.references(&bundle.manifest.id, &bundle.manifest.version),
        "diagnosis": if instance.status == annotagent_model_bundle::ModelInstanceStatus::Ready && bundle.manifest.publishable { "workflow_ready" } else if bundle.manifest.fixture { "fixture_only_not_publishable" } else { "setup_or_repair_required" },
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn garbage_collect(workspace: &Path) -> Result<()> {
    let report = registry(workspace)?.garbage_collect()?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn parse_instance_id(value: &str) -> Result<ModelInstanceId> {
    value.parse().context("invalid Model Instance id")
}

fn parse_bundle(value: &str) -> Result<(ModelBundleId, Version)> {
    let (id, version) = value
        .rsplit_once('@')
        .context("expected <bundle-id>@<version>")?;
    Ok((ModelBundleId::parse(id)?, Version::parse(version)?))
}

fn parse_optional_bundle(value: &str) -> Result<(ModelBundleId, Option<Version>)> {
    if let Some((id, version)) = value.rsplit_once('@') {
        Ok((ModelBundleId::parse(id)?, Some(Version::parse(version)?)))
    } else {
        Ok((ModelBundleId::parse(value)?, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_spec_uses_the_final_at_separator() {
        let (id, version) = parse_bundle("org.annotagent.models.fixture@1.2.3").expect("spec");
        assert_eq!(id.as_str(), "org.annotagent.models.fixture");
        assert_eq!(version, Version::new(1, 2, 3));
    }

    #[test]
    fn search_normalizes_capability_spelling() {
        assert_eq!(normalized("Prompted-Segmentation"), "prompted segmentation");
        assert_eq!(normalized("prompted_segmentation"), "prompted segmentation");
    }
}
