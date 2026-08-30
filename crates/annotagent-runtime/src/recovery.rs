//! Bounded, evidence-driven fallback selection for detection workflows.

use std::{collections::BTreeMap, sync::Arc, time::Instant};

use annotagent_core::{
    AgentKind, AgentSession, ArtifactKind, ArtifactRef, ArtifactValidationState,
    CandidateClusterSetArtifact, CorrectionRisk, DETECTION_RECOVERY_PROTOCOL_VERSION,
    DetectionRecoveryAction, DetectionRecoveryPolicy, DetectionRecoveryReport,
    DetectionRecoveryRequest, DetectionRecoveryStopCondition, DetectionSetArtifact,
    EvidenceGateDecision, EvidenceGateInput, EvidenceGateReason, EvidenceGateReport, ModelImage,
    PIPELINE_VISION_PROTOCOL_VERSION, PipelineArtifact, PipelineInferenceRequest,
    PipelineModelBackend, ValidationIssue, VisionCapability, VisionNodeDescriptor,
};
use async_trait::async_trait;
use serde_json::Value;

use crate::{
    DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner, DagNodeUsage,
    cluster_single_detection_set, evaluate_detection_evidence, match_detection_sets,
};

pub const DETECTION_RECOVERY_OPERATION: &str = "agent.detection_recovery";

#[must_use]
pub fn detection_recovery_node_descriptor() -> VisionNodeDescriptor {
    VisionNodeDescriptor {
        id: DETECTION_RECOVERY_OPERATION.to_owned(),
        display_name: "Detection recovery".to_owned(),
        required_capabilities: vec![VisionCapability::OpenVocabularyDetection],
        accepts: vec![ArtifactKind::Image, ArtifactKind::DetectionSet],
        produces: vec![ArtifactKind::CandidateClusterSet],
        deterministic: false,
    }
}

pub struct DetectionRecoveryAgent {
    fallback_backend: Arc<dyn PipelineModelBackend>,
    fallback_model_id: String,
    image: Option<ModelImage>,
}

impl DetectionRecoveryAgent {
    pub fn new(
        fallback_backend: Arc<dyn PipelineModelBackend>,
        fallback_model_id: impl Into<String>,
        image: Option<ModelImage>,
    ) -> Result<Self, String> {
        if fallback_backend.capability() != VisionCapability::OpenVocabularyDetection {
            return Err(
                "Detection Recovery requires an OpenVocabularyDetection backend".to_owned(),
            );
        }
        let fallback_model_id = fallback_model_id.into();
        if fallback_model_id.trim().is_empty() {
            return Err("Detection Recovery requires a fallback Model binding".to_owned());
        }
        Ok(Self {
            fallback_backend,
            fallback_model_id,
            image,
        })
    }
}

#[async_trait]
impl DagNodeRunner for DetectionRecoveryAgent {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        let primary = one_detection_set(&context)?;
        primary
            .validate()
            .map_err(|error| DagNodeFailure::terminal("invalid_primary_detection_set", error))?;
        if !context
            .input_pipeline_artifacts
            .iter()
            .any(|artifact| matches!(artifact, PipelineArtifact::Image(_)))
        {
            return Err(DagNodeFailure::terminal(
                "missing_image_input",
                "Detection Recovery requires the source Image and primary DetectionSet",
            ));
        }
        let request = recovery_request(&context)?;
        request
            .validate()
            .map_err(|error| DagNodeFailure::terminal("invalid_recovery_policy", error))?;
        let output_ref = ArtifactRef {
            artifact_id: format!(
                "candidate-clusters:{}:{}:{}",
                context.run_id, context.image_id, context.node.id
            ),
            source_node: context.node.id.clone(),
            port: "candidates".to_owned(),
            artifact_type: ArtifactKind::CandidateClusterSet,
            item_id: None,
        };
        let mut primary_clusters = cluster_single_detection_set(output_ref.clone(), primary)
            .map_err(|error| {
                DagNodeFailure::terminal("primary_evidence_projection_failed", error)
            })?;
        let validation_issues = collected_validation_issues(&context)?;
        let correction_risk = collected_correction_risk(&context)?;
        let initial_input = EvidenceGateInput {
            candidates: primary_clusters.candidates.clone(),
            validation_issues: validation_issues.clone(),
            correction_risk: correction_risk.clone(),
        };
        let initial_sources = vec![(primary.model_binding.clone(), primary.detections.len())];
        let initial_evidence = evaluate_detection_evidence(
            &initial_input,
            &request.policy.initial_gate,
            &initial_sources,
        );
        let mut session =
            AgentSession::start(AgentKind::AnnotationRecovery, request.budget.clone())
                .with_project(context.project_id.to_string())
                .with_run(context.run_id);
        session
            .record_tool(
                "evaluate_primary_detection_evidence",
                serde_json::json!({
                    "source_model_id": primary.model_binding,
                    "detection_count": primary.detections.len(),
                    "validation_issue_count": validation_issues.len(),
                    "has_correction_risk": correction_risk.is_some(),
                }),
                evidence_trace_result(&initial_evidence),
                true,
            )
            .map_err(|error| DagNodeFailure::terminal("recovery_budget_exhausted", error))?;

