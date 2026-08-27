use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicUsize, Ordering},
    },
    time::Duration,
};

use annotagent_core::{
    ArtifactId, ArtifactKind, ArtifactProvenance, ArtifactRole, ArtifactValidationState,
    FallbackPolicy, ImageId, NodePort, NormalizedRect, PublishedWorkflowVersion, RetryPolicy,
    RunId, VisionArtifact, VisionArtifactValue, WORKFLOW_SCHEMA_VERSION, WorkflowDraft,
    WorkflowDraftNode, WorkflowDraftStatus, WorkflowEdge, WorkflowNodeKind, WorkflowSnapshot,
};
use annotagent_runtime::{
    DagExecutionRequest, DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner,
    DagNodeStatus, DagNodeUsage, DagRunStatus, PublishedDagExecutor,
};
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
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
