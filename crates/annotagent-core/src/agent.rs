//! Shared, auditable session state for bounded `AnnotAgent` loops.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ModelBindingSource, ModelProfileId, ProviderAdapterKind, ProviderId, UsageSource};

pub const DETECTION_RECOVERY_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    PipelineBuilder,
    WorkflowAdvisor,
    AnnotationRecovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSessionStatus {
    Running,
    WaitingForHuman,
    Succeeded,
    Failed,
    BudgetExceeded,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentBudget {
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub max_tokens: Option<u64>,
    pub max_cost: Option<Decimal>,
}

impl Default for AgentBudget {
    fn default() -> Self {
        Self {
            max_steps: 16,
            max_tool_calls: 16,
            max_tokens: None,
            max_cost: None,
        }
    }
}

impl AgentBudget {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_steps == 0 {
            return Err("Agent max_steps must be greater than zero".to_owned());
        }
        if self.max_tool_calls == 0 {
            return Err("Agent max_tool_calls must be greater than zero".to_owned());
        }
        if self.max_cost.is_some_and(|value| value < Decimal::ZERO) {
            return Err("Agent max_cost cannot be negative".to_owned());
        }
        Ok(())
    }

    #[must_use]
    pub fn can_reserve(&self, usage: &AgentUsage, tool_calls: u32, cost: Decimal) -> bool {
        usage.steps.saturating_add(tool_calls) <= self.max_steps
            && usage.tool_calls.saturating_add(tool_calls) <= self.max_tool_calls
            && self.max_cost.is_none_or(|limit| usage.cost + cost <= limit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectionFallbackQuery {
    pub id: String,
    pub text: String,
    pub target_label: crate::LabelId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DetectionRecoveryPolicy {
    pub allow_fallback: bool,
    pub max_fallback_calls: u32,
    pub fallback_estimated_cost: Decimal,
    pub match_minimum_iou: f32,
    pub initial_gate: crate::EvidenceGateConfig,
    pub final_gate: crate::EvidenceGateConfig,
}

impl Default for DetectionRecoveryPolicy {
    fn default() -> Self {
        Self {
            allow_fallback: true,
            max_fallback_calls: 1,
            fallback_estimated_cost: Decimal::ZERO,
            match_minimum_iou: 0.6,
            initial_gate: crate::EvidenceGateConfig {
                accept_when: vec![crate::EvidenceAcceptRule {
                    minimum_score: Some(0.85),
                    no_domain_issue: true,
                    ..crate::EvidenceAcceptRule::default()
                }],
                fallback_when: vec![
                    crate::EvidenceFallbackRule {
                        empty_specialist_result: true,
                        ..crate::EvidenceFallbackRule::default()
                    },
                    crate::EvidenceFallbackRule {
                        specialist_score_below: Some(0.55),
                        ..crate::EvidenceFallbackRule::default()
                    },
                    crate::EvidenceFallbackRule {
                        domain_issue: true,
                        ..crate::EvidenceFallbackRule::default()
                    },
                    crate::EvidenceFallbackRule {
                        correction_risk_above: Some(0.7),
                        ..crate::EvidenceFallbackRule::default()
                    },
                ],
                review_when: vec![crate::EvidenceReviewRule {
                    score_missing: true,
                    ..crate::EvidenceReviewRule::default()
                }],
                reject_when: Vec::new(),
            },
            final_gate: crate::EvidenceGateConfig {
                accept_when: vec![crate::EvidenceAcceptRule {
                    minimum_sources: Some(2),
                    minimum_iou: Some(0.6),
                    no_domain_issue: true,
                    ..crate::EvidenceAcceptRule::default()
                }],
                fallback_when: Vec::new(),
                review_when: vec![
                    crate::EvidenceReviewRule {
                        geometry_conflict: true,
                        ..crate::EvidenceReviewRule::default()
                    },
                    crate::EvidenceReviewRule {
                        label_conflict: true,
                        ..crate::EvidenceReviewRule::default()
                    },
                    crate::EvidenceReviewRule {
                        open_vocab_only: true,
                        ..crate::EvidenceReviewRule::default()
                    },
                    crate::EvidenceReviewRule {
                        empty_result: true,
                        ..crate::EvidenceReviewRule::default()
                    },
                ],
                reject_when: Vec::new(),
            },
        }
    }
}

impl DetectionRecoveryPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.fallback_estimated_cost < Decimal::ZERO {
            return Err("fallback_estimated_cost cannot be negative".to_owned());
        }
        if self.max_fallback_calls > 1 {
            return Err("Detection Recovery Alpha permits at most one fallback call".to_owned());
        }
        if !(0.0..=1.0).contains(&self.match_minimum_iou) {
            return Err("match_minimum_iou must be within [0,1]".to_owned());
        }
        self.initial_gate.validate()?;
        self.final_gate.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionRecoveryRequest {
    #[serde(default = "default_detection_recovery_protocol_version")]
    pub protocol_version: u32,
    #[serde(default)]
    pub policy: DetectionRecoveryPolicy,
    #[serde(default)]
    pub budget: AgentBudget,
    #[serde(default)]
    pub queries: Vec<DetectionFallbackQuery>,
}

const fn default_detection_recovery_protocol_version() -> u32 {
    DETECTION_RECOVERY_PROTOCOL_VERSION
}

impl DetectionRecoveryRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.protocol_version != DETECTION_RECOVERY_PROTOCOL_VERSION {
            return Err(format!(
                "unsupported Detection Recovery protocol version {}",
                self.protocol_version
            ));
        }
        self.policy.validate()?;
        self.budget.validate()?;
        let mut query_ids = std::collections::BTreeSet::new();
        for query in &self.queries {
            if query.id.trim().is_empty()
                || query.text.trim().is_empty()
                || query.target_label.as_str().trim().is_empty()
            {
                return Err(
                    "Detection fallback queries require id, text, and target_label".to_owned(),
                );
            }
            if !query_ids.insert(query.id.as_str()) {
                return Err(format!("duplicate Detection fallback query {:?}", query.id));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionRecoveryAction {
    KeepPrimary,
    InvokeFallback,
    HumanReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionRecoveryStopCondition {
    PrimaryAccepted,
    InitialReviewRequired,
    FallbackCompleted,
    FallbackDisabled,
    BudgetInsufficient,
    FallbackUnavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionRecoveryReport {
    pub protocol_version: u32,
    pub action: DetectionRecoveryAction,
    pub initial_evidence: crate::EvidenceGateReport,
    pub final_evidence: crate::EvidenceGateReport,
    pub fallback_model_id: Option<crate::ModelId>,
    pub fallback_invoked: bool,
    pub fallback_call_count: u32,
    pub stop_condition: DetectionRecoveryStopCondition,
    pub session: AgentSession,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AgentUsage {
    pub steps: u32,
    pub tool_calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: Decimal,
}

/// Credential-free identity of the Registry model selected for an Agent session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentModelSelection {
    pub provider_profile_id: ProviderId,
    pub provider_display_name: String,
    pub provider_adapter: ProviderAdapterKind,
    pub endpoint_summary: String,
    pub model_profile_id: ModelProfileId,
    pub model_profile_revision: u64,
    pub model_display_name: String,
    pub remote_model_id: String,
    pub binding_source: ModelBindingSource,
    pub locked: bool,
}

/// One auditable Provider request made by an Agent. Secrets and request bodies are excluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentModelCall {
    pub sequence: u32,
    pub provider_profile_id: Option<ProviderId>,
    pub model_profile_id: Option<ModelProfileId>,
    pub model_profile_revision: Option<u64>,
    pub provider_name: String,
    pub remote_model_id: String,
    pub request_id: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub usage_source: UsageSource,
    pub duration_ms: u64,
    pub cost: Decimal,
    pub currency: String,
    pub retry_count: u32,
    pub succeeded: bool,
    pub safe_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentToolStep {
    pub sequence: u32,
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub result: serde_json::Value,
    pub success: bool,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: Uuid,
    pub project_id: Option<String>,
    pub run_id: Option<crate::RunId>,
    pub kind: AgentKind,
    pub status: AgentSessionStatus,
    pub budget: AgentBudget,
    #[serde(default)]
    pub builder_constraints: Option<crate::PipelineBuilderConstraints>,
    #[serde(default)]
    pub model_selection: Option<AgentModelSelection>,
    #[serde(default)]
    pub model_calls: Vec<AgentModelCall>,
    #[serde(default)]
    pub phase: Option<crate::PipelineBuilderPhase>,
    #[serde(default)]
    pub outcome: Option<crate::PipelineBuilderOutcome>,
    #[serde(default)]
    pub builder_stop_reason: Option<crate::BuilderStopReason>,
    #[serde(default)]
    pub build_mode: Option<crate::PipelineBuildMode>,
    #[serde(default)]
    pub working_draft: Option<crate::BuilderWorkingDraft>,
    #[serde(default)]
    pub working_memory: Option<crate::BuilderWorkingMemory>,
    #[serde(default)]
    pub plan_candidates: Vec<crate::PipelinePlanCandidate>,
    #[serde(default)]
    pub selected_candidate_id: Option<crate::PlanCandidateId>,
    #[serde(default)]
    pub discovered_conversion_paths: Vec<crate::PipelineFragmentId>,
    #[serde(default)]
    pub planning_events: Vec<crate::BuilderPlanEvent>,
    #[serde(default)]
    pub salvage_outcome: Option<crate::BuilderSalvageOutcome>,
    #[serde(default)]
    pub builder_budget: Option<crate::PipelineBuilderBudget>,
    #[serde(default)]
    pub progress_invariant: Option<crate::BuilderProgressInvariant>,
    #[serde(default)]
    pub model_turns: u32,
    #[serde(default)]
    pub phase_tool_calls: u32,
    #[serde(default)]
    pub duplicate_tool_calls: u32,
    #[serde(default)]
    pub cache_hits: u32,
    #[serde(default)]
    pub draft_id: Option<String>,
    /// Exact user-visible Pipeline Builder proposal persisted with the Session. The Draft is also
    /// stored independently; this snapshot preserves rationale, warnings and estimates so a page
    /// refresh can reconstruct the result instead of showing only the execution trace.
    #[serde(default)]
    pub builder_proposal: Option<crate::WorkflowSuggestion>,
    #[serde(default)]
    pub unresolved_bindings: Vec<String>,
    #[serde(default)]
    pub next_action: Option<String>,
    /// Denormalized progress counters exposed directly by the Agent Session API. `usage` remains
    /// the canonical accounting record; these fields make refresh/retry UIs unambiguous.
    #[serde(default)]
    pub total_tool_calls: u32,
    #[serde(default)]
    pub remaining_tool_calls: u32,
    #[serde(default)]
    pub reserved_finalization_calls: u32,
    pub usage: AgentUsage,
    pub steps: Vec<AgentToolStep>,
    pub stop_reason: Option<String>,
    pub pending_human_action: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AgentSession {
    #[must_use]
    pub fn start(kind: AgentKind, budget: AgentBudget) -> Self {
        let now = Utc::now();
        let remaining_tool_calls = budget.max_tool_calls;
        Self {
            id: Uuid::new_v4(),
            project_id: None,
            run_id: None,
            kind,
            status: AgentSessionStatus::Running,
            budget,
            builder_constraints: None,
            model_selection: None,
            model_calls: Vec::new(),
            phase: None,
            outcome: None,
            builder_stop_reason: None,
            build_mode: None,
            working_draft: None,
            working_memory: None,
            plan_candidates: Vec::new(),
            selected_candidate_id: None,
            discovered_conversion_paths: Vec::new(),
            planning_events: Vec::new(),
            salvage_outcome: None,
            builder_budget: None,
            progress_invariant: None,
            model_turns: 0,
            phase_tool_calls: 0,
            duplicate_tool_calls: 0,
            cache_hits: 0,
            draft_id: None,
            builder_proposal: None,
            unresolved_bindings: Vec::new(),
            next_action: None,
            total_tool_calls: 0,
            remaining_tool_calls,
            reserved_finalization_calls: 0,
            usage: AgentUsage::default(),
            steps: Vec::new(),
            stop_reason: None,
            pending_human_action: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[must_use]
    pub fn with_project(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = Some(project_id.into());
        self
    }

    #[must_use]
    pub fn with_builder_constraints(
        mut self,
        constraints: crate::PipelineBuilderConstraints,
    ) -> Self {
        self.builder_constraints = Some(constraints);
        self
    }

    #[must_use]
    pub fn with_model_selection(mut self, selection: AgentModelSelection) -> Self {
        self.model_selection = Some(selection);
        self
    }

    #[must_use]
    pub fn with_builder_progress(
        mut self,
        budget: crate::PipelineBuilderBudget,
        invariant: crate::BuilderProgressInvariant,
    ) -> Self {
        self.phase = Some(crate::PipelineBuilderPhase::ContextLoading);
        self.remaining_tool_calls = budget.max_total_tool_calls;
        self.reserved_finalization_calls = budget.reserved_finalization_calls;
        self.builder_budget = Some(budget);
        self.progress_invariant = Some(invariant);
        self.next_action = Some("Load the bounded Pipeline Builder context".to_owned());
        self
    }

    #[must_use]
    pub fn with_build_mode(mut self, build_mode: crate::PipelineBuildMode) -> Self {
        self.build_mode = Some(build_mode);
        self.updated_at = Utc::now();
        self
    }

    pub fn set_builder_working_draft(
        &mut self,
        draft_id: impl Into<String>,
        build_mode: crate::PipelineBuildMode,
        context_revision: impl Into<String>,
    ) {
        let now = Utc::now();
        let draft_id = draft_id.into();
        self.build_mode = Some(build_mode.clone());
        self.draft_id = Some(draft_id.clone());
        self.working_draft = Some(crate::BuilderWorkingDraft {
            draft_id: draft_id.clone(),
            build_mode,
            created_at: now,
            updated_at: now,
        });
        self.working_memory = Some(crate::BuilderWorkingMemory {
            session_id: self.id,
            context_revision: context_revision.into(),
            project_facts: std::collections::BTreeMap::new(),
            model_facts: std::collections::BTreeMap::new(),
            node_facts: std::collections::BTreeMap::new(),
            conversion_paths: std::collections::BTreeMap::new(),
            plan_candidate_ids: Vec::new(),
            selected_candidate_id: None,
            working_draft_id: draft_id,
        });
        self.record_builder_plan_event(
            crate::BuilderPlanEventKind::WorkingDraftCreated,
            None,
            None,
            "Created the persistent Pipeline Builder working Draft",
        );
    }

    pub fn record_pipeline_fragment(&mut self, fragment: crate::PipelineFragment) {
        if let Some(memory) = self.working_memory.as_mut() {
            memory
                .conversion_paths
                .insert(fragment.id.clone(), fragment.clone());
        }
        if !self.discovered_conversion_paths.contains(&fragment.id) {
            self.discovered_conversion_paths.push(fragment.id.clone());
        }
        self.record_builder_plan_event(
            crate::BuilderPlanEventKind::FragmentSaved,
            None,
            Some(fragment.id),
            "Saved a typed Artifact conversion path as a Pipeline Fragment",
        );
    }

    pub fn record_plan_candidate(&mut self, candidate: crate::PipelinePlanCandidate) {
        let candidate_id = candidate.id.clone();
        if let Some(existing) = self
            .plan_candidates
            .iter_mut()
            .find(|existing| existing.id == candidate_id)
        {
            *existing = candidate;
        } else {
            self.plan_candidates.push(candidate);
        }
        if let Some(memory) = self.working_memory.as_mut()
            && !memory.plan_candidate_ids.contains(&candidate_id)
        {
            memory.plan_candidate_ids.push(candidate_id.clone());
        }
        self.record_builder_plan_event(
            crate::BuilderPlanEventKind::CandidateSaved,
            Some(candidate_id),
            None,
            "Saved a Pipeline Plan Candidate",
        );
    }

    pub fn select_plan_candidate(&mut self, candidate_id: &str) -> Result<(), String> {
        if !self
            .plan_candidates
            .iter()
            .any(|candidate| candidate.id == candidate_id)
        {
            return Err(format!("unknown Pipeline Plan Candidate {candidate_id:?}"));
        }
        self.selected_candidate_id = Some(candidate_id.to_owned());
        if let Some(memory) = self.working_memory.as_mut() {
            memory.selected_candidate_id = Some(candidate_id.to_owned());
        }
        self.record_builder_plan_event(
            crate::BuilderPlanEventKind::CandidateSelected,
            Some(candidate_id.to_owned()),
            None,
            "Selected a Pipeline Plan Candidate",
        );
        Ok(())
    }

    pub fn record_builder_plan_event(
        &mut self,
        kind: crate::BuilderPlanEventKind,
        candidate_id: Option<crate::PlanCandidateId>,
        fragment_id: Option<crate::PipelineFragmentId>,
        detail: impl Into<String>,
    ) {
        self.planning_events.push(crate::BuilderPlanEvent {
            sequence: u32::try_from(self.planning_events.len() + 1).unwrap_or(u32::MAX),
            kind,
            candidate_id,
            fragment_id,
            detail: detail.into(),
            created_at: Utc::now(),
        });
        self.updated_at = Utc::now();
    }

    pub fn transition_builder_phase(
        &mut self,
        next: crate::PipelineBuilderPhase,
        next_action: impl Into<String>,
    ) -> Result<(), String> {
        let current = self
            .phase
            .ok_or_else(|| "Pipeline Builder progress is not initialized".to_owned())?;
        if !current.can_transition_to(next) {
            return Err(format!(
                "invalid Pipeline Builder phase transition {current:?} -> {next:?}"
            ));
        }
        if current != next {
            self.phase = Some(next);
            self.phase_tool_calls = 0;
        }
        self.next_action = Some(next_action.into());
        self.updated_at = Utc::now();
        Ok(())
    }

    #[must_use]
    pub fn remaining_builder_tool_calls(&self) -> u32 {
        self.builder_budget
            .as_ref()
            .map_or_else(
                || self.budget.max_tool_calls,
                |budget| budget.max_total_tool_calls,
            )
            .saturating_sub(self.usage.tool_calls)
    }

    pub fn set_builder_draft(&mut self, draft_id: impl Into<String>) {
        let draft_id = draft_id.into();
        self.draft_id = Some(draft_id.clone());
        if let Some(working_draft) = self.working_draft.as_mut() {
            working_draft.draft_id.clone_from(&draft_id);
            working_draft.updated_at = Utc::now();
        }
        if let Some(working_memory) = self.working_memory.as_mut() {
            working_memory.working_draft_id = draft_id;
        }
        self.updated_at = Utc::now();
    }

    pub fn complete_builder(
        &mut self,
        outcome: crate::PipelineBuilderOutcome,
        reason: crate::BuilderStopReason,
        next_action: impl Into<String>,
    ) {
        self.outcome = Some(outcome);
        self.builder_stop_reason = Some(reason);
        self.next_action = Some(next_action.into());
        match outcome {
            crate::PipelineBuilderOutcome::DraftReadyForHumanReview => {
                self.status = AgentSessionStatus::WaitingForHuman;
                self.phase = Some(crate::PipelineBuilderPhase::WaitingForHuman);
                self.pending_human_action = Some("approve_pipeline_draft".to_owned());
            }
            crate::PipelineBuilderOutcome::BlockedDraftReady
            | crate::PipelineBuilderOutcome::ProviderSetupRequired => {
                self.status = AgentSessionStatus::WaitingForHuman;
                self.phase = Some(crate::PipelineBuilderPhase::WaitingForHuman);
                self.pending_human_action = Some("configure_model_binding".to_owned());
            }
            crate::PipelineBuilderOutcome::UnsupportedRequest => {
                self.status = AgentSessionStatus::Succeeded;
                self.phase = Some(crate::PipelineBuilderPhase::Completed);
                self.pending_human_action = None;
            }
            crate::PipelineBuilderOutcome::Cancelled => {
                self.status = AgentSessionStatus::Cancelled;
                self.phase = Some(crate::PipelineBuilderPhase::Cancelled);
                self.pending_human_action = None;
            }
            crate::PipelineBuilderOutcome::BudgetExceeded => {
                self.status = AgentSessionStatus::BudgetExceeded;
                self.phase = Some(crate::PipelineBuilderPhase::Failed);
                self.pending_human_action = None;
            }
            crate::PipelineBuilderOutcome::Failed => {
                self.status = AgentSessionStatus::Failed;
                self.phase = Some(crate::PipelineBuilderPhase::Failed);
                self.pending_human_action = None;
            }
        }
        self.stop_reason = Some(format!("{reason:?}"));
        self.sync_builder_progress_counters();
        self.updated_at = Utc::now();
    }

    #[must_use]
    pub fn with_run(mut self, run_id: crate::RunId) -> Self {
        self.run_id = Some(run_id);
        self
    }

    pub fn record_tool(
        &mut self,
        tool_name: impl Into<String>,
        arguments: serde_json::Value,
        result: serde_json::Value,
        success: bool,
    ) -> Result<(), String> {
        if self.status != AgentSessionStatus::Running {
            return Err("cannot record a tool after the Agent session stopped".to_owned());
        }
        if self.usage.steps >= self.budget.max_steps
            || self.usage.tool_calls >= self.budget.max_tool_calls
        {
            self.stop_budget("step or tool-call budget exhausted");
            return Err("Agent budget exhausted".to_owned());
        }
        let now = Utc::now();
        self.usage.steps += 1;
        self.usage.tool_calls += 1;
        self.sync_builder_progress_counters();
        if self.phase.is_some() {
            self.phase_tool_calls = self.phase_tool_calls.saturating_add(1);
        }
        self.steps.push(AgentToolStep {
            sequence: self.usage.steps,
            call_id: format!("{}:{}", self.id, self.usage.steps),
            tool_name: tool_name.into(),
            arguments,
            result,
            success,
            started_at: now,
            finished_at: Utc::now(),
        });
        self.updated_at = Utc::now();
        Ok(())
    }

    fn sync_builder_progress_counters(&mut self) {
        self.total_tool_calls = self.usage.tool_calls;
        self.remaining_tool_calls = self.remaining_builder_tool_calls();
        self.reserved_finalization_calls = self
            .builder_budget
            .as_ref()
            .map_or(0, |budget| budget.reserved_finalization_calls);
    }

    pub fn add_model_usage(&mut self, input_tokens: u64, output_tokens: u64, cost: Decimal) {
        self.usage.input_tokens = self.usage.input_tokens.saturating_add(input_tokens);
        self.usage.output_tokens = self.usage.output_tokens.saturating_add(output_tokens);
        self.usage.cost += cost;
        let total = self
            .usage
            .input_tokens
            .saturating_add(self.usage.output_tokens);
        if self.budget.max_tokens.is_some_and(|limit| total > limit)
            || self
                .budget
                .max_cost
                .is_some_and(|limit| self.usage.cost > limit)
        {
            self.stop_budget("token or cost budget exhausted");
        }
    }

    pub fn record_model_call(&mut self, mut call: AgentModelCall) {
        call.sequence = u32::try_from(self.model_calls.len())
            .unwrap_or(u32::MAX)
            .saturating_add(1);
        self.add_model_usage(call.input_tokens, call.output_tokens, call.cost);
        self.model_calls.push(call);
        if self.phase.is_some() {
            self.model_turns = self.model_turns.saturating_add(1);
        }
        self.updated_at = Utc::now();
    }

    pub fn wait_for_human(&mut self, action: impl Into<String>) {
        self.status = AgentSessionStatus::WaitingForHuman;
        self.pending_human_action = Some(action.into());
        self.stop_reason = Some("explicit human approval is required".to_owned());
        self.updated_at = Utc::now();
    }

    pub fn cancel(&mut self) {
        self.status = AgentSessionStatus::Cancelled;
        if self.phase.is_some() {
            self.phase = Some(crate::PipelineBuilderPhase::Cancelled);
            self.outcome = Some(crate::PipelineBuilderOutcome::Cancelled);
            self.builder_stop_reason = Some(crate::BuilderStopReason::Cancelled);
            self.next_action = Some("The saved Draft is unchanged".to_owned());
        }
        self.pending_human_action = None;
        self.stop_reason = Some("cancelled by operator".to_owned());
        self.sync_builder_progress_counters();
        self.updated_at = Utc::now();
    }

    pub fn fail(&mut self, reason: impl Into<String>) {
        self.status = AgentSessionStatus::Failed;
        if self.phase.is_some() {
            self.phase = Some(crate::PipelineBuilderPhase::Failed);
            self.outcome = Some(crate::PipelineBuilderOutcome::Failed);
        }
        self.stop_reason = Some(reason.into());
        self.sync_builder_progress_counters();
        self.updated_at = Utc::now();
    }

    pub fn succeed(&mut self, reason: impl Into<String>) {
        self.status = AgentSessionStatus::Succeeded;
        self.stop_reason = Some(reason.into());
        self.updated_at = Utc::now();
    }

    fn stop_budget(&mut self, reason: &str) {
        self.status = AgentSessionStatus::BudgetExceeded;
        if self.phase.is_some() {
            self.phase = Some(crate::PipelineBuilderPhase::Failed);
            self.outcome = Some(crate::PipelineBuilderOutcome::BudgetExceeded);
            self.builder_stop_reason = Some(crate::BuilderStopReason::TotalToolBudgetReached);
            self.next_action =
                Some("Open the latest saved Draft and retry from that state".to_owned());
        }
        self.stop_reason = Some(reason.to_owned());
        self.sync_builder_progress_counters();
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_enforces_tool_budget_and_human_stop() {
        let mut session = AgentSession::start(
            AgentKind::WorkflowAdvisor,
            AgentBudget {
                max_steps: 1,
                max_tool_calls: 1,
                ..AgentBudget::default()
            },
        );
        session
            .record_tool(
                "inspect",
                serde_json::json!({}),
                serde_json::json!({}),
                true,
            )
            .expect("first tool");
        assert!(
            session
                .record_tool("again", serde_json::json!({}), serde_json::json!({}), true)
                .is_err()
        );
        assert_eq!(session.status, AgentSessionStatus::BudgetExceeded);

        let mut approval = AgentSession::start(AgentKind::WorkflowAdvisor, AgentBudget::default());
        approval.wait_for_human("publish_workflow");
        assert_eq!(approval.status, AgentSessionStatus::WaitingForHuman);
        approval.cancel();
        assert_eq!(approval.status, AgentSessionStatus::Cancelled);
        assert!(approval.pending_human_action.is_none());

        let policy = DetectionRecoveryPolicy {
            max_fallback_calls: 2,
            ..DetectionRecoveryPolicy::default()
        };
        assert!(
            policy
                .validate()
                .expect_err("Alpha fallback limit")
                .contains("at most one")
        );
    }

    #[test]
    fn builder_working_plan_round_trips_fragments_candidates_and_selection() {
        let mut session = AgentSession::start(AgentKind::PipelineBuilder, AgentBudget::default())
            .with_build_mode(crate::PipelineBuildMode::FromScratch);
        session.set_builder_working_draft(
            "working-draft",
            crate::PipelineBuildMode::FromScratch,
            "registry-revision-1",
        );
        let now = Utc::now();
        let fragment = crate::PipelineFragment {
            id: "fragment-1".to_owned(),
            context_revision: "registry-revision-1".to_owned(),
            from_artifact: crate::ArtifactKind::DetectionSet,
            to_artifact: crate::ArtifactKind::DetectionSet,
            node_blueprints: Vec::new(),
            edge_blueprints: Vec::new(),
            required_model_capabilities: std::collections::BTreeSet::from([
                crate::ModelCapability::PromptedSegmentation,
            ]),
            required_skills: std::collections::BTreeSet::new(),
            source_observation_ids: vec!["tool-call-3".to_owned()],
            created_at: now,
        };
        session.record_pipeline_fragment(fragment);
        let candidate = crate::PipelinePlanCandidate {
            id: "candidate-1".to_owned(),
            name: "Typed geometry refinement".to_owned(),
            source: crate::PipelineCandidateSource::ConversionPath,
            status: crate::PipelineCandidateStatus::Runnable,
            sufficiency: crate::CandidateSufficiency::Complete,
            fragment_ids: vec!["fragment-1".to_owned()],
            node_blueprints: Vec::new(),
            edge_blueprints: Vec::new(),
            model_bindings: Vec::new(),
            skill_bindings: Vec::new(),
            evidence: Vec::new(),
            unresolved_bindings: Vec::new(),
            output_artifact: crate::ArtifactKind::DetectionSet,
            geometry_safety: crate::CandidateGeometrySafety::Evaluated,
            has_review_path: true,
            has_commit_path: true,
            registry_revision: "registry-revision-1".to_owned(),
            score: crate::PlanCandidateScore {
                runnable: true,
                deterministic_total: 100,
                ..crate::PlanCandidateScore::default()
            },
            created_at: now,
            updated_at: now,
        };
        session.record_plan_candidate(candidate);
        session
            .select_plan_candidate("candidate-1")
            .expect("candidate selection");

        let restored: AgentSession = serde_json::from_str(
            &serde_json::to_string(&session).expect("serialize Builder Session"),
        )
        .expect("restore Builder Session");
        assert_eq!(
            restored.build_mode,
            Some(crate::PipelineBuildMode::FromScratch)
        );
        assert_eq!(
            restored
                .working_draft
                .as_ref()
                .map(|working| working.draft_id.as_str()),
            Some("working-draft")
        );
        assert_eq!(restored.discovered_conversion_paths, ["fragment-1"]);
        assert_eq!(
            restored.selected_candidate_id.as_deref(),
            Some("candidate-1")
        );
        assert_eq!(
            restored
                .working_memory
                .as_ref()
                .map(|memory| memory.conversion_paths.len()),
            Some(1)
        );
        assert_eq!(restored.planning_events.len(), 4);
    }
}