        match initial_evidence.decision {
            EvidenceGateDecision::Accept => {
                primary_clusters.validation_state = ArtifactValidationState::Valid;
                session.succeed("primary detection evidence satisfied the configured accept rule");
                return recovery_output(
                    primary_clusters,
                    RecoveryOutputSpec {
                        action: DetectionRecoveryAction::KeepPrimary,
                        initial_evidence: initial_evidence.clone(),
                        final_evidence: initial_evidence,
                        fallback_model_id: Some(self.fallback_model_id.clone()),
                        fallback_invoked: false,
                        fallback_call_count: 0,
                        stop_condition: DetectionRecoveryStopCondition::PrimaryAccepted,
                        session,
                        route: "accept",
                        usage: DagNodeUsage::default(),
                        extra_metadata: BTreeMap::new(),
                    },
                );
            }
            EvidenceGateDecision::Review | EvidenceGateDecision::Reject => {
                primary_clusters.validation_state = ArtifactValidationState::NeedsReview;
                session.wait_for_human("review primary detector evidence");
                return recovery_output(
                    primary_clusters,
                    RecoveryOutputSpec {
                        action: DetectionRecoveryAction::HumanReview,
                        initial_evidence: initial_evidence.clone(),
                        final_evidence: initial_evidence,
                        fallback_model_id: Some(self.fallback_model_id.clone()),
                        fallback_invoked: false,
                        fallback_call_count: 0,
                        stop_condition: DetectionRecoveryStopCondition::InitialReviewRequired,
                        session,
                        route: "review",
                        usage: DagNodeUsage::default(),
                        extra_metadata: BTreeMap::new(),
                    },
                );
            }
            EvidenceGateDecision::Fallback => {}
        }

        if !request.policy.allow_fallback || request.policy.max_fallback_calls == 0 {
            return stopped_for_review(
                primary_clusters,
                initial_evidence,
                session,
                &self.fallback_model_id,
                DetectionRecoveryStopCondition::FallbackDisabled,
                "fallback_disabled",
                "Fallback is disabled by the published Recovery policy",
                "review primary result because fallback is disabled",
            );
        }
        if request.queries.is_empty() {
            return stopped_for_review(
                primary_clusters,
                initial_evidence,
                session,
                &self.fallback_model_id,
                DetectionRecoveryStopCondition::FallbackUnavailable,
                "fallback_queries_missing",
                "No registry-bounded fallback query is configured",
                "configure fallback queries or review the primary result",
            );
        }
        if !session
            .budget
            .can_reserve(&session.usage, 1, request.policy.fallback_estimated_cost)
        {
            return stopped_for_review(
                primary_clusters,
                initial_evidence,
                session,
                &self.fallback_model_id,
                DetectionRecoveryStopCondition::BudgetInsufficient,
                "fallback_budget_insufficient",
                "Remaining Agent budget is insufficient for one fallback request",
                "review primary result because fallback budget is exhausted",
            );
        }

