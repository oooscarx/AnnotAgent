use std::{collections::BTreeMap, path::PathBuf};

use annotagent_core::{
    ImageId, PipelineArtifact, PipelineInferenceRequest, RunId, VisionCapability,
};
use annotagent_plugin_api::{PluginManifest, PluginStatus};
use annotagent_plugin_host::{HostedPlugin, PluginProcessConfig, process_directories};

#[tokio::test]
async fn host_contains_process_crash_and_preserves_typed_inference() {
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_annotagent-plugin-dummy-detector"));
    let installation_root = executable.parent().expect("binary parent").to_path_buf();
    let temporary = tempfile::tempdir().expect("temporary root");
    let process_root = temporary.path().join("process");
    let weights = temporary.path().join("weights");
    std::fs::create_dir_all(&weights).expect("weights");
    let (state, cache, scratch) = process_directories(&process_root);
    let manifest =
        PluginManifest::from_toml(include_str!("../annotagent-plugin.toml")).expect("manifest");
    let host = HostedPlugin::start(
        manifest,
        PluginProcessConfig {
            executable,
            installation_root,
            state_dir: state,
            weights_dir: weights,
            cache_dir: cache,
            temporary_dir: scratch,
            max_request_bytes: 1_000_000,
            max_response_bytes: 1_000_000,
        },
    )
    .await
    .expect("host");
    assert_eq!(host.health().await.expect("health").status, "ready");

    let request = PipelineInferenceRequest {
        protocol_version: annotagent_core::PIPELINE_VISION_PROTOCOL_VERSION,
        request_id: "host-process-1".to_owned(),
        run_id: RunId::new(),
        image_id: ImageId::new(),
        node_id: "detector".to_owned(),
        model_id: "dummy-detector-v1".to_owned(),
        operation: VisionCapability::ObjectDetection,
        image: None,
        input_artifacts: Vec::new(),
        parameters: BTreeMap::new(),
        timeout_ms: Some(1_000),
    };
    let response = host.infer(&request).await.expect("infer");
    assert!(matches!(
        response.artifacts.as_slice(),
        [PipelineArtifact::DetectionSet(_)]
    ));
    assert!(host.test(Some(&request)).await.expect("test").passed);

    host.kill_for_test().await.expect("kill");
    for _ in 0..20 {
        if host.status().await.expect("status") == PluginStatus::Crashed {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(host.status().await.expect("status"), PluginStatus::Crashed);
    assert!(host.health().await.is_err());
}
