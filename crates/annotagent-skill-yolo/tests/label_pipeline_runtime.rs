use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use annotagent_core::{
    ArtifactKind, ArtifactRef, CoreResult, ImageArtifact, ImageId, NodePort, PipelineArtifact,
    PipelineInferenceRequest, PipelineInferenceResponse, PipelineModelBackend, ProjectSchema,
    PublishedWorkflowVersion, RunId, ValidationCatalog, VisionCapability, WORKFLOW_SCHEMA_VERSION,
    WorkflowDraft, WorkflowDraftNode, WorkflowDraftStatus, WorkflowEdge, WorkflowNodeKind,
    WorkflowSnapshot,
};
use annotagent_runtime::{
    CORE_ATTACH_RESULT, CORE_CONFIDENCE_GATE, CORE_CROP, CORE_FILTER, CorePipelineRunner,
    DagExecutionRequest, DagRunStatus, PublishedDagExecutor,
};
use annotagent_skill_classification::{
    CLASSIFICATION_OPERATION, ClassificationSkillRunner, MockClassificationBackend,
};
use annotagent_skill_yolo::{MockYoloBackend, YOLO_DETECTION_OPERATION, YoloDetectionSkillRunner};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

struct CountingBackend {
    count: Arc<AtomicUsize>,
    inner: Arc<dyn PipelineModelBackend>,
}

#[async_trait]
impl PipelineModelBackend for CountingBackend {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn capability(&self) -> VisionCapability {
        self.inner.capability()
    }