        let started = Instant::now();
        let backend_result = self
            .fallback_backend
            .infer_pipeline(
                PipelineInferenceRequest {
                    protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
                    request_id: format!(
                        "recovery:{}:{}:{}",
                        context.run_id, context.image_id, context.node.id
                    ),
                    run_id: context.run_id,
                    image_id: context.image_id,
                    node_id: context.node.id.clone(),
                    model_id: self.fallback_model_id.clone(),
                    operation: VisionCapability::OpenVocabularyDetection,
                    image: self.image.clone(),
                    input_artifacts: context.input_pipeline_artifacts.clone(),
                    parameters: fallback_parameters(&context, &request)?,
                    timeout_ms: context
                        .node
                        .resources
                        .timeout_seconds
                        .map(|seconds| seconds.saturating_mul(1_000)),
                },
                context.cancellation.clone(),
            )
            .await;
        let elapsed_ms = started.elapsed().as_millis() as u64;
        session.add_model_usage(0, 0, request.policy.fallback_estimated_cost);
        let response = match backend_result {
            Ok(response) if response.error.is_none() => response,
            Ok(response) => {
                let error = response.error.expect("checked as present");
                let _ = session.record_tool(
                    "invoke_fallback_detection",
                    fallback_trace_arguments(&self.fallback_model_id, &request),
                    serde_json::json!({
                        "error_code": error.code,
                        "error_message": error.message,
                        "retryable": error.retryable,
                        "duration_ms": elapsed_ms,
                    }),
                    false,
                );
                return invoked_fallback_failed_for_review(
                    primary_clusters,
                    initial_evidence,
                    session,
                    &self.fallback_model_id,
                    "fallback_model_error",
                    "Fallback detector returned a structured error; primary evidence was preserved",
                    "review primary result or retry fallback from this node",
                    request.policy.fallback_estimated_cost,
                );
            }
            Err(error) => {
                let _ = session.record_tool(
                    "invoke_fallback_detection",
                    fallback_trace_arguments(&self.fallback_model_id, &request),
                    serde_json::json!({
                        "error": error.to_string(),
                        "duration_ms": elapsed_ms,
                    }),
                    false,
                );
                return invoked_fallback_failed_for_review(
                    primary_clusters,
                    initial_evidence,
                    session,
                    &self.fallback_model_id,
                    "fallback_unavailable",
                    "Fallback detector could not be reached; primary evidence was preserved",
                    "review primary result or retry fallback from this node",
                    request.policy.fallback_estimated_cost,
                );
            }
        };
        let response_metadata = response.metadata.clone();
        let fallback = match one_fallback_detection_set(response.artifacts, context.image_id) {
            Ok(fallback) => fallback,
            Err(error) => {
                let _ = session.record_tool(
                    "invoke_fallback_detection",
                    fallback_trace_arguments(&self.fallback_model_id, &request),
                    serde_json::json!({
                        "error_code": "invalid_fallback_output",
                        "error_message": error,
                        "duration_ms": elapsed_ms,
                    }),
                    false,
                );
                return invoked_fallback_failed_for_review(
                    primary_clusters,
                    initial_evidence,
                    session,
                    &self.fallback_model_id,
                    "invalid_fallback_output",
                    "Fallback detector output was invalid; primary evidence was preserved",
                    "review primary result or repair the fallback Worker",
                    request.policy.fallback_estimated_cost,
                );
            }
        };
        let mut combined = match match_detection_sets(
            output_ref,
            primary,
            &fallback,
            request.policy.match_minimum_iou,
            true,
        ) {
            Ok(combined) => combined,
            Err(error) => {
                let _ = session.record_tool(
                    "invoke_fallback_detection",
                    fallback_trace_arguments(&self.fallback_model_id, &request),
                    serde_json::json!({
                        "error_code": "fallback_match_failed",
                        "error_message": error,
                        "duration_ms": elapsed_ms,
                    }),
                    false,
                );
                return invoked_fallback_failed_for_review(
                    primary_clusters,
                    initial_evidence,
                    session,
                    &self.fallback_model_id,
                    "fallback_match_failed",
                    "Fallback evidence could not be matched safely; primary evidence was preserved",
                    "review primary result and invalid fallback evidence",
                    request.policy.fallback_estimated_cost,
                );
            }
        };
        let final_input = EvidenceGateInput {
            candidates: combined.candidates.clone(),
            validation_issues,
            correction_risk,
        };
        let final_sources = vec![
            (primary.model_binding.clone(), primary.detections.len()),
            (fallback.model_binding.clone(), fallback.detections.len()),
        ];
        let mut final_evidence =
            evaluate_detection_evidence(&final_input, &request.policy.final_gate, &final_sources);
        if final_evidence.decision == EvidenceGateDecision::Fallback {
            final_evidence = forced_review(
                &combined,
                "fallback_limit_reached",
                "The bounded fallback call completed; another automatic model call is not allowed",
            );
        }
        combined.validation_state = validation_state(final_evidence.decision);
        session
            .record_tool(
                "invoke_fallback_detection",
                fallback_trace_arguments(&self.fallback_model_id, &request),
                serde_json::json!({
                    "detection_count": fallback.detections.len(),
                    "duration_ms": elapsed_ms,
                    "match_minimum_iou": request.policy.match_minimum_iou,
                    "candidate_count": combined.candidates.len(),
                    "final_decision": final_evidence.decision,
                    "reason_codes": reason_codes(&final_evidence),
                }),
                true,
            )
            .map_err(|error| DagNodeFailure::terminal("recovery_budget_exhausted", error))?;
        let route = if final_evidence.decision == EvidenceGateDecision::Accept {
            session.succeed("fallback evidence completed and the final Evidence Gate accepted");
            "accept"
        } else {
            session.wait_for_human("review combined detector evidence");
            "review"
        };
        recovery_output(
            combined,
            RecoveryOutputSpec {
                action: DetectionRecoveryAction::InvokeFallback,
                initial_evidence,
                final_evidence,
                fallback_model_id: Some(self.fallback_model_id.clone()),
                fallback_invoked: true,
                fallback_call_count: 1,
                stop_condition: DetectionRecoveryStopCondition::FallbackCompleted,
                session,
                route,
                usage: DagNodeUsage {
                    cost: request.policy.fallback_estimated_cost,
                    ..DagNodeUsage::default()
                },
                extra_metadata: BTreeMap::from([
                    (
                        "fallback_backend".to_owned(),
                        Value::Object(response_metadata.into_iter().collect()),
                    ),
                    (
                        "source_summaries".to_owned(),
                        serde_json::json!(
                            final_sources
                                .iter()
                                .map(|(model_id, detection_count)| serde_json::json!({
                                    "model_id": model_id,
                                    "detection_count": detection_count,
                                }))
                                .collect::<Vec<_>>()
                        ),
                    ),
                ]),
            },
        )
    }
}

