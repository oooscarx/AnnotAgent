use std::path::PathBuf;

use annotagent_core::{ModelAvailability, ModelCapability};
use annotagent_plugin_api::PluginStatus;
use annotagent_plugin_host::{current_target, pack_directory};
use annotagent_plugin_registry::{InstallApproval, PluginRegistry};

#[test]
fn pidnet_package_projects_a_semantic_needs_weights_profile() {
    let root = tempfile::tempdir().expect("tempdir");
    let package_source = root.path().join("package");
    let binary_directory = package_source.join("bin").join(current_target());
    std::fs::create_dir_all(&binary_directory).expect("bin");
    std::fs::copy(
        env!("CARGO_BIN_EXE_annotagent-plugin-pidnet-onnx"),
        binary_directory.join(executable_name()),
    )
    .expect("copy executable");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("annotagent-plugin.toml"),
        package_source.join("annotagent-plugin.toml"),
    )
    .expect("copy manifest");
    let package = root.path().join("pidnet.annotplugin");
    pack_directory(&package_source, &package).expect("pack");
    let mut registry = PluginRegistry::open(root.path().join("registry")).expect("registry");
    let installation = registry
        .install(
            &package,
            &InstallApproval {
                permissions_reviewed: true,
                code_license_accepted: true,
                weight_license_accepted: true,
            },
        )
        .expect("install");
    assert_eq!(installation.status, PluginStatus::NeedsWeights);
    let profile = &registry.ready_models()[0];
    assert_eq!(profile.availability, ModelAvailability::MissingWeights);
    assert!(
        profile
            .capabilities
            .contains(&ModelCapability::SemanticSegmentation)
    );
    assert!(profile.reference.checkpoint_sha256.is_none());
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "annotagent-plugin-pidnet-onnx.exe"
    } else {
        "annotagent-plugin-pidnet-onnx"
    }
}
