use std::path::PathBuf;

use annotagent_core::{ModelAvailability, ModelCapability};
use annotagent_plugin_api::{PluginImplementationStatus, PluginStatus, PluginTestReport};
use annotagent_plugin_host::{current_target, pack_directory};
use annotagent_plugin_registry::{InstallApproval, PluginRegistry};

#[test]
fn locate_anything_contract_installs_disabled_and_cannot_be_promoted() {
    let root = tempfile::tempdir().expect("tempdir");
    let package_source = root.path().join("package");
    let binary_directory = package_source.join("bin").join(current_target());
    std::fs::create_dir_all(&binary_directory).expect("bin");
    std::fs::copy(
        env!("CARGO_BIN_EXE_annotagent-plugin-locate-anything-rust"),
        binary_directory.join(executable_name()),
    )
    .expect("copy executable");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("annotagent-plugin.toml"),
        package_source.join("annotagent-plugin.toml"),
    )
    .expect("copy manifest");
    let package = root.path().join("locate-anything.annotplugin");
    pack_directory(&package_source, &package).expect("pack");
    let mut registry = PluginRegistry::open(root.path().join("registry")).expect("registry");
    let installation = registry
        .install(
            &package,
            &InstallApproval {
                permissions_reviewed: true,
                code_license_accepted: true,
                weight_license_accepted: false,
            },
        )
        .expect("install contract");
    assert_eq!(
        installation.manifest.implementation_status,
        PluginImplementationStatus::Unsupported
    );
    assert_eq!(installation.status, PluginStatus::UnsupportedPlatform);
    assert!(!installation.enabled);
    let profile = &registry.ready_models()[0];
    assert_eq!(profile.availability, ModelAvailability::Unknown);
    assert!(!profile.enabled);
    assert!(
        profile
            .capabilities
            .contains(&ModelCapability::OpenVocabularyDetection)
    );
    assert!(
        profile
            .capabilities
            .contains(&ModelCapability::PhraseGrounding)
    );

    let status = registry
        .enable(&installation.manifest.id, &installation.manifest.version)
        .expect("enable remains unsupported");
    assert_eq!(status, PluginStatus::UnsupportedPlatform);
    assert!(!registry.ready_models()[0].enabled);
    let report = PluginTestReport {
        plugin_id: installation.manifest.id.clone(),
        plugin_version: installation.manifest.version.clone(),
        passed: true,
        checks: Vec::new(),
        started_at: chrono::Utc::now(),
        finished_at: chrono::Utc::now(),
    };
    assert!(registry.record_test(report).is_err());
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "annotagent-plugin-locate-anything-rust.exe"
    } else {
        "annotagent-plugin-locate-anything-rust"
    }
}