struct RecoveryOutputSpec<'a> {
    action: DetectionRecoveryAction,
    initial_evidence: EvidenceGateReport,
    final_evidence: EvidenceGateReport,
    fallback_model_id: Option<String>,
    fallback_invoked: bool,
    fallback_call_count: u32,
    stop_condition: DetectionRecoveryStopCondition,
    session: AgentSession,
    route: &'a str,
    usage: DagNodeUsage,
    extra_metadata: BTreeMap<String, Value>,
}

fn recovery_output(
    artifact: CandidateClusterSetArtifact,
    spec: RecoveryOutputSpec<'_>,
) -> Result<DagNodeOutput, DagNodeFailure> {
    artifact
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_recovery_output", error))?;
    let report = DetectionRecoveryReport {
        protocol_version: DETECTION_RECOVERY_PROTOCOL_VERSION,
        action: spec.action,
        initial_evidence: spec.initial_evidence,
        final_evidence: spec.final_evidence,
        fallback_model_id: spec.fallback_model_id,
        fallback_invoked: spec.fallback_invoked,
        fallback_call_count: spec.fallback_call_count,
        stop_condition: spec.stop_condition,
        session: spec.session,
    };
    let mut metadata = spec.extra_metadata;
    metadata.insert(
        "agent_trace".to_owned(),
        serde_json::to_value(&report.session).unwrap_or_else(|_| serde_json::json!({})),
    );
    metadata.insert(
        "recovery_agent".to_owned(),
        serde_json::to_value(&report).unwrap_or_else(|_| serde_json::json!({})),
    );
    Ok(DagNodeOutput {
        pipeline_artifacts: vec![PipelineArtifact::CandidateClusterSet(artifact)],
        route: Some(spec.route.to_owned()),
        usage: spec.usage,
        metadata,
        ..DagNodeOutput::default()
    })
}

