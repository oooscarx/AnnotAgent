use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicUsize, Ordering},
    },
    time::Duration,
};

use annotagent_core::{
    ArtifactId, ArtifactKind, ArtifactProvenance, ArtifactRef, ArtifactRole,
    ArtifactValidationState, FallbackPolicy, ImageArtifact, ImageId, ModelVersionMetadata,
    NodePort, NormalizedRect, PipelineArtifact, PublishedWorkflowVersion, RetryPolicy, RunId,
    VisionArtifact, VisionArtifactValue, VisionModelDescriptor, WORKFLOW_SCHEMA_VERSION,
    WorkflowDraft, WorkflowDraftNode, WorkflowDraftStatus, WorkflowEdge, WorkflowNodeKind,
    WorkflowSnapshot,
};
use annotagent_runtime::{
    DagExecutionRequest, DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner,
    DagNodeStatus, DagNodeUsage, DagRunStatus, PublishedDagExecutor,
};
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

fn artifact(confidence: f32, validation_state: ArtifactValidationState) -> VisionArtifact {
    VisionArtifact {
        id: ArtifactId::new(),
        image_id: ImageId::new(),
        task_id: None,
        label: None,
        role: ArtifactRole::Candidate,
        value: VisionArtifactValue::BoundingBox {
            rect: NormalizedRect::new(0.2, 0.2, 0.3, 0.3).expect("box"),
        },
        source_node: "input".to_owned(),
        confidence: Some(confidence),
        metadata: BTreeMap::new(),
        validation_state,
        provenance: ArtifactProvenance::default(),
        revision: 1,
        replaces_artifact_id: None,
        created_at: Utc::now(),
    }
}

fn port(id: &str) -> NodePort {
    NodePort {
        id: id.to_owned(),
        artifact_type: ArtifactKind::BoundingBox,
        required: true,
        multiple: true,
    }
}

fn node(
    id: &str,
    operation: &str,
    kind: WorkflowNodeKind,
    inputs: Vec<NodePort>,
    outputs: Vec<NodePort>,
) -> WorkflowDraftNode {
    WorkflowDraftNode {
        id: id.to_owned(),
        node_type: operation.to_owned(),
        kind,
        inputs,
        outputs,
        retry_policy: RetryPolicy { max_attempts: 1 },
        ..WorkflowDraftNode::default()
    }
}

fn edge(from: &str, to: &str, route: Option<&str>) -> WorkflowEdge {
    WorkflowEdge {
        from_node: from.to_owned(),
        from_port: "candidates".to_owned(),
        to_node: to.to_owned(),
        to_port: "candidates".to_owned(),
        route: route.map(str::to_owned),
    }
}

fn published(nodes: Vec<WorkflowDraftNode>, edges: Vec<WorkflowEdge>) -> PublishedWorkflowVersion {
    let now = Utc::now();
    let draft = WorkflowDraft {
        schema_version: WORKFLOW_SCHEMA_VERSION,
        id: "published-dag".to_owned(),
        project_id: "generic".to_owned(),
        name: "Generic DAG".to_owned(),
        status: WorkflowDraftStatus::Validated,
        nodes,
        edges,
        enabled_skills: BTreeMap::from([("generic".to_owned(), "1".to_owned())]),
        resource_versions: BTreeMap::from([("prompt".to_owned(), "sha256:test".to_owned())]),
        allow_unvalidated_commit: false,
        label_pipeline: None,
        created_at: now,
        updated_at: now,
    };
    let snapshot = WorkflowSnapshot {
        schema_version: WORKFLOW_SCHEMA_VERSION,
        draft: Some(draft.clone()),
        enabled_skills: draft.enabled_skills.clone(),
        models: Vec::new(),
        prompt_resources: draft.resource_versions.clone(),
    };
    let content_hash = format!(
        "{:x}",
        Sha256::digest(snapshot.content_hash_material().expect("hash material"))
    );
    PublishedWorkflowVersion {
        workflow_id: draft.id.clone(),
        version: 1,
        project_id: draft.project_id.clone(),
        source_draft_id: draft.id.clone(),
        content_hash,
        draft,
        snapshot,
        published_at: now,
    }
}

fn rehash(workflow: &mut PublishedWorkflowVersion) {
    workflow.snapshot.draft = Some(workflow.draft.clone());
    workflow.content_hash = format!(
        "{:x}",
        Sha256::digest(
            workflow
                .snapshot
                .content_hash_material()
                .expect("Workflow content hash material")
        )
    );
}