    async fn infer_pipeline(
        &self,
        request: PipelineInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<PipelineInferenceResponse> {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.inner.infer_pipeline(request, cancellation).await
    }
}

fn port(id: &str, artifact_type: ArtifactKind) -> NodePort {
    NodePort {
        id: id.to_owned(),
        artifact_type,
        required: true,
        multiple: false,
    }
}

fn node(
    id: &str,
    operation: &str,
    kind: WorkflowNodeKind,
    inputs: Vec<NodePort>,
    outputs: Vec<NodePort>,
    parameters: BTreeMap<String, serde_json::Value>,
) -> WorkflowDraftNode {
    WorkflowDraftNode {
        id: id.to_owned(),
        node_type: operation.to_owned(),
        kind,
        inputs,
        outputs,
        parameters,
        ..WorkflowDraftNode::default()
    }
}

fn edge(from: &str, port: &str, to: &str, input: &str) -> WorkflowEdge {
    WorkflowEdge {
        from_node: from.to_owned(),
        from_port: port.to_owned(),
        to_node: to.to_owned(),
        to_port: input.to_owned(),
        route: None,
    }
}

fn routed_edge(from: &str, port: &str, to: &str, input: &str, route: &str) -> WorkflowEdge {
    WorkflowEdge {
        route: Some(route.to_owned()),
        ..edge(from, port, to, input)
    }
}

fn workflow(
    name: &str,
    nodes: Vec<WorkflowDraftNode>,
    edges: Vec<WorkflowEdge>,
) -> PublishedWorkflowVersion {
    let now = Utc::now();
    let draft = WorkflowDraft {
        schema_version: WORKFLOW_SCHEMA_VERSION,
        id: name.to_owned(),
        project_id: "generic".to_owned(),
        name: name.to_owned(),
        status: WorkflowDraftStatus::Published,
        nodes,
        edges,
        enabled_skills: BTreeMap::new(),
        resource_versions: BTreeMap::new(),
        allow_unvalidated_commit: false,
        label_pipeline: None,
        created_at: now,
        updated_at: now,
    };
    let snapshot = WorkflowSnapshot {
        schema_version: WORKFLOW_SCHEMA_VERSION,
        draft: Some(draft.clone()),
        enabled_skills: BTreeMap::new(),
        models: Vec::new(),
        prompt_resources: BTreeMap::new(),
    };
    let content_hash = format!(
        "{:x}",
        Sha256::digest(snapshot.content_hash_material().expect("hash material"))
    );
    PublishedWorkflowVersion {
        workflow_id: name.to_owned(),
        version: 1,
        project_id: "generic".to_owned(),
        source_draft_id: name.to_owned(),
        content_hash,
        draft,
        snapshot,
        published_at: now,
    }
}

fn image(image_id: ImageId) -> PipelineArtifact {
    PipelineArtifact::Image(ImageArtifact {
        reference: ArtifactRef {
            artifact_id: format!("image:{image_id}"),
            source_node: "image".to_owned(),
            port: "image".to_owned(),
            artifact_type: ArtifactKind::Image,
            item_id: None,
        },
        image_id,
        width: 640,
        height: 480,
        mime_type: "image/png".to_owned(),
        blob_ref: "workspace://synthetic.png".to_owned(),
    })
}

fn request(image_id: ImageId) -> DagExecutionRequest {
    DagExecutionRequest {
        project_id: annotagent_core::ProjectId::new(),
        run_id: RunId::new(),
        image_id,
        initial_artifacts: Vec::new(),
        initial_pipeline_artifacts: vec![image(image_id)],
        cancellation: CancellationToken::new(),
    }
}

fn register_core(executor: &mut PublishedDagExecutor, operations: &[&str]) {
    let runner = Arc::new(CorePipelineRunner);
    for operation in operations {
        executor
            .register_runner((*operation).to_owned(), runner.clone(), true)
            .expect("Core runner");
    }
}

#[test]
fn three_example_project_schemas_are_generic_and_valid() {
    for yaml in [
        include_str!("../../../examples/label-pipelines/whole-image-classification/project.yaml"),
        include_str!("../../../examples/label-pipelines/yolo-detection/project.yaml"),
        include_str!("../../../examples/label-pipelines/yolo-crop-classification/project.yaml"),
    ] {
        let project = ProjectSchema::from_yaml(yaml).expect("example Project Schema");
        assert!(
            project.validate(&ValidationCatalog::default()).is_empty(),
            "{}",
            project.project.name
        );
        assert!(!yaml.to_ascii_lowercase().contains("robocup"));
    }
}

#[tokio::test]
async fn whole_image_classification_commits_offline() {
    let workflow = workflow(
        "whole-image-classification",
        vec![
            node(
                "image",
                "core.image_input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("image", ArtifactKind::Image)],
                BTreeMap::new(),
            ),
            node(
                "classifier",
                CLASSIFICATION_OPERATION,
                WorkflowNodeKind::VisionModel,
                vec![port("image", ArtifactKind::Image)],
                vec![port("classifications", ArtifactKind::ClassificationSet)],
                BTreeMap::from([
                    ("labels".to_owned(), json!(["day", "night"])),
                    ("mock_label".to_owned(), json!("day")),
                ]),
            ),
            node(
                "commit",
                "core.commit",
                WorkflowNodeKind::Commit,
                vec![port("classifications", ArtifactKind::ClassificationSet)],
                Vec::new(),
                BTreeMap::new(),
            ),
        ],
        vec![
            edge("image", "image", "classifier", "image"),
            edge("classifier", "classifications", "commit", "classifications"),
        ],
    );
    let backend = Arc::new(MockClassificationBackend::new("mock-classifier"));
    let classifier =
        Arc::new(ClassificationSkillRunner::new(backend, "mock-classifier", None).expect("runner"));
    let mut executor = PublishedDagExecutor::new();
    executor
        .register_runner(CLASSIFICATION_OPERATION, classifier, false)
        .expect("classifier");
    let image_id = ImageId::new();
    let result = executor
        .execute(&workflow, &request(image_id))
        .await
        .expect("execute");
    assert_eq!(result.status, DagRunStatus::Completed);
    assert!(matches!(
        result.committed_pipeline_artifacts.as_slice(),
        [PipelineArtifact::ClassificationSet(set)] if set.classifications[0].label == "day".into()
    ));
}

#[tokio::test]
async fn detection_filter_gate_and_commit_are_typed() {
    let workflow = workflow(
        "detection",
        vec![
            node(
                "image",
                "core.image_input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("image", ArtifactKind::Image)],
                BTreeMap::new(),
            ),
            node(
                "detector",
                YOLO_DETECTION_OPERATION,
                WorkflowNodeKind::VisionModel,
                vec![port("image", ArtifactKind::Image)],
                vec![port("detections", ArtifactKind::DetectionSet)],
                BTreeMap::from([("mock_confidence".to_owned(), json!(0.95))]),
            ),
            node(
                "filter",
                CORE_FILTER,
                WorkflowNodeKind::Transform,
                vec![port("detections", ArtifactKind::DetectionSet)],
                vec![port("detections", ArtifactKind::DetectionSet)],
                BTreeMap::from([("minimum_confidence".to_owned(), json!(0.8))]),
            ),
            node(
                "gate",
                CORE_CONFIDENCE_GATE,
                WorkflowNodeKind::Gate,
                vec![port("detections", ArtifactKind::DetectionSet)],
                vec![port("detections", ArtifactKind::DetectionSet)],
                BTreeMap::from([("threshold".to_owned(), json!(0.9))]),
            ),
            node(
                "commit",
                "core.commit",
                WorkflowNodeKind::Commit,
                vec![port("detections", ArtifactKind::DetectionSet)],
                Vec::new(),
                BTreeMap::new(),
            ),
        ],
        vec![
            edge("image", "image", "detector", "image"),
            edge("detector", "detections", "filter", "detections"),
            edge("filter", "detections", "gate", "detections"),
            routed_edge("gate", "detections", "commit", "detections", "pass"),
        ],
    );
    let detector = Arc::new(
        YoloDetectionSkillRunner::new(
            Arc::new(MockYoloBackend::new("mock-detector")),
            "mock-detector",
            None,
        )
        .expect("runner"),
    );
    let mut executor = PublishedDagExecutor::new();
    executor
        .register_runner(YOLO_DETECTION_OPERATION, detector, false)
        .expect("detector");
    register_core(&mut executor, &[CORE_FILTER, CORE_CONFIDENCE_GATE]);
    let result = executor
        .execute(&workflow, &request(ImageId::new()))
        .await
        .expect("execute");
    assert_eq!(result.status, DagRunStatus::Completed);
    assert!(matches!(
        result.committed_pipeline_artifacts.as_slice(),
        [PipelineArtifact::DetectionSet(set)] if set.detections.len() == 1
    ));
    assert!(result.checkpoint.traces.iter().all(|trace| {
        trace
            .output_pipeline_artifacts
            .iter()
            .all(|artifact| !matches!(artifact, PipelineArtifact::CropSet(_)))
    }));
}

#[tokio::test]
async fn crop_classification_replay_keeps_shared_detector_checkpoint() {
    let workflow = workflow(
        "crop-classification",
        vec![
            node(
                "image",
                "core.image_input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("image", ArtifactKind::Image)],
                BTreeMap::new(),
            ),
            node(
                "detector",
                YOLO_DETECTION_OPERATION,
                WorkflowNodeKind::VisionModel,
                vec![port("image", ArtifactKind::Image)],
                vec![port("detections", ArtifactKind::DetectionSet)],
                BTreeMap::new(),
            ),
            node(
                "crop",
                CORE_CROP,
                WorkflowNodeKind::Transform,
                vec![
                    port("image", ArtifactKind::Image),
                    port("detections", ArtifactKind::DetectionSet),
                ],
                vec![port("crops", ArtifactKind::CropSet)],
                BTreeMap::from([("padding".to_owned(), json!(0.05))]),
            ),
            node(
                "classifier",
                CLASSIFICATION_OPERATION,
                WorkflowNodeKind::VisionModel,
                vec![port("crops", ArtifactKind::CropSet)],
                vec![port("classifications", ArtifactKind::ClassificationSet)],
                BTreeMap::from([
                    ("labels".to_owned(), json!(["upright", "fallen"])),
                    ("mock_label".to_owned(), json!("upright")),
                ]),
            ),
            node(
                "attach",
                CORE_ATTACH_RESULT,
                WorkflowNodeKind::CandidateMerge,
                vec![
                    port("detections", ArtifactKind::DetectionSet),
                    port("classifications", ArtifactKind::ClassificationSet),
                ],
                vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
                BTreeMap::from([
                    ("task_id".to_owned(), json!("objects")),
                    ("class_mapping".to_owned(), json!({"upright": "person"})),
                ]),
            ),
            node(
                "gate",
                CORE_CONFIDENCE_GATE,
                WorkflowNodeKind::Gate,
                vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
                vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
                BTreeMap::from([("threshold".to_owned(), json!(0.8))]),
            ),
            node(
                "review",
                "core.human_review",
                WorkflowNodeKind::HumanReview,
                vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
                vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
                BTreeMap::new(),
            ),
            node(
                "commit",
                "core.commit",
                WorkflowNodeKind::Commit,
                vec![NodePort {
                    multiple: true,
                    ..port("candidates", ArtifactKind::AnnotationCandidateSet)
                }],
                Vec::new(),
                BTreeMap::new(),
            ),
        ],
        vec![
            edge("image", "image", "detector", "image"),
            edge("image", "image", "crop", "image"),
            edge("detector", "detections", "crop", "detections"),
            edge("crop", "crops", "classifier", "crops"),
            edge("detector", "detections", "attach", "detections"),
            edge("classifier", "classifications", "attach", "classifications"),
            edge("attach", "candidates", "gate", "candidates"),
            routed_edge("gate", "candidates", "commit", "candidates", "pass"),
            routed_edge("gate", "candidates", "review", "candidates", "review"),
            edge("review", "candidates", "commit", "candidates"),
        ],
    );
    let detector_count = Arc::new(AtomicUsize::new(0));
    let classifier_count = Arc::new(AtomicUsize::new(0));
    let detector_backend = Arc::new(CountingBackend {
        count: detector_count.clone(),
        inner: Arc::new(MockYoloBackend::new("mock-detector")),
    });
    let classifier_backend = Arc::new(CountingBackend {
        count: classifier_count.clone(),
        inner: Arc::new(MockClassificationBackend::new("mock-classifier")),
    });
    let mut executor = PublishedDagExecutor::new();
    executor
        .register_runner(
            YOLO_DETECTION_OPERATION,
            Arc::new(
                YoloDetectionSkillRunner::new(detector_backend, "mock-detector", None)
                    .expect("detector"),
            ),
            false,
        )
        .expect("detector registration");
    executor
        .register_runner(
            CLASSIFICATION_OPERATION,
            Arc::new(
                ClassificationSkillRunner::new(classifier_backend, "mock-classifier", None)
                    .expect("classifier"),
            ),
            false,
        )
        .expect("classifier registration");
    register_core(
        &mut executor,
        &[CORE_CROP, CORE_ATTACH_RESULT, CORE_CONFIDENCE_GATE],
    );

    let image_id = ImageId::new();
    let request = request(image_id);
    let first = executor
        .execute(&workflow, &request)
        .await
        .expect("execute");
    assert_eq!(first.status, DagRunStatus::Completed);
    assert_eq!(detector_count.load(Ordering::SeqCst), 1);
    assert_eq!(classifier_count.load(Ordering::SeqCst), 1);
    assert_eq!(first.committed_pipeline_artifacts.len(), 1);
    let crop_trace = first
        .checkpoint
        .traces
        .iter()
        .find(|trace| trace.node_id == "crop")
        .expect("Crop trace");
    let PipelineArtifact::CropSet(crops) = &crop_trace.output_pipeline_artifacts[0] else {
        panic!("CropSet")
    };
    assert_eq!(
        crops.crops[0].parent.item_id.as_deref(),
        Some("detection-0")
    );
    let classifier_trace = first
        .checkpoint
        .traces
        .iter()
        .find(|trace| trace.node_id == "classifier")
        .expect("classifier trace");
    let PipelineArtifact::ClassificationSet(classifications) =
        &classifier_trace.output_pipeline_artifacts[0]
    else {
        panic!("ClassificationSet")
    };
    assert_eq!(
        classifications.classifications[0]
            .parent
            .as_ref()
            .and_then(|parent| parent.item_id.as_deref()),
        Some("detection-0")
    );
    serde_json::to_string(&first.checkpoint).expect("checkpoint serializes typed Artifacts");

    let replayed = executor
        .replay_from(&workflow, &request, first.checkpoint, "classifier")
        .await
        .expect("replay classifier");
    assert_eq!(replayed.status, DagRunStatus::Completed);
    assert_eq!(
        detector_count.load(Ordering::SeqCst),
        1,
        "upstream detector must not rerun"
    );
    assert_eq!(classifier_count.load(Ordering::SeqCst), 2);
    assert_eq!(
        replayed.checkpoint.node_statuses.get("detector"),
        Some(&annotagent_runtime::DagNodeStatus::Succeeded)
    );
    assert!(replayed.checkpoint.approved_review_nodes.is_empty());
    assert!(
        BTreeSet::from(["classifier", "attach", "gate", "commit"])
            .iter()
            .all(|node_id| replayed.checkpoint.node_outputs.contains_key(*node_id))
    );
}
