use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ImageArtifact, ImageId, ModelImage, NodePort, PipelineArtifact,
    PipelineInferenceRequest, ProjectId, RunId, Skill as _, VisionCapability, WorkflowDraftNode,
};
use annotagent_plugin_api::PluginManifest;
use annotagent_plugin_host::{
    HostedPlugin, PluginPipelineBackend, PluginProcessConfig, process_directories,
};
use annotagent_runtime::{CORE_FILTER, CorePipelineRunner, DagNodeContext, DagNodeRunner as _};
use annotagent_skill_object_detection::{
    ObjectDetectionCapabilitySkill, ObjectDetectionSkillRunner,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

const MODEL_ENV: &str = "ANNOTAGENT_TEST_YOLOX_ONNX";
const IMAGE_ENV: &str = "ANNOTAGENT_TEST_YOLOX_IMAGE";

#[tokio::test]
#[ignore = "requires explicitly supplied legal YOLOX ONNX weights and a sample image"]
async fn real_yolox_process_runs_image_detector_filter_workflow() {
    let source_model = PathBuf::from(std::env::var(MODEL_ENV).expect("model path"));
    let source_image = PathBuf::from(std::env::var(IMAGE_ENV).expect("image path"));
    let root = tempdir().expect("tempdir");
    let weights = root.path().join("weights");
    std::fs::create_dir_all(&weights).expect("weights");
    std::fs::copy(&source_model, weights.join("yolox_nano.onnx")).expect("copy model");
    let image_bytes = std::fs::read(&source_image).expect("read image");
    let decoded = image::load_from_memory(&image_bytes).expect("decode image");
    let manifest =
        PluginManifest::from_toml(include_str!("../annotagent-plugin.toml")).expect("manifest");
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_annotagent-plugin-yolo-onnx"));
    let installation_root = executable.parent().expect("target directory").to_path_buf();
    let process_root = root.path().join("process");
    let (state_dir, cache_dir, temporary_dir) = process_directories(&process_root);
    let hosted = Arc::new(
        HostedPlugin::start(
            manifest.clone(),
            PluginProcessConfig {
                executable,
                installation_root,
                state_dir,
                weights_dir: weights,
                cache_dir,
                temporary_dir,
                max_request_bytes: 32 * 1024 * 1024,
                max_response_bytes: 32 * 1024 * 1024,
            },
        )
        .await
        .expect("start real plugin"),
    );
    let health = hosted.health().await.expect("health");
    assert_eq!(health.loaded_models, ["yolox-nano-coco-onnx"]);

    let image_id = ImageId::new();
    let image_artifact = PipelineArtifact::Image(ImageArtifact {
        reference: ArtifactRef {
            artifact_id: "image:real-yolox".to_owned(),
            source_node: "image".to_owned(),
            port: "image".to_owned(),
            artifact_type: ArtifactKind::Image,
            item_id: None,
        },
        image_id,
        width: decoded.width(),
        height: decoded.height(),
        mime_type: "image/jpeg".to_owned(),
        blob_ref: "test://real-yolox".to_owned(),
        parent: None,
        root_region: None,
    });
    let model_image = ModelImage {
        id: "real-yolox-image".to_owned(),
        mime_type: "image/jpeg".to_owned(),
        data_base64: STANDARD.encode(image_bytes),
    };
    let sample = PipelineInferenceRequest {
        protocol_version: annotagent_core::PIPELINE_VISION_PROTOCOL_VERSION,
        request_id: uuid::Uuid::new_v4().to_string(),
        run_id: RunId::new(),
        image_id,
        node_id: "detector".to_owned(),
        model_id: "yolox-nano-coco-onnx".to_owned(),
        operation: VisionCapability::ObjectDetection,
        image: Some(model_image.clone()),
        input_artifacts: vec![image_artifact.clone()],
        parameters: BTreeMap::from([
            ("confidence_threshold".to_owned(), serde_json::json!(0.25)),
            ("iou_threshold".to_owned(), serde_json::json!(0.45)),
            ("max_detections".to_owned(), serde_json::json!(100)),
        ]),
        timeout_ms: Some(30_000),
    };
    let conformance = hosted.test(Some(&sample)).await.expect("conformance");
    assert!(conformance.passed, "{:?}", conformance.checks);

    let backend = Arc::new(PluginPipelineBackend::new(
        "org.annotagent.yolo-onnx@1.0.0/yolox-nano-coco-onnx",
        VisionCapability::ObjectDetection,
        Arc::clone(&hosted),
    ));
    let runner =
        ObjectDetectionSkillRunner::new(backend, "yolox-nano-coco-onnx", Some(model_image))
            .expect("runner");
    let mut detector_node = ObjectDetectionCapabilitySkill::default().workflow_templates()[0]
        .nodes
        .iter()
        .find(|node| node.node_type == "object_detection.detect")
        .expect("detector node")
        .clone();
    detector_node.id = "detector".to_owned();
    detector_node.model_binding = Some("yolox-nano-coco-onnx".to_owned());
    detector_node.parameters.insert(
        "target_labels".to_owned(),
        serde_json::json!(["dog", "bicycle", "truck"]),
    );
    detector_node.parameters.insert(
        "class_mapping".to_owned(),
        serde_json::json!({"dog": "dog", "bicycle": "bicycle", "truck": "truck"}),
    );
    detector_node
        .parameters
        .insert("confidence_threshold".to_owned(), serde_json::json!(0.25));
    let run_id = RunId::new();
    let detected = runner
        .run(DagNodeContext {
            project_id: ProjectId::new(),
            run_id,
            image_id,
            node: &detector_node,
            input_artifacts: Vec::new(),
            input_pipeline_artifacts: vec![image_artifact],
            input_metadata: BTreeMap::new(),
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("real detection node");
    let PipelineArtifact::DetectionSet(detections) = &detected.pipeline_artifacts[0] else {
        panic!("expected DetectionSet");
    };
    assert!(!detections.detections.is_empty());
    assert!(detections.detections.iter().all(|detection| {
        detection.score.comparable_confidence().is_some()
            && detection.source_model_id == "yolox-nano-coco-onnx"
    }));

    let filter_node = WorkflowDraftNode {
        id: "filter".to_owned(),
        node_type: CORE_FILTER.to_owned(),
        inputs: vec![NodePort {
            id: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            required: true,
            multiple: false,
        }],
        outputs: vec![NodePort {
            id: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            required: true,
            multiple: false,
        }],
        parameters: BTreeMap::from([
            ("minimum_confidence".to_owned(), serde_json::json!(0.25)),
            (
                "class_ids".to_owned(),
                serde_json::json!(["dog", "bicycle", "truck"]),
            ),
        ]),
        ..WorkflowDraftNode::default()
    };
    let filtered = CorePipelineRunner
        .run(DagNodeContext {
            project_id: ProjectId::new(),
            run_id,
            image_id,
            node: &filter_node,
            input_artifacts: Vec::new(),
            input_pipeline_artifacts: detected.pipeline_artifacts,
            input_metadata: BTreeMap::new(),
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("core filter");
    let PipelineArtifact::DetectionSet(filtered) = &filtered.pipeline_artifacts[0] else {
        panic!("expected filtered DetectionSet");
    };
    assert!(!filtered.detections.is_empty());
    assert_eq!(filtered.reference.source_node, "filter");
    assert!(filtered.metadata.contains_key("checkpoint_sha256"));

    hosted.stop().await.expect("stop");
}