fn image_port(id: &str) -> NodePort {
    NodePort {
        id: id.to_owned(),
        artifact_type: ArtifactKind::Image,
        required: true,
        multiple: false,
    }
}

fn detection_cache_workflow() -> PublishedWorkflowVersion {
    let mut specialist = node(
        "specialist",
        "specialist_detector",
        WorkflowNodeKind::VisionModel,
        vec![image_port("image")],
        vec![image_port("image")],
    );
    specialist.model_binding = Some("specialist-v1".to_owned());
    specialist.parameters = BTreeMap::from([
        ("target_labels".to_owned(), json!(["ball"])),
        ("class_mapping".to_owned(), json!({"football": "ball"})),
    ]);
    let mut open_vocabulary = node(
        "open_vocabulary",
        "open_vocab_detector",
        WorkflowNodeKind::VisionModel,
        vec![image_port("image")],
        vec![image_port("image")],
    );
    open_vocabulary.model_binding = Some("open-vocabulary-v1".to_owned());
    open_vocabulary.parameters = BTreeMap::from([(
        "queries".to_owned(),
        json!([{"id": "ball", "text": "a football", "target_label": "ball"}]),
    )]);
    let mut gate = node(
        "gate",
        "evidence_gate",
        WorkflowNodeKind::Gate,
        vec![image_port("image")],
        vec![image_port("image")],
    );
    gate.parameters.insert("minimum_iou".to_owned(), json!(0.6));
    let mut workflow = published(
        vec![
            node(
                "input",
                "input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![image_port("image")],
            ),
            specialist,
            open_vocabulary,
            gate,
        ],
        vec![
            WorkflowEdge {
                from_node: "input".to_owned(),
                from_port: "image".to_owned(),
                to_node: "specialist".to_owned(),
                to_port: "image".to_owned(),
                route: None,
            },
            WorkflowEdge {
                from_node: "input".to_owned(),
                from_port: "image".to_owned(),
                to_node: "open_vocabulary".to_owned(),
                to_port: "image".to_owned(),
                route: None,
            },
            WorkflowEdge {
                from_node: "open_vocabulary".to_owned(),
                from_port: "image".to_owned(),
                to_node: "gate".to_owned(),
                to_port: "image".to_owned(),
                route: None,
            },
        ],
    );
    workflow.snapshot.models = vec![
        VisionModelDescriptor {
            id: "specialist-v1".to_owned(),
            model_version: "1".to_owned(),
            version: ModelVersionMetadata {
                architecture: Some("specialist".to_owned()),
                model_version: "1".to_owned(),
                checkpoint_sha256: Some("a".repeat(64)),
                training_dataset_version: Some("dataset-v1".to_owned()),
                backend_protocol_version: "1".to_owned(),
            },
            ..VisionModelDescriptor::default()
        },
        VisionModelDescriptor {
            id: "open-vocabulary-v1".to_owned(),
            model_version: "1".to_owned(),
            version: ModelVersionMetadata {
                architecture: Some("grounding".to_owned()),
                model_version: "1".to_owned(),
                checkpoint_sha256: None,
                training_dataset_version: None,
                backend_protocol_version: "1".to_owned(),
            },
            ..VisionModelDescriptor::default()
        },
    ];
    rehash(&mut workflow);
    workflow
}

struct PassthroughRunner;

#[async_trait]
impl DagNodeRunner for PassthroughRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        Ok(DagNodeOutput {
            artifacts: context.input_artifacts,
            usage: DagNodeUsage {
                input_tokens: 10,
                output_tokens: 2,
                cost: Decimal::new(1, 3),
            },
            ..DagNodeOutput::default()
        })
    }
}

struct CountingPipelineRunner {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl DagNodeRunner for CountingPipelineRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(DagNodeOutput {
            pipeline_artifacts: context.input_pipeline_artifacts,
            ..DagNodeOutput::default()
        })
    }
}

struct RefinerRunner;

