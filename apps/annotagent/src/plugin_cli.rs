use std::{collections::BTreeMap, path::PathBuf};

use annotagent_core::{
    ImageId, ModelCapability, ModelImage, PipelineInferenceRequest, RunId, VisionCapability,
};
use annotagent_plugin_api::{PluginId, PluginStatus, PluginVersion, Sha256Digest};
use annotagent_plugin_host::{
    HostedPlugin, PluginProcessConfig, pack_directory, process_directories, verify_package,
};
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
            weights,
            sha256,
        } => {
            let plugin_id = PluginId::parse(plugin_id)?;
            let version = PluginVersion::parse(&version)?;
            let expected = sha256.map(Sha256Digest::parse).transpose()?;
            let mut registry = PluginRegistry::open(data_root)?;
            let provisioned = registry.provision_local_weights(
                &plugin_id,
                &version,
                &model,
                &weights,
                expected.as_ref(),
            )?;
            println!("checkpoint sha256: {}", provisioned.checkpoint_sha256);
            println!("stored: {}", provisioned.stored_path.display());
        }
        PluginCommand::Test { plugin_id, version } => {
            let mut registry = PluginRegistry::open(data_root)?;
            let installation = resolve(&registry, &plugin_id, version.as_deref())?;
            let report = test_installation(&registry, &installation).await?;
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

async fn test_installation(
    registry: &PluginRegistry,
    installation: &PluginInstallation,
) -> Result<annotagent_plugin_api::PluginTestReport> {
    if installation.status == PluginStatus::NeedsWeights {
        bail!("plugin requires checkpoint provisioning before process testing");
    }
    let host = start_installation(registry, installation).await?;
    let sample = sample_request(installation)?;
    let report = host.test(Some(&sample)).await?;
    host.stop().await?;
    Ok(report)
}

async fn start_installation(
    registry: &PluginRegistry,
    installation: &PluginInstallation,
) -> Result<HostedPlugin> {
    let executable =
        registry.executable(&installation.manifest.id, &installation.manifest.version)?;
    let process_root = registry
        .data_root()
        .join("plugin-state")
        .join(installation.manifest.id.as_str())
        .join(installation.manifest.version.to_string());
    let (state, cache, temporary) = process_directories(&process_root);
    let weights =
        registry.weights_root(&installation.manifest.id, &installation.manifest.version)?;
    let response_bytes = installation
        .manifest
        .resources
        .maximum_response_mb
        .saturating_mul(1024 * 1024);
    Ok(HostedPlugin::start(
        installation.manifest.clone(),
        PluginProcessConfig {
            executable,
            installation_root: installation.installation_root.clone(),
            state_dir: state,
            weights_dir: weights,
            cache_dir: cache,
            temporary_dir: temporary,
            max_request_bytes: 64 * 1024 * 1024,
            max_response_bytes: usize::try_from(response_bytes).unwrap_or(256 * 1024 * 1024),
        },
    )
    .await?)
}

fn sample_request(installation: &PluginInstallation) -> Result<PipelineInferenceRequest> {
    let model = installation
        .manifest
        .models
        .first()
        .context("plugin declares no model")?;
    let capability = *model
        .capabilities
        .first()
        .context("model declares no capability")?;
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
            bail!("text generation is not an expert vision operation")
        }
    };
    Ok(PipelineInferenceRequest {
        protocol_version: annotagent_core::PIPELINE_VISION_PROTOCOL_VERSION,
        request_id: uuid::Uuid::new_v4().to_string(),
        run_id: RunId::new(),
        image_id: ImageId::new(),
        node_id: "plugin_conformance".to_owned(),
        model_id: model.id.clone(),
        operation,
        image: Some(ModelImage {
            id: "conformance-image".to_owned(),
            mime_type: "image/png".to_owned(),
            data_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
        }),
        input_artifacts: Vec::new(),
        parameters: BTreeMap::new(),
        timeout_ms: Some(30_000),
    })
}
