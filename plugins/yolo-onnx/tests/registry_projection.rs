use std::path::PathBuf;

use annotagent_core::{ModelAvailability, ModelCapability};
use annotagent_plugin_api::PluginStatus;
use annotagent_plugin_host::{current_target, pack_directory};
use annotagent_plugin_registry::{InstallApproval, PluginRegistry};
use tempfile::tempdir;

#[test]
fn yolo_package_projects_an_exact_needs_weights_model_profile() {
    let root = tempdir().expect("tempdir");
    let package_source = root.path().join("package");
    let binary_directory = package_source.join("bin").join(current_target());
    std::fs::create_dir_all(&binary_directory).expect("bin");
    std::fs::copy(
        env!("CARGO_BIN_EXE_annotagent-plugin-yolo-onnx"),
        binary_directory.join(executable_name()),
    )
    .expect("copy executable");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("annotagent-plugin.toml"),
        package_source.join("annotagent-plugin.toml"),
    )
    .expect("copy manifest");
    let package = root.path().join("yolo.annotplugin");
    pack_directory(&package_source, &package).expect("pack");
    let mut registry = PluginRegistry::open(root.path().join("registry")).expect("registry");
    let rejected = registry.install(
        &package,
        &InstallApproval {
            permissions_reviewed: true,
            code_license_accepted: true,
            weight_license_accepted: false,
        },
    );
    assert!(rejected.is_err());
    let installation = registry
        .install(
            &package,
            &InstallApproval {
                permissions_reviewed: true,
                code_license_accepted: true,
                weight_license_accepted: true,
            },
        )
        .expect("explicit test approval");
    assert_eq!(installation.status, PluginStatus::NeedsWeights);
    let profiles = registry.ready_models();
    assert_eq!(profiles.len(), 1);
    let profile = &profiles[0];
    assert_eq!(profile.reference.model_id, "yolox-nano-coco-onnx");
    assert_eq!(
        profile.reference.plugin_id.as_str(),
        "org.annotagent.yolo-onnx"
    );
    assert_eq!(profile.reference.plugin_version.to_string(), "1.0.0");
    assert_eq!(
        profile.reference.package_digest,
        installation.package_digest
    );
    assert_eq!(profile.availability, ModelAvailability::MissingWeights);
    assert_eq!(profile.plugin_status, PluginStatus::NeedsWeights);
    assert!(
        profile
            .capabilities
            .contains(&ModelCapability::ObjectDetection)
    );
    assert!(profile.reference.checkpoint_sha256.is_none());
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "annotagent-plugin-yolo-onnx.exe"
    } else {
        "annotagent-plugin-yolo-onnx"
    }
}
