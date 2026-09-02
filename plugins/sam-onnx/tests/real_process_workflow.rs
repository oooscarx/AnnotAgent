use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ArtifactValidationState, Detection, DetectionScore,
    DetectionSetArtifact, DetectionSource, ImageArtifact, ImageId, LabelId, ModelImage, NodePort,
    NormalizedRect, PipelineArtifact, PipelineInferenceRequest, ProjectId, RunId, VisionCapability,
    WorkflowDraftNode, WorkflowNodeKind,
};
use annotagent_plugin_api::PluginManifest;
use annotagent_plugin_host::{
    HostedPlugin, PluginPipelineBackend, PluginProcessConfig, process_directories,
};
use annotagent_runtime::{
    CORE_DETECTIONS_TO_BOX_PROMPTS, CORE_GEOMETRY_QUALITY_EVALUATION, CORE_MASK_TO_BBOX,
    CorePipelineRunner, DagNodeContext, DagNodeRunner as _,
};
use annotagent_skill_segmentation::{PROMPTED_SEGMENTATION_OPERATION, PromptedSegmentationRunner};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

const ENCODER_ENV: &str = "ANNOTAGENT_TEST_SAM_ENCODER_ONNX";
const DECODER_ENV: &str = "ANNOTAGENT_TEST_SAM_DECODER_ONNX";
const IMAGE_ENV: &str = "ANNOTAGENT_TEST_SAM_IMAGE";

#[tokio::test]
#[ignore = "requires explicitly supplied legal SAM encoder and decoder ONNX weights and a sample image"]
async fn real_sam_process_runs_prompt_mask_bbox_geometry_workflow() {
    let source_encoder = PathBuf::from(std::env::var(ENCODER_ENV).expect("encoder path"));
    let source_decoder = PathBuf::from(std::env::var(DECODER_ENV).expect("decoder path"));
    let source_image = PathBuf::from(std::env::var(IMAGE_ENV).expect("image path"));
    let root = tempdir().expect("tempdir");
    let weights = root.path().join("weights");
    std::fs::create_dir_all(&weights).expect("weights");
    std::fs::copy(&source_encoder, weights.join("sam_image_encoder.onnx")).expect("copy encoder");
    std::fs::copy(&source_decoder, weights.join("sam_mask_decoder.onnx")).expect("copy decoder");
    let image_bytes = std::fs::read(source_image).expect("read image");
    let mime_type = image_mime_type(&image_bytes);
    let decoded = image::load_from_memory(&image_bytes).expect("decode image");
    let image_id = ImageId::new();
    let image_artifact = PipelineArtifact::Image(ImageArtifact {
        reference: ArtifactRef {
            artifact_id: "image:real-sam".to_owned(),
            source_node: "image".to_owned(),
            port: "image".to_owned(),
            artifact_type: ArtifactKind::Image,
            item_id: None,
        },
        image_id,
        width: decoded.width(),
        height: decoded.height(),
        mime_type: mime_type.to_owned(),
        blob_ref: "test://real-sam".to_owned(),
        parent: None,
        root_region: None,
    });
    let model_image = ModelImage {
        id: "real-sam-image".to_owned(),
        mime_type: mime_type.to_owned(),
        data_base64: STANDARD.encode(image_bytes),
    };
    let run_id = RunId::new();
    let coarse = coarse_detection(image_id);
    let prompt_node = node(
        "prompts",
        CORE_DETECTIONS_TO_BOX_PROMPTS,
        WorkflowNodeKind::Transform,
        ArtifactKind::BoxPromptSet,
        "prompts",
    );
    let prompted = CorePipelineRunner
        .run(context(
            run_id,
            image_id,
            &prompt_node,
            vec![PipelineArtifact::DetectionSet(coarse)],
        ))
        .await
        .expect("box prompts");
    let prompt_artifact = prompted.pipeline_artifacts[0].clone();

    let manifest =
        PluginManifest::from_toml(include_str!("../annotagent-plugin.toml")).expect("manifest");
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_annotagent-plugin-sam-onnx"));
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
                cache_dir,
                temporary_dir,
                max_request_bytes: 64 * 1024 * 1024,
                max_response_bytes: 256 * 1024 * 1024,
            },
        )
        .await
        .expect("start real plugin"),
    );
    let sample = PipelineInferenceRequest {
        protocol_version: annotagent_core::PIPELINE_VISION_PROTOCOL_VERSION,
        request_id: uuid::Uuid::new_v4().to_string(),
        run_id,
        image_id,
        node_id: "sam".to_owned(),
        model_id: "sam-vit-b-onnx".to_owned(),
        operation: VisionCapability::PromptedSegmentation,
        image: Some(model_image.clone()),
        input_artifacts: vec![image_artifact.clone(), prompt_artifact.clone()],
        parameters: BTreeMap::from([
            ("multi_mask".to_owned(), serde_json::json!(false)),
            ("mask_threshold".to_owned(), serde_json::json!(0.0)),
        ]),
        timeout_ms: Some(120_000),
    };
    let conformance = hosted.test(Some(&sample)).await.expect("conformance");
    assert!(conformance.passed, "{:?}", conformance.checks);

    let backend = Arc::new(PluginPipelineBackend::new(
        "org.annotagent.sam-onnx@1.0.0/sam-vit-b-onnx",
        VisionCapability::PromptedSegmentation,
        Arc::clone(&hosted),
    ));
    let runner = PromptedSegmentationRunner::new(backend, "sam-vit-b-onnx", Some(model_image))
        .expect("runner");
    let segment_node = WorkflowDraftNode {
        id: "sam".to_owned(),
        node_type: PROMPTED_SEGMENTATION_OPERATION.to_owned(),
        kind: WorkflowNodeKind::VisionModel,
        model_binding: Some("sam-vit-b-onnx".to_owned()),
        parameters: sample.parameters,
        ..WorkflowDraftNode::default()
    };
    let segmented = runner
        .run(context(
            run_id,
            image_id,
            &segment_node,
            vec![image_artifact, prompt_artifact.clone()],
        ))
        .await
        .expect("segment");
    let PipelineArtifact::MaskSet(masks) = &segmented.pipeline_artifacts[0] else {
        panic!("MaskSet")
    };
    assert!(!masks.masks.is_empty());
    assert!(masks.metadata.contains_key("checkpoint_sha256"));

    let bbox_node = node(
        "mask-to-bbox",
        CORE_MASK_TO_BBOX,
        WorkflowNodeKind::Transform,
        ArtifactKind::DetectionSet,
        "detections",
    );
    let refined = CorePipelineRunner
        .run(context(
            run_id,
            image_id,
            &bbox_node,
            vec![prompt_artifact, segmented.pipeline_artifacts[0].clone()],
        ))
        .await
        .expect("mask to bbox");
    let evaluation_node = node(
        "geometry-quality",
        CORE_GEOMETRY_QUALITY_EVALUATION,
        WorkflowNodeKind::Validator,
        ArtifactKind::DetectionSet,
        "detections",
    );
    let evaluated = CorePipelineRunner
        .run(context(
            run_id,
            image_id,
            &evaluation_node,
            refined.pipeline_artifacts,
        ))
        .await
        .expect("geometry evaluation");
    assert_eq!(evaluated.metadata["semantic_score_used"], false);
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

