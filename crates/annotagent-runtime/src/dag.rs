//! Generic execution of immutable, published Workflow snapshots.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use annotagent_core::{
    ArtifactEnvelope, ArtifactProvenance, ArtifactValidationState, ImageId, PipelineArtifact,
    ProjectId, PublishedWorkflowVersion, RunId, VisionArtifact, WorkflowDraft, WorkflowDraftNode,
    WorkflowEdge, WorkflowNodeKind,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::join_all;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Error)]
pub enum DagRuntimeError {
    #[error("published Workflow snapshot is invalid: {0}")]
    InvalidSnapshot(String),
    #[error("DAG execution cannot make progress: {0}")]
    Deadlock(String),
    #[error("DAG state lock poisoned")]
    Poisoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DagNodeStatus {
    Pending,
    Running,
    Succeeded,
    Cached,
    AwaitingReview,
    Skipped,
    FailedWithFallback,
    Failed,
    Cancelled,
}

impl DagNodeStatus {
    const fn terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded
                | Self::Cached
                | Self::Skipped
                | Self::FailedWithFallback
                | Self::Failed
                | Self::Cancelled
        )
    }

    const fn provides_output(self) -> bool {
        matches!(self, Self::Succeeded | Self::Cached)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DagRunStatus {
    Completed,
    AwaitingReview,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DagNodeUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: Decimal,
}

impl std::ops::AddAssign<&Self> for DagNodeUsage {
    fn add_assign(&mut self, other: &Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.cost += other.cost;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DagNodeOutput {
    #[serde(default)]
    pub artifacts: Vec<VisionArtifact>,
    #[serde(default)]
    pub pipeline_artifacts: Vec<PipelineArtifact>,
    /// Gate route selected by this node, for example `pass` or `review`.
    pub route: Option<String>,
    #[serde(default)]
    pub usage: DagNodeUsage,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagNodeFailure {
    pub code: String,
    pub summary: String,
    #[serde(default = "default_retryable")]
    pub retryable: bool,
}

const fn default_retryable() -> bool {
    true
}

impl DagNodeFailure {
    #[must_use]
    pub fn retryable(code: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            summary: summary.into(),
            retryable: true,
        }
    }

    #[must_use]
    pub fn terminal(code: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            summary: summary.into(),
            retryable: false,
        }
    }
}

pub struct DagNodeContext<'a> {
    pub project_id: ProjectId,
    pub run_id: RunId,
    pub image_id: ImageId,
    pub node: &'a WorkflowDraftNode,
    pub input_artifacts: Vec<VisionArtifact>,
    pub input_pipeline_artifacts: Vec<PipelineArtifact>,
    /// Structured metadata emitted by active upstream nodes, keyed by source node id. This lets
    /// generic gates consume validator facts without turning them into annotation Artifacts.
    pub input_metadata: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    pub cancellation: CancellationToken,
}

#[async_trait]
pub trait DagNodeRunner: Send + Sync {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure>;
}

struct RegisteredRunner {
    runner: Arc<dyn DagNodeRunner>,
    deterministic: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DagNodeTrace {
    pub node_id: String,
    pub operation: String,
    pub status: DagNodeStatus,
    pub attempt_count: u32,
    pub cache_key: Option<String>,
    pub cache_hit: bool,
    pub input_artifacts: Vec<VisionArtifact>,
    pub output_artifacts: Vec<VisionArtifact>,
    #[serde(default)]
    pub input_pipeline_artifacts: Vec<PipelineArtifact>,
    #[serde(default)]
    pub output_pipeline_artifacts: Vec<PipelineArtifact>,
    #[serde(default)]
    pub input_envelopes: Vec<ArtifactEnvelope>,
    #[serde(default)]
    pub output_envelopes: Vec<ArtifactEnvelope>,
    pub route: Option<String>,
    pub usage: DagNodeUsage,
    pub error: Option<DagNodeFailure>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DagCheckpoint {
    pub workflow_content_hash: String,
    pub node_statuses: BTreeMap<String, DagNodeStatus>,
    pub node_outputs: BTreeMap<String, DagNodeOutput>,
    pub traces: Vec<DagNodeTrace>,
    pub activated_fallbacks: BTreeSet<String>,
    pub approved_review_nodes: BTreeSet<String>,
    pub usage: DagNodeUsage,
}

impl DagCheckpoint {
    fn new(workflow: &PublishedWorkflowVersion) -> Self {
        Self {
            workflow_content_hash: workflow.content_hash.clone(),
            node_statuses: workflow
                .draft
                .nodes
                .iter()
                .map(|node| (node.id.clone(), DagNodeStatus::Pending))
                .collect(),
            node_outputs: BTreeMap::new(),
            traces: Vec::new(),
            activated_fallbacks: BTreeSet::new(),
            approved_review_nodes: BTreeSet::new(),
            usage: DagNodeUsage::default(),
        }
    }
}

pub struct DagExecutionRequest {
    pub project_id: ProjectId,
    pub run_id: RunId,
    pub image_id: ImageId,
    pub initial_artifacts: Vec<VisionArtifact>,
    pub initial_pipeline_artifacts: Vec<PipelineArtifact>,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DagRunResult {
    pub status: DagRunStatus,
    pub checkpoint: DagCheckpoint,
    pub committed: Vec<VisionArtifact>,
    pub committed_pipeline_artifacts: Vec<PipelineArtifact>,
}

#[derive(Default)]
pub struct PublishedDagExecutor {
    runners: BTreeMap<String, RegisteredRunner>,
    cache: Mutex<BTreeMap<String, DagNodeOutput>>,
    default_timeout: Duration,
}

impl PublishedDagExecutor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            runners: BTreeMap::new(),
            cache: Mutex::new(BTreeMap::new()),
            default_timeout: Duration::from_secs(300),
        }
    }

    #[must_use]
    pub const fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    pub fn register_runner(
        &mut self,
        operation: impl Into<String>,
        runner: Arc<dyn DagNodeRunner>,
        deterministic: bool,
    ) -> Result<(), DagRuntimeError> {
        let operation = operation.into();
        if operation.trim().is_empty() {
            return Err(DagRuntimeError::InvalidSnapshot(
                "runner operation id cannot be empty".to_owned(),
            ));
        }
        if self
            .runners
            .insert(
                operation.clone(),
                RegisteredRunner {
                    runner,
                    deterministic,
                },
            )
            .is_some()
        {
            return Err(DagRuntimeError::InvalidSnapshot(format!(
                "runner {operation:?} is already registered"
            )));
        }
        Ok(())
    }

    pub async fn execute(
        &self,
        workflow: &PublishedWorkflowVersion,
        request: &DagExecutionRequest,
    ) -> Result<DagRunResult, DagRuntimeError> {
        self.run(
            workflow,
            request,
            DagCheckpoint::new(workflow),
            &BTreeSet::new(),
        )
        .await
    }

    pub async fn resume(
        &self,
        workflow: &PublishedWorkflowVersion,
        request: &DagExecutionRequest,
        checkpoint: DagCheckpoint,
        approved_review_nodes: BTreeSet<String>,
    ) -> Result<DagRunResult, DagRuntimeError> {
        if checkpoint.workflow_content_hash != workflow.content_hash {
            return Err(DagRuntimeError::InvalidSnapshot(
                "checkpoint belongs to a different Workflow content hash".to_owned(),
            ));
        }
        self.run(workflow, request, checkpoint, &approved_review_nodes)
            .await
    }

    /// Replay one node and every downstream consumer while preserving completed upstream outputs.
    /// Replaying a classifier therefore does not execute its shared detector or Crop ancestors.
    pub async fn replay_from(
        &self,
        workflow: &PublishedWorkflowVersion,
        request: &DagExecutionRequest,
        mut checkpoint: DagCheckpoint,
        node_id: &str,
    ) -> Result<DagRunResult, DagRuntimeError> {
        if checkpoint.workflow_content_hash != workflow.content_hash {
            return Err(DagRuntimeError::InvalidSnapshot(
                "checkpoint belongs to a different Workflow content hash".to_owned(),
            ));
        }
        let draft = snapshot_draft(workflow)?;
        if !draft.nodes.iter().any(|node| node.id == node_id) {
            return Err(DagRuntimeError::InvalidSnapshot(format!(
                "replay references unknown node {node_id:?}"
            )));
        }
        for id in descendants_including(draft, node_id) {
            checkpoint
                .node_statuses
                .insert(id.clone(), DagNodeStatus::Pending);
            checkpoint.node_outputs.remove(&id);
            checkpoint.approved_review_nodes.remove(&id);
            checkpoint.activated_fallbacks.remove(&id);
        }
        self.run(workflow, request, checkpoint, &BTreeSet::new())
            .await
    }

    async fn run(
        &self,
        workflow: &PublishedWorkflowVersion,
        request: &DagExecutionRequest,
        mut checkpoint: DagCheckpoint,
        approvals: &BTreeSet<String>,
    ) -> Result<DagRunResult, DagRuntimeError> {
        let draft = snapshot_draft(workflow)?;
        verify_snapshot_hash(workflow)?;
        for node_id in approvals {
            let Some(node) = draft.nodes.iter().find(|node| &node.id == node_id) else {
                return Err(DagRuntimeError::InvalidSnapshot(format!(
                    "approval references unknown node {node_id:?}"
                )));
            };
            if node.kind != WorkflowNodeKind::HumanReview {
                return Err(DagRuntimeError::InvalidSnapshot(format!(
                    "node {node_id:?} is not a HumanReview node"
                )));
            }
            checkpoint.approved_review_nodes.insert(node_id.clone());
            if checkpoint.node_statuses.get(node_id) == Some(&DagNodeStatus::AwaitingReview) {
                checkpoint
                    .node_statuses
                    .insert(node_id.clone(), DagNodeStatus::Pending);
            }
        }

        loop {
            if request.cancellation.is_cancelled() {
                cancel_pending(&mut checkpoint);
                return Ok(result(DagRunStatus::Cancelled, checkpoint, draft));
            }

            let mut ready = Vec::new();
            let mut skipped = Vec::new();
            for node in &draft.nodes {
                if checkpoint.node_statuses.get(&node.id) != Some(&DagNodeStatus::Pending) {
                    continue;
                }
                if held_fallback_target(draft, node, &checkpoint) {
                    continue;
                }
                match readiness(draft, node, &checkpoint) {
                    Readiness::Ready(inputs) => ready.push((
                        node,
                        inputs,
                        checkpoint.approved_review_nodes.contains(&node.id),
                    )),
                    Readiness::Skip => skipped.push(node),
                    Readiness::Wait => {}
                }
            }
            let had_skipped = !skipped.is_empty();
            for node in skipped {
                checkpoint
                    .node_statuses
                    .insert(node.id.clone(), DagNodeStatus::Skipped);
            }
            if ready.is_empty() {
                if had_skipped {
                    continue;
                }
                if checkpoint
                    .node_statuses
                    .values()
                    .any(|status| *status == DagNodeStatus::AwaitingReview)
                {
                    return Ok(result(DagRunStatus::AwaitingReview, checkpoint, draft));
                }
                if checkpoint
                    .node_statuses
                    .values()
                    .any(|status| *status == DagNodeStatus::Pending)
                {
                    return Err(DagRuntimeError::Deadlock(
                        "pending nodes have no satisfiable input path".to_owned(),
                    ));
                }
                let status = if checkpoint
                    .node_statuses
                    .values()
                    .any(|status| *status == DagNodeStatus::Failed)
                {
                    DagRunStatus::Failed
                } else if checkpoint
                    .node_statuses
                    .values()
                    .any(|status| *status == DagNodeStatus::Cancelled)
                {
                    DagRunStatus::Cancelled
                } else {
                    DagRunStatus::Completed
                };
                return Ok(result(status, checkpoint, draft));
            }

            for (node, _, _) in &ready {
                checkpoint
                    .node_statuses
                    .insert(node.id.clone(), DagNodeStatus::Running);
            }
            let outcomes = join_all(
                ready
                    .into_iter()
                    .map(|(node, inputs, approved)| async move {
                        (
                            node.id.clone(),
                            self.execute_node(workflow, request, node, inputs, approved)
                                .await,
                        )
                    }),
            )
            .await;
            for (node_id, execution) in outcomes {
                let node = draft
                    .nodes
                    .iter()
                    .find(|node| node.id == node_id)
                    .ok_or_else(|| {
                        DagRuntimeError::InvalidSnapshot("node disappeared".to_owned())
                    })?;
                match execution {
                    NodeExecution::Completed { output, trace } => {
                        checkpoint.usage += &output.usage;
                        checkpoint.node_outputs.insert(node_id.clone(), output);
                        checkpoint.node_statuses.insert(node_id, trace.status);
                        checkpoint.traces.push(trace);
                    }
                    NodeExecution::Suspended(trace) => {
                        checkpoint
                            .node_statuses
                            .insert(node_id, DagNodeStatus::AwaitingReview);
                        checkpoint.traces.push(trace);
                    }
                    NodeExecution::Failed(trace) => {
                        if let Some(fallback) = fallback_for_failure(node, trace.error.as_ref()) {
                            checkpoint.activated_fallbacks.insert(fallback.to_owned());
                            checkpoint
                                .node_statuses
                                .insert(node_id, DagNodeStatus::FailedWithFallback);
                        } else {
                            checkpoint
                                .node_statuses
                                .insert(node_id, DagNodeStatus::Failed);
                        }
                        checkpoint.traces.push(trace);
                    }
                    NodeExecution::Cancelled(trace) => {
                        checkpoint
                            .node_statuses
                            .insert(node_id, DagNodeStatus::Cancelled);
                        checkpoint.traces.push(trace);
                        cancel_pending(&mut checkpoint);
                        return Ok(result(DagRunStatus::Cancelled, checkpoint, draft));
                    }
                }
            }
        }
    }

    async fn execute_node(
        &self,
        workflow: &PublishedWorkflowVersion,
        request: &DagExecutionRequest,
        node: &WorkflowDraftNode,
        mut inputs: DagInputs,
        approved_review: bool,
    ) -> NodeExecution {
        inputs.artifacts.sort_by_key(|artifact| artifact.id);
        inputs
            .pipeline_artifacts
            .sort_by(|left, right| left.reference().cmp(right.reference()));
        let started_at = Utc::now();
        if node.kind == WorkflowNodeKind::HumanReview && !approved_review {
            return NodeExecution::Suspended(trace(
                request,
                node,
                DagNodeStatus::AwaitingReview,
                0,
                None,
                false,
                inputs,
                DagNodeOutput::default(),
                None,
                started_at,
            ));
        }

        let fixed_builtin = matches!(
            node.kind,
            WorkflowNodeKind::ImageInput | WorkflowNodeKind::HumanReview | WorkflowNodeKind::Commit
        );
        let registration = (!fixed_builtin)
            .then(|| self.runners.get(&node.node_type))
            .flatten();
        let built_in = fixed_builtin
            || (node.kind == WorkflowNodeKind::CandidateMerge && registration.is_none());
        if registration.is_none() && !built_in {
            let error = DagNodeFailure::terminal(
                "runner_not_registered",
                format!("operation {:?} has no registered runner", node.node_type),
            );
            return NodeExecution::Failed(trace(
                request,
                node,
                DagNodeStatus::Failed,
                1,
                None,
                false,
                inputs,
                DagNodeOutput::default(),
                Some(error),
                started_at,
            ));
        }

        let deterministic = registration.is_some_and(|runner| runner.deterministic)
            || matches!(
                node.kind,
                WorkflowNodeKind::DeterministicTool
                    | WorkflowNodeKind::Transform
                    | WorkflowNodeKind::CandidateMerge
                    | WorkflowNodeKind::Validator
                    | WorkflowNodeKind::Refiner
                    | WorkflowNodeKind::Commit
            );
        let cache_key = deterministic
            .then(|| node_cache_key(workflow, node, &inputs))
            .transpose()
            .ok()
            .flatten();
        if let Some(key) = cache_key.as_ref() {
            match self.cache.lock() {
                Ok(cache) => {
                    if let Some(mut output) = cache.get(key).cloned() {
                        output.usage = DagNodeUsage::default();
                        return NodeExecution::Completed {
                            trace: trace(
                                request,
                                node,
                                DagNodeStatus::Cached,
                                0,
                                Some(key.clone()),
                                true,
                                inputs,
                                output.clone(),
                                None,
                                started_at,
                            ),
                            output,
                        };
                    }
                }
                Err(_) => {
                    return NodeExecution::Failed(trace(
                        request,
                        node,
                        DagNodeStatus::Failed,
                        0,
                        Some(key.clone()),
                        false,
                        inputs,
                        DagNodeOutput::default(),
                        Some(DagNodeFailure::terminal(
                            "cache_poisoned",
                            "cache lock poisoned",
                        )),
                        started_at,
                    ));
                }
            }
        }

        let attempts = node
            .retry_policy
            .max_attempts
            .max(node.max_retries.saturating_add(1))
            .max(1);
        let timeout = Duration::from_secs(
            node.resources
                .timeout_seconds
                .unwrap_or(self.default_timeout.as_secs()),
        );
        let mut last_error = None;
        let mut attempts_made = 0;
        for attempt in 1..=attempts {
            attempts_made = attempt;
            if request.cancellation.is_cancelled() {
                return NodeExecution::Cancelled(trace(
                    request,
                    node,
                    DagNodeStatus::Cancelled,
                    attempt.saturating_sub(1),
                    cache_key,
                    false,
                    inputs,
                    DagNodeOutput::default(),
                    None,
                    started_at,
                ));
            }
            let execution = async {
                if let Some(registration) = registration {
                    registration
                        .runner
                        .run(DagNodeContext {
                            project_id: request.project_id,
                            run_id: request.run_id,
                            image_id: request.image_id,
                            node,
                            input_artifacts: inputs.artifacts.clone(),
                            input_pipeline_artifacts: inputs.pipeline_artifacts.clone(),
                            input_metadata: inputs.metadata.clone(),
                            cancellation: request.cancellation.clone(),
                        })
                        .await
                } else {
                    built_in_output(node, request, inputs.clone())
                }
            };
            let outcome = tokio::select! {
                () = request.cancellation.cancelled() => {
                    return NodeExecution::Cancelled(trace(
                        request,
                        node,
                        DagNodeStatus::Cancelled,
                        attempt.saturating_sub(1),
                        cache_key,
                        false,
                        inputs,
                        DagNodeOutput::default(),
                        None,
                        started_at,
                    ));
                }
                outcome = tokio::time::timeout(timeout, execution) => outcome,
            };
            match outcome {
                Ok(Ok(output)) => {
                    if let Some(key) = cache_key.as_ref() {
                        if let Ok(mut cache) = self.cache.lock() {
                            cache.insert(key.clone(), output.clone());
                        }
                    }
                    return NodeExecution::Completed {
                        trace: trace(
                            request,
                            node,
                            DagNodeStatus::Succeeded,
                            attempt,
                            cache_key,
                            false,
                            inputs,
                            output.clone(),
                            None,
                            started_at,
                        ),
                        output,
                    };
                }
                Ok(Err(error)) => {
                    let retryable = error.retryable;
                    last_error = Some(error);
                    if !retryable {
                        break;
                    }
                }
                Err(_) => {
                    last_error = Some(DagNodeFailure::retryable(
                        "node_timeout",
                        format!("node exceeded {} ms", timeout.as_millis()),
                    ));
                }
            }
        }
        NodeExecution::Failed(trace(
            request,
            node,
            DagNodeStatus::Failed,
            attempts_made,
            cache_key,
            false,
            inputs,
            DagNodeOutput::default(),
            last_error,
            started_at,
        ))
    }
}

fn built_in_output(
    node: &WorkflowDraftNode,
    request: &DagExecutionRequest,
    mut inputs: DagInputs,
) -> Result<DagNodeOutput, DagNodeFailure> {
    match node.kind {
        WorkflowNodeKind::ImageInput => Ok(DagNodeOutput {
            artifacts: request.initial_artifacts.clone(),
            pipeline_artifacts: request.initial_pipeline_artifacts.clone(),
            ..DagNodeOutput::default()
        }),
        WorkflowNodeKind::HumanReview => {
            for artifact in &mut inputs.artifacts {
                artifact.validation_state = ArtifactValidationState::Valid;
            }
            for artifact in &mut inputs.pipeline_artifacts {
                match artifact {
                    PipelineArtifact::DetectionSet(detections) => {
                        detections.validation_state = ArtifactValidationState::Valid;
                    }
                    PipelineArtifact::ClassificationSet(classifications) => {
                        classifications.validation_state = ArtifactValidationState::Valid;
                    }
                    PipelineArtifact::CandidateClusterSet(candidates) => {
                        candidates.validation_state = ArtifactValidationState::Valid;
                    }
                    PipelineArtifact::MaskSet(masks) => {
                        masks.validation_state = ArtifactValidationState::Valid;
                    }
                    PipelineArtifact::SemanticMask(mask) => {
                        mask.validation_state = ArtifactValidationState::Valid;
                    }
                    PipelineArtifact::AnnotationCandidateSet(candidates) => {
                        for candidate in &mut candidates.candidates {
                            candidate.validation_state = Some(ArtifactValidationState::Valid);
                        }
                    }
                    PipelineArtifact::Image(_)
                    | PipelineArtifact::BoxPromptSet(_)
                    | PipelineArtifact::PointPromptSet(_)
                    | PipelineArtifact::PolygonSet(_)
                    | PipelineArtifact::CropSet(_) => {}
                }
            }
            Ok(DagNodeOutput {
                artifacts: inputs.artifacts,
                pipeline_artifacts: inputs.pipeline_artifacts,
                metadata: BTreeMap::from([("human_approved".to_owned(), serde_json::json!(true))]),
                ..DagNodeOutput::default()
            })
        }
        WorkflowNodeKind::Commit => {
            if inputs
                .artifacts
                .iter()
                .any(|artifact| artifact.validation_state != ArtifactValidationState::Valid)
                || inputs
                    .pipeline_artifacts
                    .iter()
                    .any(|artifact| match artifact {
                        PipelineArtifact::DetectionSet(detections) => {
                            detections.validation_state != ArtifactValidationState::Valid
                        }
                        PipelineArtifact::ClassificationSet(classifications) => {
                            classifications.validation_state != ArtifactValidationState::Valid
                        }
                        PipelineArtifact::CandidateClusterSet(candidates) => {
                            candidates.validation_state != ArtifactValidationState::Valid
                        }
                        PipelineArtifact::MaskSet(masks) => {
                            masks.validation_state != ArtifactValidationState::Valid
                        }
                        PipelineArtifact::SemanticMask(mask) => {
                            mask.validation_state != ArtifactValidationState::Valid
                        }
                        PipelineArtifact::AnnotationCandidateSet(candidates) => {
                            candidates.candidates.iter().any(|candidate| {
                                candidate.validation_state != Some(ArtifactValidationState::Valid)
                            })
                        }
                        // Images and CropSets are supporting evidence/artifact-preview inputs, not
                        // annotation-shaped values. They may share the terminal Commit path so the
                        // graph is observable, but Commit never turns them into annotations.
                        PipelineArtifact::Image(_)
                        | PipelineArtifact::BoxPromptSet(_)
                        | PipelineArtifact::PointPromptSet(_)
                        | PipelineArtifact::PolygonSet(_)
                        | PipelineArtifact::CropSet(_) => false,
                    })
            {
                return Err(DagNodeFailure::terminal(
                    "unsafe_commit_input",
                    "Commit received an Artifact that was not validated or reviewed",
                ));
            }
            Ok(DagNodeOutput {
                artifacts: inputs.artifacts,
                pipeline_artifacts: inputs.pipeline_artifacts,
                metadata: BTreeMap::from([("committed".to_owned(), serde_json::json!(true))]),
                ..DagNodeOutput::default()
            })
        }
        WorkflowNodeKind::CandidateMerge => Ok(DagNodeOutput {
            artifacts: inputs.artifacts,
            pipeline_artifacts: inputs.pipeline_artifacts,
            ..DagNodeOutput::default()
        }),
        _ => Err(DagNodeFailure::terminal(
            "runner_not_registered",
            format!("operation {:?} has no registered runner", node.node_type),
        )),
    }
}

enum NodeExecution {
    Completed {
        output: DagNodeOutput,
        trace: DagNodeTrace,
    },
    Suspended(DagNodeTrace),
    Failed(DagNodeTrace),
    Cancelled(DagNodeTrace),
}

#[derive(Debug, Clone, Default)]
struct DagInputs {
    artifacts: Vec<VisionArtifact>,
    pipeline_artifacts: Vec<PipelineArtifact>,
    metadata: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
}

#[derive(Debug)]
enum Readiness {
    Ready(DagInputs),
    Wait,
    Skip,
}

fn readiness(
    draft: &WorkflowDraft,
    node: &WorkflowDraftNode,
    checkpoint: &DagCheckpoint,
) -> Readiness {
    let incoming = draft
        .edges
        .iter()
        .filter(|edge| edge.to_node == node.id)
        .collect::<Vec<_>>();
    if incoming.is_empty() {
        if node.inputs.iter().any(|port| port.required) {
            return Readiness::Skip;
        }
        return Readiness::Ready(DagInputs::default());
    }
    if incoming.iter().any(|edge| {
        !checkpoint
            .node_statuses
            .get(&edge.from_node)
            .is_some_and(|status| status.terminal())
    }) {
        return Readiness::Wait;
    }

    let required_ports = if node.inputs.is_empty() {
        vec!["__dependency".to_owned()]
    } else {
        node.inputs
            .iter()
            .filter(|port| port.required)
            .map(|port| port.id.clone())
            .collect::<Vec<_>>()
    };
    for port in required_ports {
        let candidates = if port == "__dependency" {
            incoming.clone()
        } else {
            incoming
                .iter()
                .copied()
                .filter(|edge| edge.to_port == port)
                .collect()
        };
        if candidates.is_empty() || !candidates.iter().any(|edge| edge_active(edge, checkpoint)) {
            return Readiness::Skip;
        }
    }
    let mut artifacts = incoming
        .iter()
        .filter(|edge| edge_active(edge, checkpoint))
        .flat_map(|edge| {
            checkpoint
                .node_outputs
                .get(&edge.from_node)
                .into_iter()
                .flat_map(|output| output.artifacts.clone())
        })
        .collect::<Vec<_>>();
    let mut ids = BTreeSet::new();
    artifacts.retain(|artifact| ids.insert(artifact.id));
    let mut pipeline_artifacts = incoming
        .iter()
        .filter(|edge| edge_active(edge, checkpoint))
        .flat_map(|edge| {
            checkpoint
                .node_outputs
                .get(&edge.from_node)
                .into_iter()
                .flat_map(|output| {
                    output
                        .pipeline_artifacts
                        .iter()
                        .filter(|artifact| artifact.reference().port == edge.from_port)
                        .cloned()
                })
        })
        .collect::<Vec<_>>();
    let mut pipeline_ids = BTreeSet::new();
    pipeline_artifacts.retain(|artifact| {
        pipeline_ids.insert((
            artifact.reference().artifact_id.clone(),
            artifact.reference().item_id.clone(),
        ))
    });
    let metadata = incoming
        .iter()
        .filter(|edge| edge_active(edge, checkpoint))
        .filter_map(|edge| {
            checkpoint
                .node_outputs
                .get(&edge.from_node)
                .map(|output| (edge.from_node.clone(), output.metadata.clone()))
        })
        .collect();
    Readiness::Ready(DagInputs {
        artifacts,
        pipeline_artifacts,
        metadata,
    })
}

fn edge_active(edge: &WorkflowEdge, checkpoint: &DagCheckpoint) -> bool {
    if !checkpoint
        .node_statuses
        .get(&edge.from_node)
        .is_some_and(|status| status.provides_output())
    {
        return false;
    }
    let selected_route = checkpoint
        .node_outputs
        .get(&edge.from_node)
        .and_then(|output| output.route.as_deref());
    edge.route
        .as_deref()
        .is_none_or(|route| selected_route == Some(route))
}

fn held_fallback_target(
    draft: &WorkflowDraft,
    node: &WorkflowDraftNode,
    checkpoint: &DagCheckpoint,
) -> bool {
    let is_fallback_target = draft
        .nodes
        .iter()
        .filter_map(effective_fallback)
        .any(|target| target == node.id);
    is_fallback_target
        && !checkpoint.activated_fallbacks.contains(&node.id)
        && !draft.edges.iter().any(|edge| edge.to_node == node.id)
}

fn effective_fallback(node: &WorkflowDraftNode) -> Option<&str> {
    node.fallback_policy
        .target_node
        .as_deref()
        .or(node.fallback.as_deref())
}

fn descendants_including(draft: &WorkflowDraft, node_id: &str) -> BTreeSet<String> {
    let mut descendants = BTreeSet::new();
    let mut pending = vec![node_id.to_owned()];
    while let Some(current) = pending.pop() {
        if !descendants.insert(current.clone()) {
            continue;
        }
        pending.extend(
            draft
                .edges
                .iter()
                .filter(|edge| edge.from_node == current)
                .map(|edge| edge.to_node.clone()),
        );
        pending.extend(
            draft
                .nodes
                .iter()
                .filter(|node| node.depends_on.contains(&current))
                .map(|node| node.id.clone()),
        );
    }
    descendants
}

fn fallback_for_failure<'a>(
    node: &'a WorkflowDraftNode,
    failure: Option<&DagNodeFailure>,
) -> Option<&'a str> {
    if let Some(target) = node.fallback_policy.target_node.as_deref() {
        let timed_out = failure.is_some_and(|failure| failure.code == "node_timeout");
        return ((timed_out && node.fallback_policy.on_timeout)
            || (!timed_out && node.fallback_policy.on_error))
            .then_some(target);
    }
    node.fallback.as_deref()
}