#[allow(clippy::too_many_arguments)]
fn stopped_for_review(
    mut artifact: CandidateClusterSetArtifact,
    initial_evidence: EvidenceGateReport,
    mut session: AgentSession,
    fallback_model_id: &str,
    stop_condition: DetectionRecoveryStopCondition,
    reason_code: &str,
    reason_message: &str,
    pending_action: &str,
) -> Result<DagNodeOutput, DagNodeFailure> {
    artifact.validation_state = ArtifactValidationState::NeedsReview;
    let final_evidence = forced_review(&artifact, reason_code, reason_message);
    session.wait_for_human(pending_action);
    recovery_output(
        artifact,
        RecoveryOutputSpec {
            action: DetectionRecoveryAction::HumanReview,
            initial_evidence,
            final_evidence,
            fallback_model_id: Some(fallback_model_id.to_owned()),
            fallback_invoked: false,
            fallback_call_count: 0,
            stop_condition,
            session,
            route: "review",
            usage: DagNodeUsage::default(),
            extra_metadata: BTreeMap::new(),
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn invoked_fallback_failed_for_review(
    mut artifact: CandidateClusterSetArtifact,
    initial_evidence: EvidenceGateReport,
    mut session: AgentSession,
    fallback_model_id: &str,
    reason_code: &str,
    reason_message: &str,
    pending_action: &str,
    cost: rust_decimal::Decimal,
) -> Result<DagNodeOutput, DagNodeFailure> {
    artifact.validation_state = ArtifactValidationState::NeedsReview;
    let final_evidence = forced_review(&artifact, reason_code, reason_message);
    session.wait_for_human(pending_action);
    recovery_output(
        artifact,
        RecoveryOutputSpec {
            action: DetectionRecoveryAction::InvokeFallback,
            initial_evidence,
            final_evidence,
            fallback_model_id: Some(fallback_model_id.to_owned()),
            fallback_invoked: true,
            fallback_call_count: 1,
            stop_condition: DetectionRecoveryStopCondition::FallbackUnavailable,
            session,
            route: "review",
            usage: DagNodeUsage {
                cost,
                ..DagNodeUsage::default()
            },
            extra_metadata: BTreeMap::new(),
        },
    )
}

fn forced_review(
    artifact: &CandidateClusterSetArtifact,
    code: &str,
    message: &str,
) -> EvidenceGateReport {
    EvidenceGateReport {
        decision: EvidenceGateDecision::Review,
        reasons: vec![EvidenceGateReason {
            code: code.to_owned(),
            message: message.to_owned(),
            candidate_id: None,
            source_model_ids: Vec::new(),
            metrics: BTreeMap::new(),
        }],
        candidate_count: artifact.candidates.len(),
        validation_issue_count: 0,
    }
}

fn validation_state(decision: EvidenceGateDecision) -> ArtifactValidationState {
    match decision {
        EvidenceGateDecision::Accept => ArtifactValidationState::Valid,
        EvidenceGateDecision::Fallback => ArtifactValidationState::Unvalidated,
        EvidenceGateDecision::Review => ArtifactValidationState::NeedsReview,
        EvidenceGateDecision::Reject => ArtifactValidationState::Invalid,
    }
}

fn recovery_request(
    context: &DagNodeContext<'_>,
) -> Result<DetectionRecoveryRequest, DagNodeFailure> {
    let policy = context
        .node
        .parameters
        .get("recovery_policy")
        .map(|value| serde_json::from_value::<DetectionRecoveryPolicy>(value.clone()))
        .transpose()
        .map_err(|error| DagNodeFailure::terminal("invalid_recovery_policy", error.to_string()))?
        .unwrap_or_default();
    let budget = context
        .node
        .parameters
        .get("agent_budget")
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map_err(|error| DagNodeFailure::terminal("invalid_agent_budget", error.to_string()))?
        .unwrap_or_default();
    let queries = context
        .node
        .parameters
        .get("queries")
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map_err(|error| DagNodeFailure::terminal("invalid_fallback_queries", error.to_string()))?
        .unwrap_or_default();
    Ok(DetectionRecoveryRequest {
        protocol_version: DETECTION_RECOVERY_PROTOCOL_VERSION,
        policy,
        budget,
        queries,
    })
}

fn fallback_parameters(
    context: &DagNodeContext<'_>,
    request: &DetectionRecoveryRequest,
) -> Result<BTreeMap<String, Value>, DagNodeFailure> {
    let mut parameters: BTreeMap<String, Value> = context
        .node
        .parameters
        .get("fallback_parameters")
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map_err(|error| {
            DagNodeFailure::terminal("invalid_fallback_parameters", error.to_string())
        })?
        .unwrap_or_default();
    parameters.insert(
        "queries".to_owned(),
        serde_json::to_value(&request.queries).map_err(|error| {
            DagNodeFailure::terminal("invalid_fallback_queries", error.to_string())
        })?,
    );
    Ok(parameters)
}

fn fallback_trace_arguments(model_id: &str, request: &DetectionRecoveryRequest) -> Value {
    serde_json::json!({
        "model_id": model_id,
        "capability": VisionCapability::OpenVocabularyDetection,
        "query_ids": request.queries.iter().map(|query| &query.id).collect::<Vec<_>>(),
        "estimated_cost": request.policy.fallback_estimated_cost,
        "max_fallback_calls": request.policy.max_fallback_calls,
    })
}

fn evidence_trace_result(report: &EvidenceGateReport) -> Value {
    serde_json::json!({
        "decision": report.decision,
        "reason_codes": reason_codes(report),
        "candidate_count": report.candidate_count,
        "validation_issue_count": report.validation_issue_count,
    })
}

fn reason_codes(report: &EvidenceGateReport) -> Vec<&str> {
    report
        .reasons
        .iter()
        .map(|reason| reason.code.as_str())
        .collect()
}

fn one_detection_set<'a>(
    context: &'a DagNodeContext<'_>,
) -> Result<&'a DetectionSetArtifact, DagNodeFailure> {
    let sets = context
        .input_pipeline_artifacts
        .iter()
        .filter_map(|artifact| match artifact {
            PipelineArtifact::DetectionSet(set) => Some(set),
            _ => None,
        })
        .collect::<Vec<_>>();
    if sets.len() != 1 {
        return Err(DagNodeFailure::terminal(
            "invalid_primary_detection_input",
            "Detection Recovery requires exactly one primary DetectionSet",
        ));
    }
    Ok(sets[0])
}

fn one_fallback_detection_set(
    artifacts: Vec<PipelineArtifact>,
    image_id: annotagent_core::ImageId,
) -> Result<DetectionSetArtifact, String> {
    if artifacts.len() != 1 {
        return Err("fallback backend must return exactly one DetectionSet".to_owned());
    }
    let PipelineArtifact::DetectionSet(set) = artifacts.into_iter().next().expect("length checked")
    else {
        return Err("fallback backend returned another Artifact type".to_owned());
    };
    set.validate()?;
    if set.image_id != image_id {
        return Err("fallback DetectionSet belongs to another image".to_owned());
    }
    Ok(set)
}

fn collected_validation_issues(
    context: &DagNodeContext<'_>,
) -> Result<Vec<ValidationIssue>, DagNodeFailure> {
    let mut issues = Vec::new();
    let values = context
        .input_metadata
        .values()
        .filter_map(|metadata| metadata.get("validation_issues"))
        .chain(context.node.parameters.get("validation_issues"));
    for value in values {
        issues.extend(
            serde_json::from_value::<Vec<ValidationIssue>>(value.clone()).map_err(|error| {
                DagNodeFailure::terminal("invalid_validation_evidence", error.to_string())
            })?,
        );
    }
    let mut seen = std::collections::BTreeSet::new();
    issues.retain(|issue| seen.insert((issue.code.clone(), issue.message.clone())));
    Ok(issues)
}

fn collected_correction_risk(
    context: &DagNodeContext<'_>,
) -> Result<Option<CorrectionRisk>, DagNodeFailure> {
    let value = context.node.parameters.get("correction_risk").or_else(|| {
        context
            .input_metadata
            .values()
            .find_map(|metadata| metadata.get("correction_risk"))
    });
    value
        .map(|value| {
            serde_json::from_value::<CorrectionRisk>(value.clone()).map_err(|error| {
                DagNodeFailure::terminal("invalid_correction_risk", error.to_string())
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use annotagent_core::{
        AgentBudget, ArtifactId, Detection, DetectionScore, DetectionSource, ImageArtifact,
        ImageId, LabelId, NormalizedRect, PipelineInferenceResponse, ProjectId, RunId,
        VisionBackendError, WorkflowDraftNode,
    };
    use rust_decimal::Decimal;
    use tokio_util::sync::CancellationToken;

    use super::*;

    struct FixtureFallback {
        calls: Arc<AtomicUsize>,
        rect: NormalizedRect,
        fail: bool,
    }

    #[async_trait]
    impl PipelineModelBackend for FixtureFallback {
        fn id(&self) -> &str {
            "fixture-open"
        }

        fn capability(&self) -> VisionCapability {
            VisionCapability::OpenVocabularyDetection
        }

        async fn infer_pipeline(
            &self,
            request: PipelineInferenceRequest,
            _cancellation: CancellationToken,
        ) -> annotagent_core::CoreResult<PipelineInferenceResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Ok(PipelineInferenceResponse {
                    error: Some(VisionBackendError {
                        code: "fixture_unavailable".to_owned(),
                        message: "fixture unavailable".to_owned(),
                        retryable: true,
                    }),
                    ..PipelineInferenceResponse::default()
                });
            }
            let artifact_id = format!("fallback:{}", request.request_id);
            let detection = Detection::from_source(
                "fallback-0",
                Some("query-0".to_owned()),
                None,
                Some(LabelId::from("target")),
                self.rect,
                DetectionScore::not_provided(),
                DetectionSource {
                    model_id: request.model_id.clone(),
                    capability: request.operation,
                    artifact_id: artifact_id.clone(),
                },
            )
            .map_err(annotagent_core::CoreError::Validation)?;
            Ok(PipelineInferenceResponse {
                artifacts: vec![PipelineArtifact::DetectionSet(DetectionSetArtifact {
                    schema_version: annotagent_core::DETECTION_ARTIFACT_SCHEMA_VERSION,
                    reference: ArtifactRef {
                        artifact_id,
                        source_node: request.node_id,
                        port: "detections".to_owned(),
                        artifact_type: ArtifactKind::DetectionSet,
                        item_id: None,
                    },
                    image_id: request.image_id,
                    model_binding: request.model_id,
                    validation_state: ArtifactValidationState::Unvalidated,
                    detections: vec![detection],
                    metadata: BTreeMap::new(),
                })],
                ..PipelineInferenceResponse::default()
            })
        }
    }

    fn primary(score: Option<f32>) -> DetectionSetArtifact {
        let image_id = ImageId::new();
        let reference = ArtifactRef {
            artifact_id: "primary-set".to_owned(),
            source_node: "specialist".to_owned(),
            port: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            item_id: None,
        };
        let detections = score
            .map(|score| {
                Detection::from_source(
                    "primary-0",
                    None,
                    Some("target".to_owned()),
                    Some(LabelId::from("target")),
                    NormalizedRect::new(0.1, 0.1, 0.2, 0.2).expect("rect"),
                    DetectionScore::relative(score).expect("score"),
                    DetectionSource {
                        model_id: "specialist".to_owned(),
                        capability: VisionCapability::ObjectDetection,
                        artifact_id: reference.artifact_id.clone(),
                    },
                )
                .expect("detection")
            })
            .into_iter()
            .collect();
        DetectionSetArtifact {
            schema_version: annotagent_core::DETECTION_ARTIFACT_SCHEMA_VERSION,
            reference,
            image_id,
            model_binding: "specialist".to_owned(),
            validation_state: ArtifactValidationState::Unvalidated,
            detections,
            metadata: BTreeMap::new(),
        }
    }

    fn node(policy: DetectionRecoveryPolicy, budget: AgentBudget) -> WorkflowDraftNode {
        WorkflowDraftNode {
            id: "recovery".to_owned(),
            node_type: DETECTION_RECOVERY_OPERATION.to_owned(),
            model_binding: Some("open".to_owned()),
            parameters: BTreeMap::from([
                (
                    "recovery_policy".to_owned(),
                    serde_json::to_value(policy).expect("policy"),
                ),
                (
                    "agent_budget".to_owned(),
                    serde_json::to_value(budget).expect("budget"),
                ),
                (
                    "queries".to_owned(),
                    serde_json::json!([{
                        "id": "query-0",
                        "text": "target object",
                        "target_label": "target"
                    }]),
                ),
            ]),
            ..WorkflowDraftNode::default()
        }
    }

    async fn run_case(
        primary: DetectionSetArtifact,
        policy: DetectionRecoveryPolicy,
        budget: AgentBudget,
        metadata: BTreeMap<String, BTreeMap<String, Value>>,
        calls: Arc<AtomicUsize>,
    ) -> DagNodeOutput {
        run_case_with_failure(primary, policy, budget, metadata, calls, false).await
    }

    async fn run_case_with_failure(
        primary: DetectionSetArtifact,
        policy: DetectionRecoveryPolicy,
        budget: AgentBudget,
        metadata: BTreeMap<String, BTreeMap<String, Value>>,
        calls: Arc<AtomicUsize>,
        fail: bool,
    ) -> DagNodeOutput {
        let image_id = primary.image_id;
        let node = node(policy, budget);
        let image = PipelineArtifact::Image(ImageArtifact {
            reference: ArtifactRef {
                artifact_id: ArtifactId::new().to_string(),
                source_node: "image".to_owned(),
                port: "image".to_owned(),
                artifact_type: ArtifactKind::Image,
                item_id: None,
            },
            image_id,
            width: 640,
            height: 480,
            mime_type: "image/png".to_owned(),
            blob_ref: "fixture://image".to_owned(),
        });
        DetectionRecoveryAgent::new(
            Arc::new(FixtureFallback {
                calls,
                rect: NormalizedRect::new(0.1, 0.1, 0.2, 0.2).expect("rect"),
                fail,
            }),
            "open",
            None,
        )
        .expect("agent")
        .run(DagNodeContext {
            project_id: ProjectId::new(),
            run_id: RunId::new(),
            image_id,
            node: &node,
            input_artifacts: Vec::new(),
            input_pipeline_artifacts: vec![image, PipelineArtifact::DetectionSet(primary)],
            input_metadata: metadata,
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("recovery output")
    }

    fn report(output: &DagNodeOutput) -> DetectionRecoveryReport {
        serde_json::from_value(output.metadata["recovery_agent"].clone()).expect("report")
    }

    #[tokio::test]
    async fn high_specialist_score_skips_fallback() {
        let calls = Arc::new(AtomicUsize::new(0));
        let output = run_case(
            primary(Some(0.93)),
            DetectionRecoveryPolicy::default(),
            AgentBudget::default(),
            BTreeMap::new(),
            calls.clone(),
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(output.route.as_deref(), Some("accept"));
        assert_eq!(
            report(&output).stop_condition,
            DetectionRecoveryStopCondition::PrimaryAccepted
        );
    }

    #[tokio::test]
    async fn empty_specialist_result_invokes_one_fallback_and_stops() {
        let calls = Arc::new(AtomicUsize::new(0));
        let output = run_case(
            primary(None),
            DetectionRecoveryPolicy::default(),
            AgentBudget::default(),
            BTreeMap::new(),
            calls.clone(),
        )
        .await;
        let report = report(&output);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(report.fallback_invoked);
        assert_eq!(report.fallback_call_count, 1);
        assert_eq!(
            report.initial_evidence.decision,
            EvidenceGateDecision::Fallback
        );
        assert_eq!(report.final_evidence.decision, EvidenceGateDecision::Review);
        assert_eq!(report.session.steps.len(), 2);
    }

    #[tokio::test]
    async fn domain_risk_invokes_fallback_and_trace_explains_why() {
        let calls = Arc::new(AtomicUsize::new(0));
        let metadata = BTreeMap::from([(
            "validator".to_owned(),
            BTreeMap::from([(
                "validation_issues".to_owned(),
                serde_json::json!([{
                    "code": "possible_missed_detection",
                    "message": "scene evidence suggests a missed object",
                    "severity": "warning",
                    "annotation_ids": [],
                    "suggested_action": "human_review",
                    "evidence": {"kind": "rule", "facts": {}}
                }]),
            )]),
        )]);
        let output = run_case(
            primary(Some(0.93)),
            DetectionRecoveryPolicy::default(),
            AgentBudget::default(),
            metadata,
            calls.clone(),
        )
        .await;
        let report = report(&output);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            report
                .initial_evidence
                .reasons
                .iter()
                .any(|reason| reason.code == "domain_issue")
        );
        let trace = serde_json::to_string(&report.session.steps).expect("trace");
        assert!(trace.contains("domain_issue"));
        assert!(!trace.contains("chain_of_thought"));
        assert!(!trace.contains("target object"));
    }

    #[tokio::test]
    async fn insufficient_cost_budget_routes_review_without_calling_fallback() {
        let calls = Arc::new(AtomicUsize::new(0));
        let policy = DetectionRecoveryPolicy {
            fallback_estimated_cost: Decimal::new(5, 2),
            ..DetectionRecoveryPolicy::default()
        };
        let output = run_case(
            primary(None),
            policy,
            AgentBudget {
                max_cost: Some(Decimal::new(1, 2)),
                ..AgentBudget::default()
            },
            BTreeMap::new(),
            calls.clone(),
        )
        .await;
        let report = report(&output);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(output.route.as_deref(), Some("review"));
        assert_eq!(
            report.stop_condition,
            DetectionRecoveryStopCondition::BudgetInsufficient
        );
        assert_eq!(
            report.final_evidence.reasons[0].code,
            "fallback_budget_insufficient"
        );
    }

    #[tokio::test]
    async fn agreeing_fallback_changes_gate_decision_to_accept() {
        let calls = Arc::new(AtomicUsize::new(0));
        let output = run_case(
            primary(Some(0.40)),
            DetectionRecoveryPolicy::default(),
            AgentBudget::default(),
            BTreeMap::new(),
            calls,
        )
        .await;
        let report = report(&output);
        assert_eq!(
            report.initial_evidence.decision,
            EvidenceGateDecision::Fallback
        );
        assert_eq!(report.final_evidence.decision, EvidenceGateDecision::Accept);
        assert_eq!(output.route.as_deref(), Some("accept"));
    }

    #[tokio::test]
    async fn unavailable_fallback_preserves_primary_evidence_for_review() {
        let calls = Arc::new(AtomicUsize::new(0));
        let output = run_case_with_failure(
            primary(None),
            DetectionRecoveryPolicy::default(),
            AgentBudget::default(),
            BTreeMap::new(),
            calls.clone(),
            true,
        )
        .await;
        let report = report(&output);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(report.fallback_invoked);
        assert_eq!(report.fallback_call_count, 1);
        assert_eq!(output.route.as_deref(), Some("review"));
        assert_eq!(
            report.stop_condition,
            DetectionRecoveryStopCondition::FallbackUnavailable
        );
        assert_eq!(
            report.final_evidence.reasons[0].code,
            "fallback_model_error"
        );
    }
}
