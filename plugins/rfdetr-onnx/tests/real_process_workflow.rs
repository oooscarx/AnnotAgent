use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ImageArtifact, ImageId, ModelImage, PipelineArtifact,
    PipelineInferenceRequest, ProjectId, RunId, VisionCapability, WorkflowDraftNode,
    WorkflowNodeKind,
};
use annotagent_plugin_api::PluginManifest;
use annotagent_plugin_host::{
    HostedPlugin, PluginPipelineBackend, PluginProcessConfig, process_directories,
};
use annotagent_runtime::{DagNodeContext, DagNodeRunner as _};
use annotagent_skill_object_detection::ObjectDetectionSkillRunner;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

const MODEL_ENV: &str = "ANNOTAGENT_TEST_RFDETR_ONNX";
const IMAGE_ENV: &str = "ANNOTAGENT_TEST_RFDETR_IMAGE";

#[tokio::test]
#[ignore = "requires an explicitly supplied legal official-contract RF-DETR ONNX export and sample image"]
async fn real_rfdetr_process_returns_typed_detection_set() {
    let source_model = PathBuf::from(std::env::var(MODEL_ENV).expect("model path"));
    let source_image = PathBuf::from(std::env::var(IMAGE_ENV).expect("image path"));
    let root = tempdir().expect("tempdir");
    let weights = root.path().join("weights");
    std::fs::create_dir_all(&weights).expect("weights");
    std::fs::copy(source_model, weights.join("rfdetr.onnx")).expect("copy model");
    let image_bytes = std::fs::read(source_image).expect("read image");
    let mime_type = image_mime_type(&image_bytes);
    let decoded = image::load_from_memory(&image_bytes).expect("decode image");
    let image_id = ImageId::new();
    let image_artifact = PipelineArtifact::Image(ImageArtifact {
        reference: ArtifactRef {
            artifact_id: "image:real-rfdetr".to_owned(),
            source_node: "image".to_owned(),
            port: "image".to_owned(),
            artifact_type: ArtifactKind::Image,
            item_id: None,
        },
        image_id,
        width: decoded.width(),
        height: decoded.height(),
        mime_type: mime_type.to_owned(),
        blob_ref: "test://real-rfdetr".to_owned(),
        parent: None,
        root_region: None,
    });
    let model_image = ModelImage {
        id: "real-rfdetr-image".to_owned(),
        mime_type: mime_type.to_owned(),
        data_base64: STANDARD.encode(image_bytes),
    };
    let manifest =
        PluginManifest::from_toml(include_str!("../annotagent-plugin.toml")).expect("manifest");
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_annotagent-plugin-rfdetr-onnx"));
    let installation_root = executable.parent().expect("target directory").to_path_buf();
    let (state_dir, cache_dir, temporary_dir) = process_directories(&root.path().join("process"));
    let hosted = Arc::new(
        HostedPlugin::start(
            manifest,
            PluginProcessConfig {
                executable,
                installation_root,
                state_dir,
                weights_dir: weights,
                model_files: BTreeMap::new(),
                cache_dir,
                temporary_dir,
                max_request_bytes: 64 * 1024 * 1024,
                max_response_bytes: 64 * 1024 * 1024,
            },
        )
        .await
        .expect("start real plugin"),
    );
    let parameters = BTreeMap::from([
        ("confidence_threshold".to_owned(), serde_json::json!(0.3)),
        ("max_detections".to_owned(), serde_json::json!(300)),
        (
            "training_dataset_version".to_owned(),
            serde_json::json!("externally-supplied-rfdetr-export"),
        ),
    ]);
    let sample = PipelineInferenceRequest {
        protocol_version: annotagent_core::PIPELINE_VISION_PROTOCOL_VERSION,
        request_id: uuid::Uuid::new_v4().to_string(),
        run_id: RunId::new(),
        image_id,
        node_id: "rfdetr".to_owned(),
        model_id: "rfdetr-detection-onnx-v1".to_owned(),
        operation: VisionCapability::ObjectDetection,
        image: Some(model_image.clone()),
        input_artifacts: vec![image_artifact.clone()],
        parameters: parameters.clone(),
        timeout_ms: Some(120_000),
    };
    let conformance = hosted.test(Some(&sample)).await.expect("conformance");
    assert!(conformance.passed, "{:?}", conformance.checks);

    let backend = Arc::new(PluginPipelineBackend::new(
        "org.annotagent.rfdetr-onnx@1.0.0/rfdetr-detection-onnx-v1",
        VisionCapability::ObjectDetection,
        Arc::clone(&hosted),
    ));
    let runner =
        ObjectDetectionSkillRunner::new(backend, "rfdetr-detection-onnx-v1", Some(model_image))
            .expect("runner");
    let node = WorkflowDraftNode {
        id: "rfdetr".to_owned(),
        node_type: "object_detection.detect".to_owned(),
        kind: WorkflowNodeKind::VisionModel,
        model_binding: Some("rfdetr-detection-onnx-v1".to_owned()),
        parameters,
        ..WorkflowDraftNode::default()
    };
    let output = runner
        .run(DagNodeContext {
            project_id: ProjectId::new(),
            run_id: RunId::new(),
            image_id,
            node: &node,
            input_artifacts: Vec::new(),
            input_pipeline_artifacts: vec![image_artifact],
            input_metadata: BTreeMap::new(),
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("detection");
    let PipelineArtifact::DetectionSet(detections) = &output.pipeline_artifacts[0] else {
        panic!("DetectionSet")
    };
    assert_eq!(detections.reference.source_node, "rfdetr");
    assert!(detections.metadata.contains_key("checkpoint_sha256"));
    assert!(detections.metadata.contains_key("training_dataset_version"));
    hosted.stop().await.expect("stop");
}

fn image_mime_type(bytes: &[u8]) -> &'static str {
    match image::guess_format(bytes).expect("recognize image format") {
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::WebP => "image/webp",
        format => panic!("unsupported test image format: {format:?}"),
    }
}