fn snapshot_draft(workflow: &PublishedWorkflowVersion) -> Result<&WorkflowDraft, DagRuntimeError> {
    let draft = workflow.snapshot.draft.as_ref().unwrap_or(&workflow.draft);
    if draft.status == annotagent_core::WorkflowDraftStatus::Editing
        || draft.status == annotagent_core::WorkflowDraftStatus::Suggested
    {
        return Err(DagRuntimeError::InvalidSnapshot(
            "executor requires a validated or published Workflow snapshot".to_owned(),
        ));
    }
    Ok(draft)
}

fn verify_snapshot_hash(workflow: &PublishedWorkflowVersion) -> Result<(), DagRuntimeError> {
    let material = workflow
        .snapshot
        .content_hash_material()
        .map_err(|error| DagRuntimeError::InvalidSnapshot(error.to_string()))?;
    let actual = format!("{:x}", Sha256::digest(material));
    if actual != workflow.content_hash {
        return Err(DagRuntimeError::InvalidSnapshot(format!(
            "content hash mismatch: expected {}, computed {actual}",
            workflow.content_hash
        )));
    }
    Ok(())
}

fn node_cache_key(
    workflow: &PublishedWorkflowVersion,
    node: &WorkflowDraftNode,
    inputs: &DagInputs,
) -> Result<String, serde_json::Error> {
    let model = node.model_binding.as_deref().and_then(|model_id| {
        workflow
            .snapshot
            .models
            .iter()
            .find(|model| model.id == model_id)
    });
    let pipeline_inputs = inputs
        .pipeline_artifacts
        .iter()
        .map(|artifact| match artifact {
            PipelineArtifact::Image(image) => Ok(serde_json::json!({
                "kind": "image",
                "image_id": image.image_id,
                "content_ref": image.blob_ref,
                "width": image.width,
                "height": image.height,
                "mime_type": image.mime_type,
            })),
            other => serde_json::to_value(other),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let node_config = serde_json::json!({
        "operation": node.node_type,
        "kind": node.kind,
        "parameters": node.parameters,
        "resources": node.resources,
        "retry_policy": node.retry_policy,
        "fallback_policy": node.fallback_policy,
        "gate": node.gate,
        "validators": node.validators,
        "refiners": node.refiners,
    });
    let node_config_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&node_config)?));
    let material = serde_json::to_vec(&serde_json::json!({
        "input_artifacts": inputs.artifacts,
        "pipeline_inputs": pipeline_inputs,
        "model_id": node.model_binding,
        "model_version": model.map(|model| &model.version.model_version),
        "checkpoint_sha256": model.and_then(|model| model.version.checkpoint_sha256.as_deref()),
        "backend_protocol_version": model.map(|model| &model.version.backend_protocol_version),
        "model_configuration": model.map(|model| &model.configuration),
        "node_config_hash": node_config_hash,
        "queries": node.parameters.get("queries"),
        "project_label_mapping": node.parameters.get("class_mapping"),
        "target_labels": node.parameters.get("target_labels"),
        "skills": workflow.snapshot.enabled_skills,
    }))?;
    Ok(format!("{:x}", Sha256::digest(material)))
}

