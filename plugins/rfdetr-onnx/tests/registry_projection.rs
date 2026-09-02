use std::path::PathBuf;

use annotagent_core::{ModelAvailability, ModelCapability};
use annotagent_plugin_api::{PluginImplementationStatus, PluginStatus, Sha256Digest};
use annotagent_plugin_host::{current_target, pack_directory};
use annotagent_plugin_registry::{InstallApproval, PluginRegistry};

#[test]
fn rfdetr_projects_a_hashed_live_conditional_model_until_real_smoke() {
    let root = tempfile::tempdir().expect("tempdir");
    let package_source = root.path().join("package");
    let binary_directory = package_source.join("bin").join(current_target());
    std::fs::create_dir_all(&binary_directory).expect("bin");
    std::fs::copy(
        env!("CARGO_BIN_EXE_annotagent-plugin-rfdetr-onnx"),
        binary_directory.join(executable_name()),
    )
    .expect("copy executable");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("annotagent-plugin.toml"),
        package_source.join("annotagent-plugin.toml"),
    )
    .expect("copy manifest");
    let package = root.path().join("rfdetr.annotplugin");
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
    assert_eq!(
        installation.manifest.implementation_status,
        PluginImplementationStatus::LiveConditional
    );
    assert_eq!(installation.status, PluginStatus::NeedsWeights);
    assert_eq!(
        registry.ready_models()[0].availability,
        ModelAvailability::MissingWeights
    );
    assert!(
        registry.ready_models()[0]
            .capabilities
            .contains(&ModelCapability::ObjectDetection)
    );

    let local_export = root.path().join("user-export.onnx");
    std::fs::write(&local_export, b"RF-DETR contract-only export identity").expect("fixture");
    let provisioned = registry
        .provision_local_weight_component(
            &installation.manifest.id,
            &installation.manifest.version,
            "rfdetr-detection-onnx-v1",
            "model",
            &local_export,
            None,
        )
        .expect("provision");
    assert_eq!(
        provisioned.checkpoint_sha256,
        Sha256Digest::of_bytes(b"RF-DETR contract-only export identity")
    );
    assert_eq!(
        provisioned
            .stored_path
            .file_name()
            .and_then(std::ffi::OsStr::to_str),
        Some("rfdetr.onnx")
    );
    assert_eq!(
        registry
            .get(&installation.manifest.id, &installation.manifest.version)
            .expect("installation")
            .status,
        PluginStatus::Installed
    );
    assert_eq!(
        registry.ready_models()[0].availability,
        ModelAvailability::Unknown
    );
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "annotagent-plugin-rfdetr-onnx.exe"
    } else {
        "annotagent-plugin-rfdetr-onnx"
    }
}