#[async_trait]
impl DagNodeRunner for RefinerRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        let artifacts = context
            .input_artifacts
            .into_iter()
            .map(|artifact| VisionArtifact {
                id: ArtifactId::new(),
                role: ArtifactRole::RefinedCandidate,
                source_node: context.node.id.clone(),
                revision: artifact.revision + 1,
                replaces_artifact_id: Some(artifact.id),
                provenance: ArtifactProvenance {
                    tool: Some("deterministic_refiner".to_owned()),
                    input_artifact_ids: vec![artifact.id],
                    ..ArtifactProvenance::default()
                },
                ..artifact
            })
            .collect();
        Ok(DagNodeOutput {
            artifacts,
            usage: DagNodeUsage {
                input_tokens: 0,
                output_tokens: 0,
                cost: Decimal::new(2, 3),
            },
            ..DagNodeOutput::default()
        })
    }
}

struct ValidatorRunner;

#[async_trait]
impl DagNodeRunner for ValidatorRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        let mut artifacts = context.input_artifacts;
        for artifact in &mut artifacts {
            artifact.validation_state = ArtifactValidationState::Valid;
        }
        Ok(DagNodeOutput {
            artifacts,
            ..DagNodeOutput::default()
        })
    }
}

struct ConfidenceGate;

#[async_trait]
impl DagNodeRunner for ConfidenceGate {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        let pass = context
            .input_artifacts
            .iter()
            .all(|artifact| artifact.confidence.unwrap_or(0.0) >= 0.8);
        Ok(DagNodeOutput {
            artifacts: context.input_artifacts,
            route: Some(if pass { "pass" } else { "review" }.to_owned()),
            ..DagNodeOutput::default()
        })
    }
}

fn review_workflow() -> PublishedWorkflowVersion {
    published(
        vec![
            node(
                "input",
                "input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("candidates")],
            ),
            node(
                "detector",
                "mock_detector",
                WorkflowNodeKind::VisionModel,
                vec![port("candidates")],
                vec![port("candidates")],
            ),
            node(
                "refiner",
                "deterministic_refiner",
                WorkflowNodeKind::Refiner,
                vec![port("candidates")],
                vec![port("candidates")],
            ),
            node(
                "validator",
                "validator",
                WorkflowNodeKind::Validator,
                vec![port("candidates")],
                vec![port("candidates")],
            ),
            node(
                "gate",
                "confidence_gate",
                WorkflowNodeKind::Gate,
                vec![port("candidates")],
                vec![port("candidates")],
            ),
            node(
                "review",
                "review",
                WorkflowNodeKind::HumanReview,
                vec![port("candidates")],
                vec![port("candidates")],
            ),
            node(
                "commit",
                "commit",
                WorkflowNodeKind::Commit,
                vec![port("candidates")],
                vec![port("candidates")],
            ),
        ],
        vec![
            edge("input", "detector", None),
            edge("detector", "refiner", None),
            edge("refiner", "validator", None),
            edge("validator", "gate", None),
            edge("gate", "commit", Some("pass")),
            edge("gate", "review", Some("review")),
            edge("review", "commit", None),
        ],
    )
}

fn executor() -> PublishedDagExecutor {
    let mut executor = PublishedDagExecutor::new();
    executor
        .register_runner("mock_detector", Arc::new(PassthroughRunner), false)
        .expect("detector");
    executor
        .register_runner("deterministic_refiner", Arc::new(RefinerRunner), true)
        .expect("refiner");
    executor
        .register_runner("validator", Arc::new(ValidatorRunner), true)
        .expect("validator");
    executor
        .register_runner("confidence_gate", Arc::new(ConfidenceGate), true)
        .expect("gate");
    executor
}