#[allow(clippy::too_many_arguments)]
fn trace(
    request: &DagExecutionRequest,
    node: &WorkflowDraftNode,
    status: DagNodeStatus,
    attempt_count: u32,
    cache_key: Option<String>,
    cache_hit: bool,
    inputs: DagInputs,
    output: DagNodeOutput,
    error: Option<DagNodeFailure>,
    started_at: DateTime<Utc>,
) -> DagNodeTrace {
    let input_envelopes = artifact_envelopes(
        request,
        &inputs.artifacts,
        &inputs.pipeline_artifacts,
        cache_key.as_ref(),
    );
    let output_envelopes = artifact_envelopes(
        request,
        &output.artifacts,
        &output.pipeline_artifacts,
        cache_key.as_ref(),
    );
    DagNodeTrace {
        node_id: node.id.clone(),
        operation: node.node_type.clone(),
        status,
        attempt_count,
        cache_key,
        cache_hit,
        input_artifacts: inputs.artifacts,
        output_artifacts: output.artifacts,
        input_pipeline_artifacts: inputs.pipeline_artifacts,
        output_pipeline_artifacts: output.pipeline_artifacts,
        input_envelopes,
        output_envelopes,
        route: output.route,
        usage: output.usage,
        error,
        started_at,
        finished_at: Utc::now(),
    }
}

