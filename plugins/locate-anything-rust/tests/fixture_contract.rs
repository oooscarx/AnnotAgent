use std::{collections::BTreeMap, io::Cursor, path::PathBuf};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ImageArtifact, ImageId, ModelImage, PipelineArtifact,
    PipelineInferenceRequest, RunId, VisionCapability,
};
use annotagent_plugin_api::PluginManifest;
use annotagent_plugin_host::{HostedPlugin, PluginProcessConfig, process_directories};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use tempfile::tempdir;

#[tokio::test]
async fn scripted_rust_fixture_proves_protocol_without_claiming_model_inference() {
    let root = tempdir().expect("tempdir");
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(RgbImage::from_pixel(2, 2, Rgb([40, 80, 120])))
        .write_to(&mut png, ImageFormat::Png)
        .expect("png");
    let image_bytes = png.into_inner();
    let image_id = ImageId::new();
    let image_artifact = PipelineArtifact::Image(ImageArtifact {
        reference: ArtifactRef {
            artifact_id: "image:locate-fixture".to_owned(),
            source_node: "image".to_owned(),
            port: "image".to_owned(),
            artifact_type: ArtifactKind::Image,
            item_id: None,
        },
        image_id,
        width: 2,
        height: 2,
        mime_type: "image/png".to_owned(),
        blob_ref: "test://locate-fixture".to_owned(),
        parent: None,
        root_region: None,
    });
    let request = PipelineInferenceRequest {
        protocol_version: annotagent_core::PIPELINE_VISION_PROTOCOL_VERSION,
        request_id: uuid::Uuid::new_v4().to_string(),
        run_id: RunId::new(),
        image_id,
        node_id: "locate".to_owned(),
        model_id: "locate-anything-3b-rust-contract".to_owned(),
        operation: VisionCapability::OpenVocabularyDetection,
        image: Some(ModelImage {
            id: "locate-fixture".to_owned(),
            mime_type: "image/png".to_owned(),
            data_base64: STANDARD.encode(image_bytes),
        }),
        input_artifacts: vec![image_artifact],
        parameters: BTreeMap::from([("queries".to_owned(), serde_json::json!(["ball", "robot"]))]),
        timeout_ms: Some(5_000),
    };
    let manifest =
        PluginManifest::from_toml(include_str!("../annotagent-plugin.toml")).expect("manifest");
    let executable = PathBuf::from(env!(
        "CARGO_BIN_EXE_annotagent-plugin-locate-anything-fixture"
    ));
    let installation_root = executable.parent().expect("target directory").to_path_buf();
    let (state_dir, cache_dir, temporary_dir) = process_directories(&root.path().join("process"));
    let weights_dir = root.path().join("weights");
    std::fs::create_dir_all(&weights_dir).expect("weights");
    let hosted = HostedPlugin::start(
        manifest,
        PluginProcessConfig {
            executable,
            installation_root,
            state_dir,
            weights_dir,
            cache_dir,
            temporary_dir,
            max_request_bytes: 8 * 1024 * 1024,
            max_response_bytes: 8 * 1024 * 1024,
        },
    )
    .await
    .expect("start fixture");
    let conformance = hosted
        .test(Some(&request))
        .await
        .expect("fixture conformance");
    assert!(conformance.passed, "{:?}", conformance.checks);
    let response = hosted.infer(&request).await.expect("fixture inference");
    let PipelineArtifact::DetectionSet(detections) = &response.artifacts[0] else {
        panic!("DetectionSet")
    };
    assert_eq!(detections.detections.len(), 2);
    assert_eq!(detections.metadata["protocol_fixture"], true);
    assert_eq!(detections.metadata["real_inference"], false);
    assert!(
        detections
            .detections
            .iter()
            .all(|detection| detection.score.value.is_none())
    );
    hosted.stop().await.expect("stop");
}