#[tokio::test]
async fn published_dag_branches_suspends_resumes_caches_and_replays_trace() {
    let workflow = review_workflow();
    let executor = executor();
    let high = artifact(0.95, ArtifactValidationState::Unvalidated);
    let high_request = DagExecutionRequest {
        project_id: annotagent_core::ProjectId::new(),
        run_id: RunId::new(),
        image_id: high.image_id,
        initial_artifacts: vec![high],
        initial_pipeline_artifacts: Vec::new(),
        cancellation: CancellationToken::new(),
    };
    let passed = executor
        .execute(&workflow, &high_request)
        .await
        .expect("pass branch");
    assert_eq!(passed.status, DagRunStatus::Completed);
    assert_eq!(passed.committed.len(), 1);
    assert_eq!(
        passed.checkpoint.node_statuses["review"],
        DagNodeStatus::Skipped
    );
    assert!(passed.checkpoint.usage.input_tokens > 0);
    assert!(
        passed
            .checkpoint
            .traces
            .iter()
            .flat_map(|trace| &trace.output_envelopes)
            .all(|envelope| envelope.validate().is_ok())
    );
    assert!(passed.checkpoint.traces.iter().any(|trace| {
        trace
            .output_envelopes
            .iter()
            .any(|envelope| envelope.project_id == high_request.project_id)
    }));

    let repeated = executor
        .execute(&workflow, &high_request)
        .await
        .expect("repeat");
    assert!(
        repeated
            .checkpoint
            .traces
            .iter()
            .any(|trace| trace.node_id == "refiner" && trace.cache_hit)
    );
    assert!(repeated.checkpoint.usage.cost < passed.checkpoint.usage.cost);

    let low = artifact(0.4, ArtifactValidationState::Unvalidated);
    let low_request = DagExecutionRequest {
        project_id: annotagent_core::ProjectId::new(),
        run_id: RunId::new(),
        image_id: low.image_id,
        initial_artifacts: vec![low],
        initial_pipeline_artifacts: Vec::new(),
        cancellation: CancellationToken::new(),
    };
    let suspended = executor
        .execute(&workflow, &low_request)
        .await
        .expect("review branch");
    assert_eq!(suspended.status, DagRunStatus::AwaitingReview);
    assert_eq!(
        suspended.checkpoint.node_statuses["review"],
        DagNodeStatus::AwaitingReview
    );
    assert!(suspended.committed.is_empty());

    let encoded = serde_json::to_string(&suspended.checkpoint).expect("checkpoint JSON");
    let restored = serde_json::from_str(&encoded).expect("replayable checkpoint");
    let resumed = executor
        .resume(
            &workflow,
            &low_request,
            restored,
            BTreeSet::from(["review".to_owned()]),
        )
        .await
        .expect("resume");
    assert_eq!(resumed.status, DagRunStatus::Completed);
    assert_eq!(resumed.committed.len(), 1);
    assert_eq!(
        resumed.checkpoint.node_statuses["commit"],
        DagNodeStatus::Succeeded
    );
}

