use std::path::PathBuf;

use annotagent_plugin_api::{PluginId, PluginStatus, PluginVersion, Sha256Digest};
use annotagent_plugin_host::{HostedPlugin, pack_directory, verify_package};
use annotagent_plugin_registry::{
    InstallApproval, PluginInstallation, PluginRegistry, default_plugin_data_root,
};
use anyhow::{Context as _, Result, bail};

use crate::PluginCommand;

pub async fn run(command: PluginCommand, data_dir: Option<PathBuf>) -> Result<()> {
    let data_root = data_dir.unwrap_or_else(default_plugin_data_root);
    match command {
        PluginCommand::Inspect { package } | PluginCommand::Verify { package } => {
            let verified = verify_package(&package)?;
            println!("{}", verified.manifest.to_toml()?);
            println!("package sha256: {}", verified.package_digest);
            println!("signature: {:?}", verified.signature);
        }
        PluginCommand::Pack { directory, output } => {
            let digest = pack_directory(&directory, &output)?;
            println!("packed {}", output.display());
            println!("package sha256: {digest}");
        }
        PluginCommand::Install { package, accept } | PluginCommand::Update { package, accept } => {
            let verified = verify_package(&package)?;
            println!(
                "Plugin: {} {}",
                verified.manifest.display_name, verified.manifest.version
            );
            println!("Permissions: {:#?}", verified.manifest.permissions);
            println!("Code license: {}", verified.manifest.license.code);
            println!("Weight license: {}", verified.manifest.license.weights);
            if !accept {
                bail!("review the package and repeat with --accept to install it");
            }
            let mut registry = PluginRegistry::open(data_root)?;
            let installed = registry.install(
                &package,
                &InstallApproval {
                    permissions_reviewed: true,
                    code_license_accepted: true,
                    weight_license_accepted: true,
                },
            )?;
            print_installation(&installed);
        }
        PluginCommand::List => {
            for installation in PluginRegistry::open(data_root)?.list() {
                print_installation(&installation);
            }
        }
        PluginCommand::Show { plugin_id, version }
        | PluginCommand::Doctor { plugin_id, version } => {
            let registry = PluginRegistry::open(data_root)?;
            let installation = resolve(&registry, &plugin_id, version.as_deref())?;
            println!("{}", serde_json::to_string_pretty(&installation)?);
            let references =
                registry.references(&installation.manifest.id, &installation.manifest.version);
            println!("references: {}", references.len());
        }
        PluginCommand::Versions { plugin_id } => {
            let plugin_id = PluginId::parse(plugin_id)?;
            for installation in PluginRegistry::open(data_root)?
                .list()
                .into_iter()
                .filter(|installation| installation.manifest.id == plugin_id)
            {
                println!(
                    "{} {:?}",
                    installation.manifest.version, installation.status
                );
            }
        }
        PluginCommand::Provision {
            plugin_id,
            version,
            model,
            component,
            weights,
            sha256,
        } => {
            let plugin_id = PluginId::parse(plugin_id)?;
            let version = PluginVersion::parse(&version)?;
            let expected = sha256.map(Sha256Digest::parse).transpose()?;
            let mut registry = PluginRegistry::open(data_root)?;
            let provisioned = if let Some(component) = component {
                registry.provision_local_weight_component(
                    &plugin_id,
                    &version,
                    &model,
                    &component,
                    &weights,
                    expected.as_ref(),
                )?
            } else {
                registry.provision_local_weights(
                    &plugin_id,
                    &version,
                    &model,
                    &weights,
                    expected.as_ref(),
                )?
            };
            println!("checkpoint sha256: {}", provisioned.checkpoint_sha256);
            println!("stored: {}", provisioned.stored_path.display());
        }
        PluginCommand::Test { plugin_id, version } => {
            let mut registry = PluginRegistry::open(data_root)?;
            let installation = resolve(&registry, &plugin_id, version.as_deref())?;
            let report = registry.test_installation(&installation).await?;
            let status = registry.record_test(report.clone())?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            println!("status: {status:?}");
        }
        PluginCommand::Start { plugin_id, version }
        | PluginCommand::Restart { plugin_id, version } => {
            let registry = PluginRegistry::open(data_root)?;
            let installation = resolve(&registry, &plugin_id, version.as_deref())?;
            if !installation.enabled || installation.status != PluginStatus::Ready {
                bail!("plugin must be enabled and Ready before it can start");
            }
            let host = start_installation(&registry, &installation).await?;
            println!("running {} at {}", installation.key(), host.endpoint());
            println!("press Ctrl-C to stop");
            tokio::signal::ctrl_c().await?;
            host.stop().await?;
        }
        PluginCommand::Stop { plugin_id, version } => {
            let registry = PluginRegistry::open(data_root)?;
            let installation = resolve(&registry, &plugin_id, version.as_deref())?;
            bail!(
                "{} has no foreground process in this command; stop it in the terminal that started it or through the server lifecycle API",
                installation.key()
            );
        }
        PluginCommand::Enable { plugin_id, version } => {
            let mut registry = PluginRegistry::open(data_root)?;
            let installation = resolve(&registry, &plugin_id, version.as_deref())?;
            let status =
                registry.enable(&installation.manifest.id, &installation.manifest.version)?;
            println!("status: {status:?}");
        }
        PluginCommand::Disable { plugin_id, version } => {
            let mut registry = PluginRegistry::open(data_root)?;
            let installation = resolve(&registry, &plugin_id, version.as_deref())?;
            registry.disable(&installation.manifest.id, &installation.manifest.version)?;
            println!("status: Disabled");
        }
        PluginCommand::Uninstall { plugin_id, version } => {
            let plugin_id = PluginId::parse(plugin_id)?;
            let version = PluginVersion::parse(&version)?;
            PluginRegistry::open(data_root)?.uninstall(&plugin_id, &version)?;
            println!("uninstalled {plugin_id}@{version}");
        }
        PluginCommand::References { plugin_id, version } => {
            let plugin_id = PluginId::parse(plugin_id)?;
            let version = PluginVersion::parse(&version)?;
            let references = PluginRegistry::open(data_root)?.references(&plugin_id, &version);
            println!("{}", serde_json::to_string_pretty(&references)?);
        }
    }
    Ok(())
}

fn resolve(
    registry: &PluginRegistry,
    plugin_id: &str,
    version: Option<&str>,
) -> Result<PluginInstallation> {
    let plugin_id = PluginId::parse(plugin_id)?;
    if let Some(version) = version {
        return Ok(registry
            .get(&plugin_id, &PluginVersion::parse(version)?)?
            .clone());
    }
    registry
        .list()
        .into_iter()
        .filter(|installation| installation.manifest.id == plugin_id)
        .max_by(|left, right| left.manifest.version.cmp(&right.manifest.version))
        .with_context(|| format!("plugin {plugin_id} is not installed"))
}

fn print_installation(installation: &PluginInstallation) {
    println!(
        "{} {} {:?} {}",
        installation.manifest.id,
        installation.manifest.version,
        installation.status,
        installation.installation_root.display()
    );
}

async fn start_installation(
    registry: &PluginRegistry,
    installation: &PluginInstallation,
) -> Result<HostedPlugin> {
    Ok(HostedPlugin::start(
        installation.manifest.clone(),
        registry.process_config(installation)?,
    )
    .await?)
}