fn coarse_detection(image_id: ImageId) -> DetectionSetArtifact {
    let artifact_id = "coarse-detections".to_owned();
    let detection = Detection::from_source(
        "target-1",
        Some("target".to_owned()),
        Some("target".to_owned()),
        Some(LabelId::from("target")),
        NormalizedRect::new(0.15, 0.15, 0.7, 0.7).expect("bbox"),
        DetectionScore::relative(0.8).expect("score"),
        DetectionSource {
            model_id: "coarse-model".to_owned(),
            capability: VisionCapability::ObjectDetection,
            artifact_id: artifact_id.clone(),
        },
    )
    .expect("detection");
    DetectionSetArtifact {
        schema_version: annotagent_core::DETECTION_ARTIFACT_SCHEMA_VERSION,
        reference: ArtifactRef {
            artifact_id,
            source_node: "coarse".to_owned(),
            port: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            item_id: None,
        },
        image_id,
        model_binding: "coarse-model".to_owned(),
        validation_state: ArtifactValidationState::Unvalidated,
        detections: vec![detection],
        metadata: BTreeMap::new(),
    }
}

fn node(
    id: &str,
    node_type: &str,
    kind: WorkflowNodeKind,
    artifact_type: ArtifactKind,
    port: &str,
) -> WorkflowDraftNode {
    WorkflowDraftNode {
        id: id.to_owned(),
        node_type: node_type.to_owned(),
        kind,
        outputs: vec![NodePort {
            id: port.to_owned(),
            artifact_type,
            required: true,
            multiple: false,
        }],
        ..WorkflowDraftNode::default()
    }
}

fn context(
    run_id: RunId,
    image_id: ImageId,
    node: &WorkflowDraftNode,
    artifacts: Vec<PipelineArtifact>,
) -> DagNodeContext<'_> {
    DagNodeContext {
        project_id: ProjectId::new(),
        run_id,
        image_id,
        node,
        input_artifacts: Vec::new(),
        input_pipeline_artifacts: artifacts,
        input_metadata: BTreeMap::new(),
        cancellation: CancellationToken::new(),
    }
}