#[tokio::test]
async fn detector_cache_is_model_query_mapping_and_config_aware() {
    let specialist_calls = Arc::new(AtomicUsize::new(0));
    let open_vocabulary_calls = Arc::new(AtomicUsize::new(0));
    let mut executor = PublishedDagExecutor::new();
    executor
        .register_runner(
            "specialist_detector",
            Arc::new(CountingPipelineRunner {
                calls: specialist_calls.clone(),
            }),
            true,
        )
        .expect("specialist runner");
    executor
        .register_runner(
            "open_vocab_detector",
            Arc::new(CountingPipelineRunner {
                calls: open_vocabulary_calls.clone(),
            }),
            true,
        )
        .expect("open-vocabulary runner");
    executor
        .register_runner("evidence_gate", Arc::new(PassthroughRunner), true)
        .expect("gate runner");

    let workflow = detection_cache_workflow();
    let image_id = ImageId::new();
    let request = DagExecutionRequest {
        project_id: annotagent_core::ProjectId::new(),
        run_id: RunId::new(),
        image_id,
        initial_artifacts: Vec::new(),
        initial_pipeline_artifacts: vec![PipelineArtifact::Image(ImageArtifact {
            reference: ArtifactRef {
                artifact_id: format!("image:{image_id}"),
                source_node: "input".to_owned(),
                port: "image".to_owned(),
                artifact_type: ArtifactKind::Image,
                item_id: None,
            },
            image_id,
            width: 640,
            height: 480,
            mime_type: "image/png".to_owned(),
            blob_ref: format!("workspace://sha256/{}", "f".repeat(64)),
        })],
        cancellation: CancellationToken::new(),
    };

    let first = executor
        .execute(&workflow, &request)
        .await
        .expect("initial detector execution");
    assert_eq!(first.status, DagRunStatus::Completed);
    assert_eq!(specialist_calls.load(Ordering::SeqCst), 1);
    assert_eq!(open_vocabulary_calls.load(Ordering::SeqCst), 1);
    assert!(first.checkpoint.traces.iter().all(|trace| {
        !matches!(trace.node_id.as_str(), "specialist" | "open_vocabulary")
            || trace.cache_key.is_some()
    }));

    let repeated = executor
        .execute(&workflow, &request)
        .await
        .expect("identical detector execution");
    assert_eq!(specialist_calls.load(Ordering::SeqCst), 1);
    assert_eq!(open_vocabulary_calls.load(Ordering::SeqCst), 1);
    for detector in ["specialist", "open_vocabulary"] {
        assert!(repeated.checkpoint.traces.iter().any(|trace| {
            trace.node_id == detector && trace.status == DagNodeStatus::Cached && trace.cache_hit
        }));
    }

    let mut gate_edit = workflow.clone();
    gate_edit
        .draft
        .nodes
        .iter_mut()
        .find(|node| node.id == "gate")
        .expect("gate")
        .parameters
        .insert("minimum_iou".to_owned(), json!(0.72));
    rehash(&mut gate_edit);
    let gate_result = executor
        .execute(&gate_edit, &request)
        .await
        .expect("gate-only edit");
    assert_eq!(specialist_calls.load(Ordering::SeqCst), 1);
    assert_eq!(open_vocabulary_calls.load(Ordering::SeqCst), 1);
    assert!(
        gate_result.checkpoint.traces.iter().any(|trace| {
            trace.node_id == "specialist" && trace.status == DagNodeStatus::Cached
        })
    );
    assert!(gate_result.checkpoint.traces.iter().any(|trace| {
        trace.node_id == "open_vocabulary" && trace.status == DagNodeStatus::Cached
    }));

    let mut query_edit = gate_edit.clone();
    query_edit
        .draft
        .nodes
        .iter_mut()
        .find(|node| node.id == "open_vocabulary")
        .expect("open-vocabulary detector")
        .parameters
        .insert(
            "queries".to_owned(),
            json!([{"id": "ball", "text": "the match football", "target_label": "ball"}]),
        );
    rehash(&mut query_edit);
    let query_result = executor
        .execute(&query_edit, &request)
        .await
        .expect("query edit");
    assert_eq!(specialist_calls.load(Ordering::SeqCst), 1);
    assert_eq!(open_vocabulary_calls.load(Ordering::SeqCst), 2);
    assert!(
        query_result.checkpoint.traces.iter().any(|trace| {
            trace.node_id == "specialist" && trace.status == DagNodeStatus::Cached
        })
    );
    assert!(query_result.checkpoint.traces.iter().any(|trace| {
        trace.node_id == "open_vocabulary" && trace.status == DagNodeStatus::Succeeded
    }));

    let mut model_version_edit = query_edit.clone();
    let specialist_model = model_version_edit
        .snapshot
        .models
        .iter_mut()
        .find(|model| model.id == "specialist-v1")
        .expect("specialist model");
    specialist_model.version.model_version = "2".to_owned();
    rehash(&mut model_version_edit);
    executor
        .execute(&model_version_edit, &request)
        .await
        .expect("model version edit");
    assert_eq!(specialist_calls.load(Ordering::SeqCst), 2);
    assert_eq!(open_vocabulary_calls.load(Ordering::SeqCst), 2);

    let mut checkpoint_edit = model_version_edit.clone();
    checkpoint_edit.snapshot.models[0].version.checkpoint_sha256 = Some("b".repeat(64));
    rehash(&mut checkpoint_edit);
    executor
        .execute(&checkpoint_edit, &request)
        .await
        .expect("checkpoint edit");
    assert_eq!(specialist_calls.load(Ordering::SeqCst), 3);

    let mut protocol_edit = checkpoint_edit.clone();
    protocol_edit.snapshot.models[0]
        .version
        .backend_protocol_version = "2".to_owned();
    rehash(&mut protocol_edit);
    executor
        .execute(&protocol_edit, &request)
        .await
        .expect("protocol edit");
    assert_eq!(specialist_calls.load(Ordering::SeqCst), 4);

    let mut mapping_edit = protocol_edit;
    mapping_edit
        .draft
        .nodes
        .iter_mut()
        .find(|node| node.id == "specialist")
        .expect("specialist detector")
        .parameters
        .insert("class_mapping".to_owned(), json!({"soccer_ball": "ball"}));
    rehash(&mut mapping_edit);
    executor
        .execute(&mapping_edit, &request)
        .await
        .expect("Project Label mapping edit");
    assert_eq!(specialist_calls.load(Ordering::SeqCst), 5);
    assert_eq!(open_vocabulary_calls.load(Ordering::SeqCst), 2);
}

struct FlakyRunner {
    calls: AtomicU32,
    failures: u32,
}

#[async_trait]
impl DagNodeRunner for FlakyRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call <= self.failures {
            Err(DagNodeFailure::retryable("temporary", "retry me"))
        } else {
            Ok(DagNodeOutput {
                artifacts: context.input_artifacts,
                ..DagNodeOutput::default()
            })
        }
    }
}

