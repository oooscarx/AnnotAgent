use std::path::PathBuf;

use annotagent_core::{ModelAvailability, ModelCapability};
use annotagent_plugin_api::{PluginStatus, Sha256Digest};
use annotagent_plugin_host::{current_target, pack_directory};
use annotagent_plugin_registry::{InstallApproval, PluginRegistry};

#[test]
fn sam_requires_both_hashed_components_before_leaving_needs_weights() {
    let root = tempfile::tempdir().expect("tempdir");
    let package_source = root.path().join("package");
    let binary_directory = package_source.join("bin").join(current_target());
    std::fs::create_dir_all(&binary_directory).expect("bin");
    std::fs::copy(
        env!("CARGO_BIN_EXE_annotagent-plugin-sam-onnx"),
        binary_directory.join(executable_name()),
    )
    .expect("copy executable");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("annotagent-plugin.toml"),
        package_source.join("annotagent-plugin.toml"),
    )
    .expect("copy manifest");
    let package = root.path().join("sam.annotplugin");
    pack_directory(&package_source, &package).expect("pack");
    let registry_root = root.path().join("registry");
    let mut registry = PluginRegistry::open(&registry_root).expect("registry");
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
    assert_eq!(
        registry.ready_models()[0].availability,
        ModelAvailability::MissingWeights
    );

    let encoder = root.path().join("local-encoder.bin");
    let decoder = root.path().join("local-decoder.bin");
    std::fs::write(&encoder, b"encoder fixture identity").expect("encoder");
    std::fs::write(&decoder, b"decoder fixture identity").expect("decoder");
    let encoder_set = registry
        .provision_local_weight_component(
            &installation.manifest.id,
            &installation.manifest.version,
            "sam-vit-b-onnx",
            "image_encoder",
            &encoder,
            None,
        )
        .expect("encoder component");
    assert_eq!(encoder_set.original_filename, "local-encoder.bin");
    assert_eq!(
        encoder_set
            .stored_path
            .file_name()
            .and_then(std::ffi::OsStr::to_str),
        Some("sam_image_encoder.onnx")
    );
    assert_eq!(
        registry
            .get(&installation.manifest.id, &installation.manifest.version)
            .expect("installation")
            .status,
        PluginStatus::NeedsWeights
    );

    let decoder_set = registry
        .provision_local_weight_component(
            &installation.manifest.id,
            &installation.manifest.version,
            "sam-vit-b-onnx",
            "mask_decoder",
            &decoder,
            None,
        )
        .expect("decoder component");
    assert_eq!(
        decoder_set
            .stored_path
            .file_name()
            .and_then(std::ffi::OsStr::to_str),
        Some("sam_mask_decoder.onnx")
    );
    assert_eq!(
        registry
            .get(&installation.manifest.id, &installation.manifest.version)
            .expect("installation")
            .status,
        PluginStatus::Installed
    );
    let profile = &registry.ready_models()[0];
    assert_eq!(profile.availability, ModelAvailability::Unknown);
    assert!(
        profile
            .capabilities
            .contains(&ModelCapability::PromptedSegmentation)
    );
    let expected_identity = Sha256Digest::of_bytes(
        format!(
            "image_encoder:{}\nmask_decoder:{}",
            encoder_set.checkpoint_sha256, decoder_set.checkpoint_sha256
        )
        .as_bytes(),
    );
    assert_eq!(
        profile.reference.checkpoint_sha256.as_ref(),
        Some(&expected_identity)
    );

    let reopened = PluginRegistry::open(registry_root).expect("reopen");
    assert_eq!(
        reopened.ready_models()[0].reference.checkpoint_sha256,
        Some(expected_identity)
    );
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "annotagent-plugin-sam-onnx.exe"
    } else {
        "annotagent-plugin-sam-onnx"
    }
}