fn artifact_envelopes(
    request: &DagExecutionRequest,
    artifacts: &[VisionArtifact],
    pipeline_artifacts: &[PipelineArtifact],
    cache_key: Option<&String>,
) -> Vec<ArtifactEnvelope> {
    artifacts
        .iter()
        .cloned()
        .map(|artifact| {
            let source_node = artifact.source_node.clone();
            ArtifactEnvelope::from_vision(
                request.project_id,
                request.run_id,
                source_node,
                artifact,
                cache_key.cloned(),
            )
        })
        .chain(pipeline_artifacts.iter().cloned().map(|artifact| {
            let source_node = artifact.reference().source_node.clone();
            ArtifactEnvelope::from_pipeline(
                request.project_id,
                request.run_id,
                source_node,
                artifact,
                ArtifactProvenance::default(),
                cache_key.cloned(),
            )
        }))
        .collect()
}

fn cancel_pending(checkpoint: &mut DagCheckpoint) {
    for status in checkpoint.node_statuses.values_mut() {
        if matches!(
            status,
            DagNodeStatus::Pending | DagNodeStatus::Running | DagNodeStatus::AwaitingReview
        ) {
            *status = DagNodeStatus::Cancelled;
        }
    }
}

fn result(status: DagRunStatus, checkpoint: DagCheckpoint, draft: &WorkflowDraft) -> DagRunResult {
    let committed = draft
        .nodes
        .iter()
        .filter(|node| node.kind == WorkflowNodeKind::Commit)
        .filter_map(|node| checkpoint.node_outputs.get(&node.id))
        .flat_map(|output| output.artifacts.clone())
        .collect();
    let committed_pipeline_artifacts = draft
        .nodes
        .iter()
        .filter(|node| node.kind == WorkflowNodeKind::Commit)
        .filter_map(|node| checkpoint.node_outputs.get(&node.id))
        .flat_map(|output| output.pipeline_artifacts.clone())
        .collect();
    DagRunResult {
        status,
        checkpoint,
        committed,
        committed_pipeline_artifacts,
    }
}