#[tokio::test]
async fn retry_limit_and_fallback_are_bounded() {
    let mut flaky = node(
        "flaky",
        "flaky",
        WorkflowNodeKind::DeterministicTool,
        Vec::new(),
        Vec::new(),
    );
    flaky.retry_policy.max_attempts = 3;
    let retry_workflow = published(vec![flaky], Vec::new());
    let runner = Arc::new(FlakyRunner {
        calls: AtomicU32::new(0),
        failures: 2,
    });
    let mut executor = PublishedDagExecutor::new();
    executor
        .register_runner("flaky", runner.clone(), false)
        .expect("runner");
    let request = DagExecutionRequest {
        project_id: annotagent_core::ProjectId::new(),
        run_id: RunId::new(),
        image_id: ImageId::new(),
        initial_artifacts: Vec::new(),
        initial_pipeline_artifacts: Vec::new(),
        cancellation: CancellationToken::new(),
    };
    let result = executor
        .execute(&retry_workflow, &request)
        .await
        .expect("retry result");
    assert_eq!(result.status, DagRunStatus::Completed);
    assert_eq!(runner.calls.load(Ordering::SeqCst), 3);
    assert_eq!(result.checkpoint.traces[0].attempt_count, 3);

    let mut primary = node(
        "primary",
        "always_fails",
        WorkflowNodeKind::VisionModel,
        Vec::new(),
        Vec::new(),
    );
    primary.fallback_policy = FallbackPolicy {
        target_node: Some("fallback".to_owned()),
        on_timeout: true,
        on_error: true,
    };
    let fallback = node(
        "fallback",
        "fallback",
        WorkflowNodeKind::DeterministicTool,
        Vec::new(),
        Vec::new(),
    );
    let fallback_workflow = published(vec![primary, fallback], Vec::new());
    let mut fallback_executor = PublishedDagExecutor::new();
    fallback_executor
        .register_runner(
            "always_fails",
            Arc::new(FlakyRunner {
                calls: AtomicU32::new(0),
                failures: u32::MAX,
            }),
            false,
        )
        .expect("failure runner");
    fallback_executor
        .register_runner("fallback", Arc::new(PassthroughRunner), true)
        .expect("fallback runner");
    let recovered = fallback_executor
        .execute(&fallback_workflow, &request)
        .await
        .expect("fallback result");
    assert_eq!(recovered.status, DagRunStatus::Completed);
    assert_eq!(
        recovered.checkpoint.node_statuses["primary"],
        DagNodeStatus::FailedWithFallback
    );
    assert_eq!(
        recovered.checkpoint.node_statuses["fallback"],
        DagNodeStatus::Succeeded
    );
}

struct ParallelRunner {
    active: AtomicUsize,
    maximum: AtomicUsize,
}

#[async_trait]
impl DagNodeRunner for ParallelRunner {
    async fn run(&self, _context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(30)).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(DagNodeOutput::default())
    }
}

#[tokio::test]
async fn independent_nodes_execute_in_parallel() {
    let workflow = published(
        vec![
            node(
                "left",
                "parallel",
                WorkflowNodeKind::VisionModel,
                Vec::new(),
                Vec::new(),
            ),
            node(
                "right",
                "parallel",
                WorkflowNodeKind::VisionModel,
                Vec::new(),
                Vec::new(),
            ),
        ],
        Vec::new(),
    );
    let runner = Arc::new(ParallelRunner {
        active: AtomicUsize::new(0),
        maximum: AtomicUsize::new(0),
    });
    let mut executor = PublishedDagExecutor::new();
    executor
        .register_runner("parallel", runner.clone(), false)
        .expect("runner");
    let request = DagExecutionRequest {
        project_id: annotagent_core::ProjectId::new(),
        run_id: RunId::new(),
        image_id: ImageId::new(),
        initial_artifacts: Vec::new(),
        initial_pipeline_artifacts: Vec::new(),
        cancellation: CancellationToken::new(),
    };
    let result = executor
        .execute(&workflow, &request)
        .await
        .expect("parallel result");
    assert_eq!(result.status, DagRunStatus::Completed);
    assert_eq!(runner.maximum.load(Ordering::SeqCst), 2);
}

struct SlowRunner;

#[async_trait]
impl DagNodeRunner for SlowRunner {
    async fn run(&self, _context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        tokio::time::sleep(Duration::from_secs(5)).await;
        Ok(DagNodeOutput::default())
    }
}

