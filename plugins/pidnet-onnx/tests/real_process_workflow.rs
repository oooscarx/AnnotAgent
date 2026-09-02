use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ImageArtifact, ImageId, ModelImage, PipelineArtifact, ProjectId,
    RunId, VisionCapability, WorkflowDraftNode, WorkflowNodeKind,
};
use annotagent_plugin_api::PluginManifest;
use annotagent_plugin_host::{
    HostedPlugin, PluginPipelineBackend, PluginProcessConfig, process_directories,
};
use annotagent_runtime::{DagNodeContext, DagNodeRunner as _};
use annotagent_skill_segmentation::{SEMANTIC_SEGMENTATION_OPERATION, SemanticSegmentationRunner};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

const MODEL_ENV: &str = "ANNOTAGENT_TEST_PIDNET_ONNX";
const IMAGE_ENV: &str = "ANNOTAGENT_TEST_PIDNET_IMAGE";

#[tokio::test]
#[ignore = "requires explicitly supplied legal PIDNet-compatible ONNX weights and a sample image"]
async fn real_pidnet_process_returns_original_size_semantic_artifact() {
    let source_model = PathBuf::from(std::env::var(MODEL_ENV).expect("model path"));
    let source_image = PathBuf::from(std::env::var(IMAGE_ENV).expect("image path"));
    let root = tempdir().expect("tempdir");
    let weights = root.path().join("weights");
    std::fs::create_dir_all(&weights).expect("weights");
    std::fs::copy(&source_model, weights.join("pidnet.onnx")).expect("copy model");
    let image_bytes = std::fs::read(source_image).expect("read image");
    let mime_type = image_mime_type(&image_bytes);
    let decoded = image::load_from_memory(&image_bytes).expect("decode image");
    let image_id = ImageId::new();
    let image_artifact = PipelineArtifact::Image(ImageArtifact {
        reference: ArtifactRef {
            artifact_id: "image:real-pidnet".to_owned(),
            source_node: "image".to_owned(),
            port: "image".to_owned(),
            artifact_type: ArtifactKind::Image,
            item_id: None,
        },
        image_id,
        width: decoded.width(),
        height: decoded.height(),
        mime_type: mime_type.to_owned(),
        blob_ref: "test://real-pidnet".to_owned(),
        parent: None,
        root_region: None,
    });
    let model_image = ModelImage {
        id: "real-pidnet-image".to_owned(),
        mime_type: mime_type.to_owned(),
        data_base64: STANDARD.encode(image_bytes),
    };
    let manifest =
        PluginManifest::from_toml(include_str!("../annotagent-plugin.toml")).expect("manifest");
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_annotagent-plugin-pidnet-onnx"));
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
                max_response_bytes: 256 * 1024 * 1024,
            },
        )
        .await
        .expect("start real plugin"),
    );
    let backend = Arc::new(PluginPipelineBackend::new(
        "org.annotagent.pidnet-onnx@1.0.0/pidnet-semantic-onnx",
        VisionCapability::SemanticSegmentation,
        Arc::clone(&hosted),
    ));
    let runner =
        SemanticSegmentationRunner::new(backend, "pidnet-semantic-onnx", Some(model_image))
            .expect("runner");
    let node = WorkflowDraftNode {
        id: "pidnet".to_owned(),
        node_type: SEMANTIC_SEGMENTATION_OPERATION.to_owned(),
        kind: WorkflowNodeKind::VisionModel,
        model_binding: Some("pidnet-semantic-onnx".to_owned()),
        parameters: BTreeMap::from([
            ("input_width".to_owned(), serde_json::json!(512)),
            ("input_height".to_owned(), serde_json::json!(256)),
        ]),
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
        .expect("semantic segmentation");
    let PipelineArtifact::SemanticMask(mask) = &output.pipeline_artifacts[0] else {
        panic!("SemanticMask")
    };
    assert_eq!(
        (mask.width, mask.height),
        (decoded.width(), decoded.height())
    );
    assert_eq!(
        mask.class_ids.len(),
        decoded.width() as usize * decoded.height() as usize
    );
    assert!(mask.metadata.contains_key("checkpoint_sha256"));
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