#[tokio::test]
async fn cancellation_stops_running_and_pending_nodes() {
    let workflow = published(
        vec![
            node(
                "slow",
                "slow",
                WorkflowNodeKind::VisionModel,
                Vec::new(),
                Vec::new(),
            ),
            node(
                "never-started",
                "slow",
                WorkflowNodeKind::VisionModel,
                Vec::new(),
                Vec::new(),
            ),
        ],
        vec![edge("slow", "never-started", None)],
    );
    let mut executor = PublishedDagExecutor::new();
    executor
        .register_runner("slow", Arc::new(SlowRunner), false)
        .expect("runner");
    let cancellation = CancellationToken::new();
    let request = DagExecutionRequest {
        project_id: annotagent_core::ProjectId::new(),
        run_id: RunId::new(),
        image_id: ImageId::new(),
        initial_artifacts: Vec::new(),
        initial_pipeline_artifacts: Vec::new(),
        cancellation: cancellation.clone(),
    };
    let (result, ()) = tokio::join!(executor.execute(&workflow, &request), async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        cancellation.cancel();
    });
    let result = result.expect("cancel result");
    assert_eq!(result.status, DagRunStatus::Cancelled);
    assert_eq!(
        result.checkpoint.node_statuses["never-started"],
        DagNodeStatus::Cancelled
    );
}

#[tokio::test]
async fn node_timeout_is_structured_and_tampered_snapshot_is_rejected() {
    let workflow = published(
        vec![node(
            "slow",
            "slow",
            WorkflowNodeKind::VisionModel,
            Vec::new(),
            Vec::new(),
        )],
        Vec::new(),
    );
    let mut executor = PublishedDagExecutor::new().with_default_timeout(Duration::from_millis(10));
    executor
        .register_runner("slow", Arc::new(SlowRunner), false)
        .expect("runner");
    let request = DagExecutionRequest {
        project_id: annotagent_core::ProjectId::new(),
        run_id: RunId::new(),
        image_id: ImageId::new(),
        initial_artifacts: Vec::new(),
        initial_pipeline_artifacts: Vec::new(),
        cancellation: CancellationToken::new(),
    };
    let timed_out = executor
        .execute(&workflow, &request)
        .await
        .expect("timeout result");
    assert_eq!(timed_out.status, DagRunStatus::Failed);
    assert_eq!(
        timed_out.checkpoint.traces[0]
            .error
            .as_ref()
            .map(|error| error.code.as_str()),
        Some("node_timeout")
    );

    let mut tampered = workflow;
    tampered
        .snapshot
        .draft
        .as_mut()
        .expect("snapshot draft")
        .name = "tampered after publish".to_owned();
    let error = executor
        .execute(&tampered, &request)
        .await
        .expect_err("tampered content hash must fail");
    assert!(error.to_string().contains("content hash mismatch"));
}

#[tokio::test]
async fn commit_builtin_cannot_be_overridden_or_accept_unvalidated_artifacts() {
    let workflow = published(
        vec![
            node(
                "input",
                "input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("candidates")],
            ),
            node(
                "commit",
                "commit",
                WorkflowNodeKind::Commit,
                vec![port("candidates")],
                vec![port("candidates")],
            ),
        ],
        vec![edge("input", "commit", None)],
    );
    let mut executor = PublishedDagExecutor::new();
    executor
        .register_runner("commit", Arc::new(PassthroughRunner), false)
        .expect("attempted override registration");
    let candidate = artifact(0.99, ArtifactValidationState::Unvalidated);
    let request = DagExecutionRequest {
        project_id: annotagent_core::ProjectId::new(),
        run_id: RunId::new(),
        image_id: candidate.image_id,
        initial_artifacts: vec![candidate],
        initial_pipeline_artifacts: Vec::new(),
        cancellation: CancellationToken::new(),
    };
    let result = executor
        .execute(&workflow, &request)
        .await
        .expect("safe failure result");
    assert_eq!(result.status, DagRunStatus::Failed);
    let commit_trace = result
        .checkpoint
        .traces
        .iter()
        .find(|trace| trace.node_id == "commit")
        .expect("commit trace");
    assert_eq!(
        commit_trace.error.as_ref().map(|error| error.code.as_str()),
        Some("unsafe_commit_input")
    );
    assert!(result.committed.is_empty());
}
