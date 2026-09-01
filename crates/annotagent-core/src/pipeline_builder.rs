//! Constrained, auditable primitives for the Lean Pipeline Builder Agent.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AgentBudget, AgentKind, AgentSession, AgentSessionStatus, AgentUsage, CoreError, CoreResult,
    ModelProfile, ModelProfileStatus, ModelRegistry, NodeRegistry, ProviderAdapterKind,
    ProviderHealthStatus, ProviderId, StoredPayloadRef, ValidationCatalog, VisionBackendKind,
    WorkflowDraft, WorkflowDraftNode, WorkflowDraftStatus, WorkflowEdge, WorkflowModelBinding,
    WorkflowNodeKind, WorkflowStaticValidator, WorkflowValidationIssue, WorkflowValidationReport,
};

pub const PIPELINE_BUILDER_PROTOCOL_VERSION: u32 = 1;

/// Credential-safe Provider information allowed into the Builder model context. In particular,
/// this deliberately has no credential reference, locator, headers, or secret-bearing URL path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineBuilderProviderProfile {
    pub id: ProviderId,
    pub display_name: String,
    pub adapter: ProviderAdapterKind,
    pub endpoint_summary: String,
    pub enabled: bool,
    pub health_status: ProviderHealthStatus,
    pub credential_configured: bool,
    pub model_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineAdvisorBackend {
    Llm,
    RuleBased,
    ScriptedMock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationPriority {
    Fast,
    #[default]
    Balanced,
    Accurate,
    LowCost,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeDiff {
    pub change_id: String,
    pub node_id: String,
    pub node_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeParameterDiff {
    pub change_id: String,
    pub node_id: String,
    pub before: BTreeMap<String, serde_json::Value>,
    pub after: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeDiff {
    pub change_id: String,
    pub edge: WorkflowEdge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelBindingDiff {
    pub change_id: String,
    pub node_id: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub before_profile: Option<WorkflowModelBinding>,
    pub after_profile: Option<WorkflowModelBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyDiff {
    pub change_id: String,
    pub node_id: String,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PipelineDraftDiff {
    pub added_nodes: Vec<NodeDiff>,
    pub removed_nodes: Vec<NodeDiff>,
    pub modified_nodes: Vec<NodeParameterDiff>,
    pub added_edges: Vec<EdgeDiff>,
    pub removed_edges: Vec<EdgeDiff>,
    pub model_binding_changes: Vec<ModelBindingDiff>,
    pub policy_changes: Vec<PolicyDiff>,
}

impl PipelineDraftDiff {
    pub fn between(base: &WorkflowDraft, proposed: &WorkflowDraft) -> CoreResult<Self> {
        if base.project_id != proposed.project_id {
            return Err(CoreError::Validation(
                "Pipeline Draft Diff requires Drafts from the same Project".to_owned(),
            ));
        }
        let base_nodes = base
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<BTreeMap<_, _>>();
        let proposed_nodes = proposed
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<BTreeMap<_, _>>();
        let mut diff = Self::default();
        for node in &proposed.nodes {
            let Some(previous) = base_nodes.get(node.id.as_str()) else {
                diff.added_nodes.push(NodeDiff {
                    change_id: format!("node:add:{}", node.id),
                    node_id: node.id.clone(),
                    node_type: node.node_type.clone(),
                });
                continue;
            };
            if !node_structure_equal(previous, node) {
                diff.modified_nodes.push(NodeParameterDiff {
                    change_id: format!("node:structure:{}", node.id),
                    node_id: node.id.clone(),
                    before: node_structure(previous),
                    after: node_structure(node),
                });
            }
            if previous.parameters != node.parameters {
                diff.modified_nodes.push(NodeParameterDiff {
                    change_id: format!("node:parameters:{}", node.id),
                    node_id: node.id.clone(),
                    before: previous.parameters.clone(),
                    after: node.parameters.clone(),
                });
            }
            if previous.model_binding != node.model_binding
                || previous.model_profile_binding != node.model_profile_binding
            {
                diff.model_binding_changes.push(ModelBindingDiff {
                    change_id: format!("node:model:{}", node.id),
                    node_id: node.id.clone(),
                    before: previous.model_binding.clone(),
                    after: node.model_binding.clone(),
                    before_profile: previous.model_profile_binding,
                    after_profile: node.model_profile_binding,
                });
            }
            let previous_policy = node_policy(previous);
            let proposed_policy = node_policy(node);
            if previous_policy != proposed_policy {
                diff.policy_changes.push(PolicyDiff {
                    change_id: format!("node:policy:{}", node.id),
                    node_id: node.id.clone(),
                    before: previous_policy,
                    after: proposed_policy,
                });
            }
        }
        for node in &base.nodes {
            if !proposed_nodes.contains_key(node.id.as_str()) {
                diff.removed_nodes.push(NodeDiff {
                    change_id: format!("node:remove:{}", node.id),
                    node_id: node.id.clone(),
                    node_type: node.node_type.clone(),
                });
            }
        }
        for edge in &proposed.edges {
            if !base.edges.contains(edge) {
                diff.added_edges.push(EdgeDiff {
                    change_id: edge_change_id("add", edge),
                    edge: edge.clone(),
                });
            }
        }
        for edge in &base.edges {
            if !proposed.edges.contains(edge) {
                diff.removed_edges.push(EdgeDiff {
                    change_id: edge_change_id("remove", edge),
                    edge: edge.clone(),
                });
            }
        }
        if base.runtime_policies != proposed.runtime_policies {
            diff.policy_changes.push(PolicyDiff {
                change_id: "workflow:runtime_policies".to_owned(),
                node_id: "$workflow".to_owned(),
                before: serde_json::json!(base.runtime_policies),
                after: serde_json::json!(proposed.runtime_policies),
            });
        }
        Ok(diff)
    }

    #[must_use]
    pub fn all_change_ids(&self) -> BTreeSet<String> {
        self.added_nodes
            .iter()
            .chain(&self.removed_nodes)
            .map(|change| change.change_id.clone())
            .chain(
                self.modified_nodes
                    .iter()
                    .map(|change| change.change_id.clone()),
            )
            .chain(
                self.added_edges
                    .iter()
                    .chain(&self.removed_edges)
                    .map(|change| change.change_id.clone()),
            )
            .chain(
                self.model_binding_changes
                    .iter()
                    .map(|change| change.change_id.clone()),
            )
            .chain(
                self.policy_changes
                    .iter()
                    .map(|change| change.change_id.clone()),
            )
            .collect()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.all_change_ids().is_empty()
    }

    pub fn apply_selected(
        &self,
        base: &WorkflowDraft,
        proposed: &WorkflowDraft,
        selected_change_ids: &BTreeSet<String>,
    ) -> CoreResult<WorkflowDraft> {
        ensure_mutable(base)?;
        if base.project_id != proposed.project_id {
            return Err(CoreError::Validation(
                "Pipeline Draft changes cannot cross Project boundaries".to_owned(),
            ));
        }
        let known = self.all_change_ids();
        if let Some(unknown) = selected_change_ids.difference(&known).next() {
            return Err(CoreError::Validation(format!(
                "unknown Pipeline Draft change {unknown:?}"
            )));
        }
        if selected_change_ids.is_empty() {
            return Err(CoreError::Validation(
                "select at least one Pipeline Draft change to apply".to_owned(),
            ));
        }
        if *selected_change_ids == known {
            return Ok(applied_identity(base, proposed.clone()));
        }

        let mut applied = base.clone();
        for change in &self.removed_nodes {
            if selected_change_ids.contains(&change.change_id) {
                applied.nodes.retain(|node| node.id != change.node_id);
                applied.edges.retain(|edge| {
                    edge.from_node != change.node_id && edge.to_node != change.node_id
                });
            }
        }
        for change in &self.added_nodes {
            if selected_change_ids.contains(&change.change_id)
                && let Some(node) = proposed.nodes.iter().find(|node| node.id == change.node_id)
            {
                applied.nodes.push(node.clone());
            }
        }
        for change in &self.modified_nodes {
            if !selected_change_ids.contains(&change.change_id) {
                continue;
            }
            let Some(target) = applied
                .nodes
                .iter_mut()
                .find(|node| node.id == change.node_id)
            else {
                continue;
            };
            let Some(source) = proposed.nodes.iter().find(|node| node.id == change.node_id) else {
                continue;
            };
            if change.change_id.starts_with("node:parameters:") {
                target.parameters = source.parameters.clone();
            } else {
                copy_node_structure(target, source);
            }
        }
        for change in &self.model_binding_changes {
            if selected_change_ids.contains(&change.change_id)
                && let Some(node) = applied
                    .nodes
                    .iter_mut()
                    .find(|node| node.id == change.node_id)
            {
                node.model_binding.clone_from(&change.after);
                node.model_profile_binding = change.after_profile;
            }
        }
        for change in &self.policy_changes {
            if selected_change_ids.contains(&change.change_id)
                && change.change_id == "workflow:runtime_policies"
            {
                applied
                    .runtime_policies
                    .clone_from(&proposed.runtime_policies);
            } else if selected_change_ids.contains(&change.change_id)
                && let (Some(target), Some(source)) = (
                    applied
                        .nodes
                        .iter_mut()
                        .find(|node| node.id == change.node_id),
                    proposed.nodes.iter().find(|node| node.id == change.node_id),
                )
            {
                copy_node_policy(target, source);
            }
        }
        for change in &self.removed_edges {
            if selected_change_ids.contains(&change.change_id) {
                applied.edges.retain(|edge| edge != &change.edge);
            }
        }
        for change in &self.added_edges {
            if selected_change_ids.contains(&change.change_id)
                && !applied.edges.contains(&change.edge)
            {
                applied.edges.push(change.edge.clone());
            }
        }
        let node_ids = applied
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<BTreeSet<_>>();
        applied.edges.retain(|edge| {
            node_ids.contains(edge.from_node.as_str()) && node_ids.contains(edge.to_node.as_str())
        });
        for node in &mut applied.nodes {
            node.depends_on = applied
                .edges
                .iter()
                .filter(|edge| edge.to_node == node.id)
                .map(|edge| edge.from_node.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
        }
        // A partial technical selection may no longer map losslessly to the original Label authoring
        // projection. The Guided recipe remains available from flat node types, while Expert editing
        // uses the exact selected DAG.
        applied.label_pipeline = None;
        applied.status = WorkflowDraftStatus::Editing;
        applied.updated_at = Utc::now();
        Ok(applied)
    }
}

fn applied_identity(base: &WorkflowDraft, mut proposed: WorkflowDraft) -> WorkflowDraft {
    proposed.id.clone_from(&base.id);
    proposed.project_id.clone_from(&base.project_id);
    proposed.created_at = base.created_at;
    proposed.status = WorkflowDraftStatus::Editing;
    proposed.updated_at = Utc::now();
    proposed
}

fn edge_change_id(action: &str, edge: &WorkflowEdge) -> String {
    format!(
        "edge:{action}:{}:{}:{}:{}:{}",
        edge.from_node,
        edge.from_port,
        edge.to_node,
        edge.to_port,
        edge.route.as_deref().unwrap_or("default")
    )
}

fn node_structure_equal(left: &WorkflowDraftNode, right: &WorkflowDraftNode) -> bool {
    left.node_type == right.node_type
        && left.kind == right.kind
        && left.inputs == right.inputs
        && left.outputs == right.outputs
        && left.required_skills == right.required_skills
}

fn node_structure(node: &WorkflowDraftNode) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("node_type".to_owned(), serde_json::json!(node.node_type)),
        ("kind".to_owned(), serde_json::json!(node.kind)),
        ("inputs".to_owned(), serde_json::json!(node.inputs)),
        ("outputs".to_owned(), serde_json::json!(node.outputs)),
        (
            "required_skills".to_owned(),
            serde_json::json!(node.required_skills),
        ),
    ])
}

fn node_policy(node: &WorkflowDraftNode) -> serde_json::Value {
    serde_json::json!({
        "validators": node.validators,
        "refiners": node.refiners,
        "fallback": node.fallback,
        "max_retries": node.max_retries,
        "review_gate": node.review_gate,
        "retry_policy": node.retry_policy,
        "fallback_policy": node.fallback_policy,
        "gate": node.gate,
        "resources": node.resources,
    })
}

fn copy_node_structure(target: &mut WorkflowDraftNode, source: &WorkflowDraftNode) {
    target.node_type.clone_from(&source.node_type);
    target.kind = source.kind;
    target.inputs.clone_from(&source.inputs);
    target.outputs.clone_from(&source.outputs);
    target.required_skills.clone_from(&source.required_skills);
}

fn copy_node_policy(target: &mut WorkflowDraftNode, source: &WorkflowDraftNode) {
    target.validators.clone_from(&source.validators);
    target.refiners.clone_from(&source.refiners);
    target.fallback.clone_from(&source.fallback);
    target.max_retries = source.max_retries;
    target.review_gate = source.review_gate;
    target.retry_policy = source.retry_policy;
    target.fallback_policy.clone_from(&source.fallback_policy);
    target.gate = source.gate;
    target.resources.clone_from(&source.resources);
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PipelineBuilderConstraints {
    pub priority: OptimizationPriority,
    pub max_cost_per_image: Option<Decimal>,
    pub max_model_calls_per_image: Option<u32>,
    pub max_expected_latency_ms: Option<u64>,
    pub target_review_rate: Option<f32>,
    pub allow_external_models: bool,
    pub allow_human_review: bool,
    pub maximum_agent_turns: u32,
    pub maximum_tool_calls: u32,
    pub maximum_dry_runs: u32,
    pub maximum_agent_cost: Decimal,
}

impl Default for PipelineBuilderConstraints {
    fn default() -> Self {
        Self {
            priority: OptimizationPriority::Balanced,
            max_cost_per_image: None,
            max_model_calls_per_image: Some(4),
            max_expected_latency_ms: None,
            target_review_rate: Some(0.25),
            allow_external_models: false,
            allow_human_review: true,
            maximum_agent_turns: 16,
            maximum_tool_calls: 48,
            maximum_dry_runs: 3,
            maximum_agent_cost: Decimal::ONE,
        }
    }
}

impl PipelineBuilderConstraints {
    pub fn validate(&self) -> CoreResult<()> {
        if self.maximum_agent_turns == 0
            || self.maximum_tool_calls == 0
            || self.maximum_dry_runs == 0
        {
            return Err(CoreError::Validation(
                "Pipeline Builder turn, tool-call, and Dry Run limits must be greater than zero"
                    .to_owned(),
            ));
        }
        if self.maximum_agent_cost < Decimal::ZERO
            || self
                .max_cost_per_image
                .is_some_and(|cost| cost < Decimal::ZERO)
        {
            return Err(CoreError::Validation(
                "Pipeline Builder cost limits cannot be negative".to_owned(),
            ));
        }
        if self
            .target_review_rate
            .is_some_and(|rate| !rate.is_finite() || !(0.0..=1.0).contains(&rate))
        {
            return Err(CoreError::Validation(
                "target_review_rate must be within [0,1]".to_owned(),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn agent_budget(&self) -> AgentBudget {
        AgentBudget {
            // `AgentSession` records one step per Tool Call. Provider turns are enforced by the
            // Pipeline Builder loop itself, so mapping them onto `max_steps` would accidentally
            // stop a valid multi-tool turn before the independent Tool Call budget is exhausted.
            max_steps: self.maximum_tool_calls,
            max_tool_calls: self.maximum_tool_calls,
            max_tokens: None,
            max_cost: Some(self.maximum_agent_cost),
        }
    }
}

/// Durable execution phase for the constrained Pipeline Builder. The phase is persisted on the
/// Agent Session so a UI or retry can distinguish discovery from draft work and finalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PipelineBuilderPhase {
    #[default]
    ContextLoading,
    FeasibilityAnalysis,
    Drafting,
    Validating,
    DryRunning,
    Revising,
    Finalizing,
    WaitingForHuman,
    Completed,
    Cancelled,
    Failed,
}

impl PipelineBuilderPhase {
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        matches!(
            (self, next),
            (Self::ContextLoading, Self::FeasibilityAnalysis)
                | (Self::FeasibilityAnalysis, Self::Drafting | Self::Completed)
                | (
                    Self::Drafting | Self::Revising,
                    Self::Validating | Self::Finalizing | Self::Failed
                )
                | (
                    Self::Validating,
                    Self::DryRunning | Self::Revising | Self::Finalizing | Self::Failed
                )
                | (
                    Self::DryRunning,
                    Self::Revising | Self::Finalizing | Self::Failed
                )
                | (
                    Self::Finalizing,
                    Self::WaitingForHuman | Self::Completed | Self::Failed
                )
                | (_, Self::Cancelled)
        )
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::WaitingForHuman | Self::Completed | Self::Cancelled | Self::Failed
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineBuilderOutcome {
    DraftReadyForHumanReview,
    BlockedDraftReady,
    ProviderSetupRequired,
    UnsupportedRequest,
    Cancelled,
    BudgetExceeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderStopReason {
    DraftReady,
    SetupRequired,
    UnsupportedRequest,
    DiscoveryLimitReached,
    DraftDeadlineReached,
    ValidationRepairLimitReached,
    DryRunLimitReached,
    TotalToolBudgetReached,
    ModelTurnBudgetReached,
    TokenBudgetReached,
    CostBudgetReached,
    Cancelled,
    ProviderError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PipelineBuilderBudget {
    pub max_model_turns: u32,
    pub max_total_tool_calls: u32,
    pub max_discovery_tool_calls: u32,
    pub max_draft_tool_calls: u32,
    pub max_validation_tool_calls: u32,
    pub max_dry_run_tool_calls: u32,
    pub reserved_finalization_calls: u32,
    pub max_parallel_tools_per_turn: u32,
    pub max_duplicate_calls: u32,
}

impl Default for PipelineBuilderBudget {
    fn default() -> Self {
        Self {
            max_model_turns: 16,
            max_total_tool_calls: 48,
            max_discovery_tool_calls: 10,
            max_draft_tool_calls: 10,
            max_validation_tool_calls: 10,
            max_dry_run_tool_calls: 10,
            reserved_finalization_calls: 6,
            max_parallel_tools_per_turn: 4,
            max_duplicate_calls: 1,
        }
    }
}

impl PipelineBuilderBudget {
    #[must_use]
    pub fn from_constraints(constraints: &PipelineBuilderConstraints) -> Self {
        let mut budget = Self {
            max_model_turns: constraints.maximum_agent_turns,
            max_total_tool_calls: constraints.maximum_tool_calls,
            ..Self::default()
        };
        budget.reserved_finalization_calls = budget
            .reserved_finalization_calls
            .min(budget.max_total_tool_calls.saturating_sub(1));
        let discovery_capacity = budget
            .max_total_tool_calls
            .saturating_sub(budget.reserved_finalization_calls);
        budget.max_discovery_tool_calls = budget
            .max_discovery_tool_calls
            .min(discovery_capacity)
            .min(budget.max_total_tool_calls / 4);
        budget
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.max_model_turns == 0
            || self.max_total_tool_calls == 0
            || self.max_parallel_tools_per_turn == 0
        {
            return Err(CoreError::Validation(
                "Pipeline Builder model, Tool Call, and parallel limits must be greater than zero"
                    .to_owned(),
            ));
        }
        if self.reserved_finalization_calls >= self.max_total_tool_calls {
            return Err(CoreError::Validation(
                "Pipeline Builder finalization reserve must leave at least one non-finalization Tool Call"
                    .to_owned(),
            ));
        }
        if self.max_discovery_tool_calls
            > self
                .max_total_tool_calls
                .saturating_sub(self.reserved_finalization_calls)
        {
            return Err(CoreError::Validation(
                "Pipeline Builder discovery budget cannot consume the finalization reserve"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub const fn remaining(&self, used: u32) -> u32 {
        self.max_total_tool_calls.saturating_sub(used)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuilderProgressInvariant {
    pub context_deadline_tool_call: u32,
    pub feasibility_deadline_tool_call: u32,
    pub draft_deadline_tool_call: u32,
    pub maximum_validation_repairs: u32,
    pub maximum_dry_runs: u32,
}

impl Default for BuilderProgressInvariant {
    fn default() -> Self {
        Self {
            context_deadline_tool_call: 6,
            feasibility_deadline_tool_call: 10,
            draft_deadline_tool_call: 12,
            maximum_validation_repairs: 2,
            maximum_dry_runs: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailableAgentActions {
    pub phase: PipelineBuilderPhase,
    pub tools: Vec<String>,
    pub remaining_tool_calls: u32,
    pub reserved_finalization_calls: u32,
    pub required_next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectBuildSummary {
    pub project_id: String,
    pub display_name: String,
    pub image_count: usize,
    pub task_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelBuildSummary {
    pub task_id: String,
    pub label: String,
    pub annotation_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSummary {
    pub id: String,
    pub resource_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSummary {
    pub id: String,
    pub display_name: String,
    pub input_artifacts: Vec<String>,
    pub output_artifacts: Vec<String>,
    pub required_model_capability: Option<String>,
    pub required_protocol_features: Vec<String>,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCompatibilitySummary {
    pub model_profile_id: String,
    pub display_name: String,
    pub modalities: Vec<String>,
    pub task_capabilities: Vec<String>,
    pub protocol_features: Vec<String>,
    pub health: String,
    pub credential_configured: bool,
    pub compatible_node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftSummary {
    pub id: String,
    pub name: String,
    pub status: String,
    pub node_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineTemplateSummary {
    pub id: String,
    pub name: String,
    pub node_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRequirement {
    pub node_id: String,
    pub capability: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CapabilityMatrix {
    pub node_to_model_profiles: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineBuilderContextSnapshot {
    pub project: ProjectBuildSummary,
    pub target_labels: Vec<LabelBuildSummary>,
    pub enabled_skills: Vec<SkillSummary>,
    pub node_catalog: Vec<NodeSummary>,
    pub model_profiles: Vec<ModelCompatibilitySummary>,
    pub existing_drafts: Vec<DraftSummary>,
    pub templates: Vec<PipelineTemplateSummary>,
    pub capability_matrix: CapabilityMatrix,
    pub unavailable_capabilities: Vec<CapabilityRequirement>,
    pub context_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationRef {
    pub id: String,
    pub tool_name: String,
    pub original_call_id: String,
    pub context_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BuilderContextDigest {
    pub context_revision: String,
    pub project_summary: String,
    pub capability_summary: String,
    pub model_summary: String,
    pub draft_summary: Option<String>,
    pub validation_summary: Option<String>,
    pub dry_run_summary: Option<String>,
    pub observation_refs: Vec<ObservationRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupActionKind {
    ConfigureProvider,
    AddModelProfile,
    SupplyWeights,
    EnableModel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupAction {
    pub kind: SetupActionKind,
    pub label: String,
    pub route: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedModelRequirement {
    pub id: String,
    pub node_id: String,
    pub required_capabilities: Vec<crate::ModelCapability>,
    pub required_modalities: Vec<crate::InputModality>,
    pub required_protocol_features: Vec<String>,
    pub reason: String,
    pub compatible_profiles: Vec<crate::ModelProfileId>,
    pub setup_actions: Vec<SetupAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildFeasibility {
    Runnable {
        candidate_templates: Vec<String>,
        compatible_bindings: Vec<String>,
        warnings: Vec<String>,
    },
    RunnableWithDegradedQuality {
        candidate_templates: Vec<String>,
        warnings: Vec<String>,
    },
    BlockedByBindings {
        requirements: Vec<UnresolvedModelRequirement>,
        candidate_templates: Vec<String>,
    },
    Unsupported {
        missing_nodes: Vec<String>,
        missing_conversion_paths: Vec<String>,
        reasons: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineBuilderStatus {
    Inspecting,
    Drafting,
    Validating,
    Testing,
    Revising,
    WaitingForHuman,
    Completed,
    Cancelled,
    BudgetExceeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineBuilderStopReason {
    DraftReadyForHumanReview,
    HumanInputRequired,
    NoCompatibleModel,
    ValidationCouldNotBeResolved,
    DryRunTargetNotReached,
    MaximumTurnsReached,
    MaximumToolCallsReached,
    MaximumDryRunsReached,
    BudgetExceeded,
    Cancelled,
    ProviderError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentToolResult {
    pub persisted_payload: Option<StoredPayloadRef>,
    pub model_payload: serde_json::Value,
    pub display_summary: String,
}

impl AgentToolResult {
    #[must_use]
    pub fn summary(summary: impl Into<String>, model_payload: serde_json::Value) -> Self {
        Self {
            persisted_payload: None,
            model_payload,
            display_summary: summary.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineBuilderTool {
    GetPipelineBuilderContext,
    ResolvePipelineFeasibility,
    InspectNodesBatch,
    InspectModelsBatch,
    InspectContractsBatch,
    CreateBlockedDraft,
    SetUnresolvedBinding,
    FinishWithSetupRequirements,
    InspectProject,
    InspectLabelSchema,
    InspectLabel,
    SampleDataset,
    InspectSampleImage,
    InspectExistingPipeline,
    InspectExistingAutomations,
    ListEnabledSkills,
    LoadSkillResource,
    ListNodeDefinitions,
    InspectNodeDefinition,
    FindArtifactConversionPath,
    ListPipelineTemplates,
    ListProviderProfiles,
    ListAvailableCapabilities,
    ListCompatibleModels,
    InspectModelProfile,
    InspectWorkerHealth,
    InspectModelContracts,
    InspectLabelSpace,
    InspectScoreSemantics,
    InspectGeometrySemantics,
    InspectModelQualityContract,
    InspectProjectGeometryPolicy,
    InspectGeometryCorrectionSummary,
    InspectGeometryCalibration,
    FindGeometryRefinementPath,
    CheckCapabilityPath,
    CheckProviderAvailability,
    EstimateModelCost,
    CreatePipelineDraft,
    CreateDraftFromTemplate,
    AddPipelineNode,
    RemovePipelineNode,
    ConnectPipelineNodes,
    DisconnectPipelineNodes,
    SetNodeConfiguration,
    BindModelProfile,
    SetLabelMapping,
    SetDecisionPolicy,
    SetRuntimePolicy,
    ComparePipelineDrafts,
    UndoLastDraftChange,
    ValidatePipeline,
    EstimatePipelineCost,
    DryRunPipeline,
    InspectDryRunSummary,
    InspectFailureClasses,
    InspectGeometryQuality,
    InspectFailedSamples,
    InspectReviewSamples,
    InspectNodeStatistics,
    InspectNodeArtifacts,
    CompareDryRuns,
    SubmitDraftForHumanApproval,
    FinishAgentSession,
}

impl PipelineBuilderTool {
    pub const ALL: [Self; 64] = [
        Self::GetPipelineBuilderContext,
        Self::ResolvePipelineFeasibility,
        Self::InspectNodesBatch,
        Self::InspectModelsBatch,
        Self::InspectContractsBatch,
        Self::CreateBlockedDraft,
        Self::SetUnresolvedBinding,
        Self::FinishWithSetupRequirements,
        Self::InspectProject,
        Self::InspectLabelSchema,
        Self::InspectLabel,
        Self::SampleDataset,
        Self::InspectSampleImage,
        Self::InspectExistingPipeline,
        Self::InspectExistingAutomations,
        Self::ListEnabledSkills,
        Self::LoadSkillResource,
        Self::ListNodeDefinitions,
        Self::InspectNodeDefinition,
        Self::FindArtifactConversionPath,
        Self::ListPipelineTemplates,
        Self::ListProviderProfiles,
        Self::ListAvailableCapabilities,
        Self::ListCompatibleModels,
        Self::InspectModelProfile,
        Self::InspectWorkerHealth,
        Self::InspectModelContracts,
        Self::InspectLabelSpace,
        Self::InspectScoreSemantics,
        Self::InspectGeometrySemantics,
        Self::InspectModelQualityContract,
        Self::InspectProjectGeometryPolicy,
        Self::InspectGeometryCorrectionSummary,
        Self::InspectGeometryCalibration,
        Self::FindGeometryRefinementPath,
        Self::CheckCapabilityPath,
        Self::CheckProviderAvailability,
        Self::EstimateModelCost,
        Self::CreatePipelineDraft,
        Self::CreateDraftFromTemplate,
        Self::AddPipelineNode,
        Self::RemovePipelineNode,
        Self::ConnectPipelineNodes,
        Self::DisconnectPipelineNodes,
        Self::SetNodeConfiguration,
        Self::BindModelProfile,
        Self::SetLabelMapping,
        Self::SetDecisionPolicy,
        Self::SetRuntimePolicy,
        Self::ComparePipelineDrafts,
        Self::UndoLastDraftChange,
        Self::ValidatePipeline,
        Self::EstimatePipelineCost,
        Self::DryRunPipeline,
        Self::InspectDryRunSummary,
        Self::InspectFailureClasses,
        Self::InspectGeometryQuality,
        Self::InspectFailedSamples,
        Self::InspectReviewSamples,
        Self::InspectNodeStatistics,
        Self::InspectNodeArtifacts,
        Self::CompareDryRuns,
        Self::SubmitDraftForHumanApproval,
        Self::FinishAgentSession,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GetPipelineBuilderContext => "get_pipeline_builder_context",
            Self::ResolvePipelineFeasibility => "resolve_pipeline_feasibility",
            Self::InspectNodesBatch => "inspect_nodes_batch",
            Self::InspectModelsBatch => "inspect_models_batch",
            Self::InspectContractsBatch => "inspect_contracts_batch",
            Self::CreateBlockedDraft => "create_blocked_draft",
            Self::SetUnresolvedBinding => "set_unresolved_binding",
            Self::FinishWithSetupRequirements => "finish_with_setup_requirements",
            Self::InspectProject => "inspect_project",
            Self::InspectLabelSchema => "inspect_label_schema",
            Self::InspectLabel => "inspect_label",
            Self::SampleDataset => "sample_dataset",
            Self::InspectSampleImage => "inspect_sample_image",
            Self::InspectExistingPipeline => "inspect_existing_pipeline",
            Self::InspectExistingAutomations => "inspect_existing_automations",
            Self::ListEnabledSkills => "list_enabled_skills",
            Self::LoadSkillResource => "load_skill_resource",
            Self::ListNodeDefinitions => "list_node_definitions",
            Self::InspectNodeDefinition => "inspect_node_definition",
            Self::FindArtifactConversionPath => "find_artifact_conversion_path",
            Self::ListPipelineTemplates => "list_pipeline_templates",
            Self::ListProviderProfiles => "list_provider_profiles",
            Self::ListAvailableCapabilities => "list_available_capabilities",
            Self::ListCompatibleModels => "list_compatible_models",
            Self::InspectModelProfile => "inspect_model_profile",
            Self::InspectWorkerHealth => "inspect_worker_health",
            Self::InspectModelContracts => "inspect_model_contracts",
            Self::InspectLabelSpace => "inspect_label_space",
            Self::InspectScoreSemantics => "inspect_score_semantics",
            Self::InspectGeometrySemantics => "inspect_geometry_semantics",
            Self::InspectModelQualityContract => "inspect_model_quality_contract",
            Self::InspectProjectGeometryPolicy => "inspect_project_geometry_policy",
            Self::InspectGeometryCorrectionSummary => "inspect_geometry_correction_summary",
            Self::InspectGeometryCalibration => "inspect_geometry_calibration",
            Self::FindGeometryRefinementPath => "find_geometry_refinement_path",
            Self::CheckCapabilityPath => "check_capability_path",
            Self::CheckProviderAvailability => "check_provider_availability",
            Self::EstimateModelCost => "estimate_model_cost",
            Self::CreatePipelineDraft => "create_pipeline_draft",
            Self::CreateDraftFromTemplate => "create_draft_from_template",
            Self::AddPipelineNode => "add_pipeline_node",
            Self::RemovePipelineNode => "remove_pipeline_node",
            Self::ConnectPipelineNodes => "connect_pipeline_nodes",
            Self::DisconnectPipelineNodes => "disconnect_pipeline_nodes",
            Self::SetNodeConfiguration => "set_node_configuration",
            Self::BindModelProfile => "bind_model_profile",
            Self::SetLabelMapping => "set_label_mapping",
            Self::SetDecisionPolicy => "set_decision_policy",
            Self::SetRuntimePolicy => "set_runtime_policy",
            Self::ComparePipelineDrafts => "compare_pipeline_drafts",
            Self::UndoLastDraftChange => "undo_last_draft_change",
            Self::ValidatePipeline => "validate_pipeline",
            Self::EstimatePipelineCost => "estimate_pipeline_cost",
            Self::DryRunPipeline => "dry_run_pipeline",
            Self::InspectDryRunSummary => "inspect_dry_run_summary",
            Self::InspectFailureClasses => "inspect_failure_classes",
            Self::InspectGeometryQuality => "inspect_geometry_quality",
            Self::InspectFailedSamples => "inspect_failed_samples",
            Self::InspectReviewSamples => "inspect_review_samples",
            Self::InspectNodeStatistics => "inspect_node_statistics",
            Self::InspectNodeArtifacts => "inspect_node_artifacts",
            Self::CompareDryRuns => "compare_dry_runs",
            Self::SubmitDraftForHumanApproval => "submit_draft_for_human_approval",
            Self::FinishAgentSession => "finish_agent_session",
        }
    }

    #[must_use]
    pub const fn mutates_draft(self) -> bool {
        matches!(
            self,
            Self::CreateDraftFromTemplate
                | Self::CreatePipelineDraft
                | Self::CreateBlockedDraft
                | Self::SetUnresolvedBinding
                | Self::AddPipelineNode
                | Self::RemovePipelineNode
                | Self::ConnectPipelineNodes
                | Self::DisconnectPipelineNodes
                | Self::SetNodeConfiguration
                | Self::BindModelProfile
                | Self::SetLabelMapping
                | Self::SetDecisionPolicy
                | Self::SetRuntimePolicy
                | Self::UndoLastDraftChange
                | Self::SubmitDraftForHumanApproval
        )
    }

    #[must_use]
    pub const fn cacheable_observation(self) -> bool {
        matches!(
            self,
            Self::GetPipelineBuilderContext
                | Self::ResolvePipelineFeasibility
                | Self::InspectNodesBatch
                | Self::InspectModelsBatch
                | Self::InspectContractsBatch
                | Self::InspectNodeDefinition
                | Self::ListCompatibleModels
                | Self::InspectModelProfile
                | Self::InspectModelContracts
                | Self::InspectLabelSpace
                | Self::InspectScoreSemantics
                | Self::InspectGeometrySemantics
                | Self::InspectModelQualityContract
                | Self::InspectProjectGeometryPolicy
                | Self::InspectGeometryCorrectionSummary
                | Self::InspectGeometryCalibration
                | Self::FindGeometryRefinementPath
                | Self::FindArtifactConversionPath
        )
    }

    #[must_use]
    pub const fn permission(self) -> PipelineBuilderPermission {
        match self {
            Self::GetPipelineBuilderContext
            | Self::InspectProject
            | Self::InspectLabelSchema
            | Self::InspectLabel
            | Self::SampleDataset
            | Self::InspectSampleImage
            | Self::InspectExistingPipeline
            | Self::InspectExistingAutomations
            | Self::InspectProjectGeometryPolicy
            | Self::InspectGeometryCorrectionSummary
            | Self::InspectGeometryCalibration => PipelineBuilderPermission::ReadProject,
            Self::ResolvePipelineFeasibility
            | Self::InspectNodesBatch
            | Self::InspectModelsBatch
            | Self::InspectContractsBatch
            | Self::ListEnabledSkills
            | Self::LoadSkillResource
            | Self::ListNodeDefinitions
            | Self::InspectNodeDefinition
            | Self::FindArtifactConversionPath
            | Self::ListPipelineTemplates
            | Self::ListProviderProfiles
            | Self::ListAvailableCapabilities
            | Self::ListCompatibleModels
            | Self::InspectModelProfile
            | Self::InspectWorkerHealth
            | Self::InspectModelContracts
            | Self::InspectLabelSpace
            | Self::InspectScoreSemantics
            | Self::InspectGeometrySemantics
            | Self::InspectModelQualityContract
            | Self::FindGeometryRefinementPath
            | Self::CheckCapabilityPath
            | Self::EstimateModelCost => PipelineBuilderPermission::ReadRegistry,
            Self::CheckProviderAvailability => PipelineBuilderPermission::PassiveProviderCheck,
            Self::CreatePipelineDraft
            | Self::CreateDraftFromTemplate
            | Self::CreateBlockedDraft => PipelineBuilderPermission::CreateDraft,
            Self::AddPipelineNode
            | Self::RemovePipelineNode
            | Self::ConnectPipelineNodes
            | Self::DisconnectPipelineNodes
            | Self::SetNodeConfiguration
            | Self::BindModelProfile
            | Self::SetLabelMapping
            | Self::SetDecisionPolicy
            | Self::SetRuntimePolicy
            | Self::SetUnresolvedBinding
            | Self::UndoLastDraftChange => PipelineBuilderPermission::MutateDraft,
            Self::ComparePipelineDrafts | Self::ValidatePipeline | Self::EstimatePipelineCost => {
                PipelineBuilderPermission::ReadDraft
            }
            Self::DryRunPipeline
            | Self::InspectDryRunSummary
            | Self::InspectFailureClasses
            | Self::InspectGeometryQuality
            | Self::InspectFailedSamples
            | Self::InspectReviewSamples
            | Self::InspectNodeStatistics
            | Self::InspectNodeArtifacts
            | Self::CompareDryRuns => PipelineBuilderPermission::DryRunSandbox,
            Self::SubmitDraftForHumanApproval
            | Self::FinishWithSetupRequirements
            | Self::FinishAgentSession => PipelineBuilderPermission::RequestHumanApproval,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineBuilderPermission {
    ReadProject,
    ReadRegistry,
    PassiveProviderCheck,
    CreateDraft,
    ReadDraft,
    MutateDraft,
    DryRunSandbox,
    RequestHumanApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineBuilderToolDescriptor {
    pub name: String,
    pub mutates_draft: bool,
    pub permission: PipelineBuilderPermission,
    pub description: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PipelineBuilderToolRegistry;

impl PipelineBuilderToolRegistry {
    #[must_use]
    pub fn tools(self) -> Vec<PipelineBuilderToolDescriptor> {
        PipelineBuilderTool::ALL
            .into_iter()
            .map(|tool| PipelineBuilderToolDescriptor {
                name: tool.as_str().to_owned(),
                mutates_draft: tool.mutates_draft(),
                permission: tool.permission(),
                description: tool_description(tool).to_owned(),
            })
            .collect()
    }

    pub fn resolve(self, name: &str) -> CoreResult<PipelineBuilderTool> {
        PipelineBuilderTool::ALL
            .into_iter()
            .find(|tool| tool.as_str() == name)
            .ok_or_else(|| {
                CoreError::Validation(format!(
                    "Pipeline Builder tool {name:?} is not registered; Shell, code execution, package installation, downloads, and arbitrary URLs are forbidden"
                ))
            })
    }
}

const fn tool_description(tool: PipelineBuilderTool) -> &'static str {
    match tool {
        PipelineBuilderTool::ValidatePipeline => "Run Rust static validation on the current Draft.",
        PipelineBuilderTool::DryRunPipeline => {
            "Run the current Draft in the non-committing sandbox."
        }
        PipelineBuilderTool::SubmitDraftForHumanApproval => {
            "Stop with an editable Draft that requires explicit human approval."
        }
        tool if tool.mutates_draft() => "Apply one bounded mutation to the current Draft.",
        _ => "Read bounded Project, Registry, sample, or test information.",
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineBuilderSession {
    pub protocol_version: u32,
    pub id: Uuid,
    pub project_id: String,
    pub target_draft_id: String,
    pub provider: String,
    pub backend: PipelineAdvisorBackend,
    pub status: PipelineBuilderStatus,
    pub constraints: PipelineBuilderConstraints,
    pub turns: u32,
    pub tool_calls: u32,
    pub dry_runs: u32,
    pub usage: AgentUsage,
    pub stop_reason: Option<PipelineBuilderStopReason>,
    pub audit: AgentSession,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PipelineBuilderSession {
    pub fn start(
        project_id: impl Into<String>,
        target_draft_id: impl Into<String>,
        provider: impl Into<String>,
        backend: PipelineAdvisorBackend,
        constraints: PipelineBuilderConstraints,
    ) -> CoreResult<Self> {
        constraints.validate()?;
        let project_id = project_id.into();
        let now = Utc::now();
        let audit = AgentSession::start(AgentKind::PipelineBuilder, constraints.agent_budget())
            .with_builder_constraints(constraints.clone())
            .with_builder_progress(
                PipelineBuilderBudget::from_constraints(&constraints),
                BuilderProgressInvariant::default(),
            )
            .with_project(project_id.clone());
        Ok(Self {
            protocol_version: PIPELINE_BUILDER_PROTOCOL_VERSION,
            id: audit.id,
            project_id,
            target_draft_id: target_draft_id.into(),
            provider: provider.into(),
            backend,
            status: PipelineBuilderStatus::Inspecting,
            constraints,
            turns: 0,
            tool_calls: 0,
            dry_runs: 0,
            usage: AgentUsage::default(),
            stop_reason: None,
            audit,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn begin_turn(&mut self) -> CoreResult<()> {
        if self.turns >= self.constraints.maximum_agent_turns {
            self.stop(PipelineBuilderStopReason::MaximumTurnsReached);
            return Err(CoreError::Validation(
                "Pipeline Builder maximum turns reached".to_owned(),
            ));
        }
        self.turns += 1;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn record_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
        result: AgentToolResult,
        success: bool,
    ) -> CoreResult<()> {
        let tool = PipelineBuilderToolRegistry.resolve(tool_name)?;
        if self.tool_calls >= self.constraints.maximum_tool_calls {
            self.stop(PipelineBuilderStopReason::MaximumToolCallsReached);
            return Err(CoreError::Validation(
                "Pipeline Builder maximum tool calls reached".to_owned(),
            ));
        }
        if tool == PipelineBuilderTool::DryRunPipeline {
            if self.dry_runs >= self.constraints.maximum_dry_runs {
                self.stop(PipelineBuilderStopReason::MaximumDryRunsReached);
                return Err(CoreError::Validation(
                    "Pipeline Builder maximum Dry Runs reached".to_owned(),
                ));
            }
            self.dry_runs += 1;
        }
        self.audit
            .record_tool(
                tool.as_str(),
                arguments,
                serde_json::to_value(result)
                    .map_err(|error| CoreError::Validation(error.to_string()))?,
                success,
            )
            .map_err(CoreError::Validation)?;
        self.tool_calls += 1;
        self.usage = self.audit.usage.clone();
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn transition(&mut self, status: PipelineBuilderStatus) {
        if !matches!(
            self.status,
            PipelineBuilderStatus::WaitingForHuman
                | PipelineBuilderStatus::Completed
                | PipelineBuilderStatus::Cancelled
                | PipelineBuilderStatus::BudgetExceeded
                | PipelineBuilderStatus::Failed
        ) {
            self.status = status;
            self.updated_at = Utc::now();
        }
    }

    pub fn request_human_approval(&mut self) {
        self.status = PipelineBuilderStatus::WaitingForHuman;
        self.stop_reason = Some(PipelineBuilderStopReason::DraftReadyForHumanReview);
        self.audit.wait_for_human("approve_pipeline_draft");
        self.updated_at = Utc::now();
    }

    pub fn cancel(&mut self) {
        self.status = PipelineBuilderStatus::Cancelled;
        self.stop_reason = Some(PipelineBuilderStopReason::Cancelled);
        self.audit.cancel();
        self.updated_at = Utc::now();
    }

    pub fn fail(&mut self, reason: PipelineBuilderStopReason) {
        self.status = PipelineBuilderStatus::Failed;
        self.stop_reason = Some(reason);
        self.audit.fail(format!("{reason:?}"));
        self.updated_at = Utc::now();
    }

    fn stop(&mut self, reason: PipelineBuilderStopReason) {
        self.status = PipelineBuilderStatus::BudgetExceeded;
        self.stop_reason = Some(reason);
        self.audit.status = AgentSessionStatus::BudgetExceeded;
        self.audit.stop_reason = Some(format!("{reason:?}"));
        self.updated_at = Utc::now();
    }
}

/// Bounded mutations used by Application-service Agent tools. They never publish or run a Draft.
#[derive(Debug, Default, Clone, Copy)]
pub struct PipelineDraftTools;

/// Checks the semantic contract between a public Node Definition and a Provider Model Profile.
/// A VLM that emits a `DetectionSet` is not a native detector: it requires image input plus a
/// structured response channel, while native detection still requires `ObjectDetection`.
#[must_use]
pub fn model_profile_satisfies_node_contract(
    definition: &crate::NodeDefinition,
    model: &ModelProfile,
) -> bool {
    if definition
        .required_model_capability
        .is_some_and(|required| !model.task_capabilities.contains(&required))
    {
        return false;
    }
    let consumes_images = definition
        .input_ports
        .iter()
        .any(|port| port.artifact_type == crate::ArtifactKind::Image);
    if consumes_images
        && !model
            .input_modalities
            .contains(&crate::InputModality::Image)
    {
        return false;
    }
    let structured_vlm_detection = definition.required_model_capability
        == Some(crate::ModelCapability::VisionLanguage)
        && definition
            .output_ports
            .iter()
            .any(|port| port.artifact_type == crate::ArtifactKind::DetectionSet);
    !structured_vlm_detection
        || model.protocol_features.structured_output
        || model.protocol_features.tool_calls
}

/// Session-local undo journal. Every entry is a complete, previously persisted Draft snapshot;
/// the Agent still performs mutations through typed tools and never receives a database handle.
#[derive(Debug, Clone)]
pub struct PipelineDraftHistory {
    entries: Vec<WorkflowDraft>,
    maximum_entries: usize,
}

impl Default for PipelineDraftHistory {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            maximum_entries: 32,
        }
    }
}

impl PipelineDraftHistory {
    pub fn record_before_change(&mut self, draft: &WorkflowDraft) -> CoreResult<()> {
        ensure_mutable(draft)?;
        if self.maximum_entries == 0 {
            return Err(CoreError::Validation(
                "Pipeline Draft undo history is disabled".to_owned(),
            ));
        }
        if self.entries.len() == self.maximum_entries {
            self.entries.remove(0);
        }
        self.entries.push(draft.clone());
        Ok(())
    }

    pub fn undo_last(&mut self, draft: &mut WorkflowDraft) -> CoreResult<()> {
        ensure_mutable(draft)?;
        let previous = self.entries.pop().ok_or_else(|| {
            CoreError::Validation("Pipeline Draft has no Builder change to undo".to_owned())
        })?;
        if previous.id != draft.id || previous.project_id != draft.project_id {
            return Err(CoreError::Validation(
                "Pipeline Draft undo history belongs to another Draft".to_owned(),
            ));
        }
        *draft = previous;
        touch(draft);
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl PipelineDraftTools {
    pub fn add_node(
        self,
        draft: &mut WorkflowDraft,
        node: WorkflowDraftNode,
        node_registry: &NodeRegistry,
        model_registry: &ModelRegistry,
        enabled_skills: &BTreeSet<String>,
    ) -> CoreResult<()> {
        ensure_mutable(draft)?;
        if draft.nodes.iter().any(|current| current.id == node.id) {
            return Err(CoreError::Validation(format!(
                "Draft already contains node {:?}",
                node.id
            )));
        }
        validate_node_binding(&node, node_registry, model_registry, enabled_skills)?;
        draft.nodes.push(node);
        touch(draft);
        Ok(())
    }

    pub fn remove_node(self, draft: &mut WorkflowDraft, node_id: &str) -> CoreResult<()> {
        ensure_mutable(draft)?;
        let before = draft.nodes.len();
        draft.nodes.retain(|node| node.id != node_id);
        if before == draft.nodes.len() {
            return Err(CoreError::Validation(format!(
                "Draft does not contain node {node_id:?}"
            )));
        }
        draft
            .edges
            .retain(|edge| edge.from_node != node_id && edge.to_node != node_id);
        for node in &mut draft.nodes {
            node.depends_on.retain(|dependency| dependency != node_id);
            if node.fallback.as_deref() == Some(node_id) {
                node.fallback = None;
            }
            if node.fallback_policy.target_node.as_deref() == Some(node_id) {
                node.fallback_policy.target_node = None;
            }
        }
        touch(draft);
        Ok(())
    }

    pub fn connect(self, draft: &mut WorkflowDraft, edge: WorkflowEdge) -> CoreResult<()> {
        ensure_mutable(draft)?;
        let source = draft
            .nodes
            .iter()
            .find(|node| node.id == edge.from_node)
            .ok_or_else(|| {
                CoreError::Validation("connection source is not a Draft node".to_owned())
            })?;
        let target_index = draft
            .nodes
            .iter()
            .position(|node| node.id == edge.to_node)
            .ok_or_else(|| {
                CoreError::Validation("connection target is not a Draft node".to_owned())
            })?;
        let output = source
            .outputs
            .iter()
            .find(|port| port.id == edge.from_port)
            .ok_or_else(|| {
                CoreError::Validation("connection source port is not registered".to_owned())
            })?;
        let input = draft.nodes[target_index]
            .inputs
            .iter()
            .find(|port| port.id == edge.to_port)
            .ok_or_else(|| {
                CoreError::Validation("connection target port is not registered".to_owned())
            })?;
        if output.artifact_type != input.artifact_type {
            return Err(CoreError::Validation(
                "connection Artifact types do not match".to_owned(),
            ));
        }
        if draft.edges.iter().any(|current| current == &edge) {
            return Err(CoreError::Validation(
                "connection already exists".to_owned(),
            ));
        }
        let source_id = edge.from_node.clone();
        let target_id = edge.to_node.clone();
        draft.edges.push(edge);
        if !draft.nodes[target_index].depends_on.contains(&source_id) {
            draft.nodes[target_index].depends_on.push(source_id.clone());
        }
        if contains_cycle(draft) {
            draft
                .edges
                .retain(|edge| !(edge.from_node == source_id && edge.to_node == target_id));
            draft.nodes[target_index]
                .depends_on
                .retain(|dependency| dependency != &source_id);
            return Err(CoreError::Validation(
                "connection would create a Pipeline cycle".to_owned(),
            ));
        }
        touch(draft);
        Ok(())
    }

    pub fn disconnect(
        self,
        draft: &mut WorkflowDraft,
        from_node: &str,
        to_node: &str,
    ) -> CoreResult<Vec<WorkflowEdge>> {
        ensure_mutable(draft)?;
        let mut removed = Vec::new();
        draft.edges.retain(|edge| {
            if edge.from_node == from_node && edge.to_node == to_node {
                removed.push(edge.clone());
                false
            } else {
                true
            }
        });
        if removed.is_empty() {
            return Err(CoreError::Validation(
                "requested Pipeline connection does not exist".to_owned(),
            ));
        }
        if !draft
            .edges
            .iter()
            .any(|edge| edge.from_node == from_node && edge.to_node == to_node)
            && let Some(target) = draft.nodes.iter_mut().find(|node| node.id == to_node)
        {
            target
                .depends_on
                .retain(|dependency| dependency != from_node);
        }
        touch(draft);
        Ok(removed)
    }

    pub fn set_parameter(
        self,
        draft: &mut WorkflowDraft,
        node_id: &str,
        parameter: impl Into<String>,
        value: serde_json::Value,
    ) -> CoreResult<()> {
        ensure_mutable(draft)?;
        let node = draft
            .nodes
            .iter_mut()
            .find(|node| node.id == node_id)
            .ok_or_else(|| CoreError::Validation(format!("unknown Draft node {node_id:?}")))?;
        node.parameters.insert(parameter.into(), value);
        touch(draft);
        Ok(())
    }

    pub fn set_configuration(
        self,
        draft: &mut WorkflowDraft,
        node_id: &str,
        configuration: BTreeMap<String, serde_json::Value>,
        node_registry: &NodeRegistry,
    ) -> CoreResult<()> {
        ensure_mutable(draft)?;
        let node = draft
            .nodes
            .iter_mut()
            .find(|node| node.id == node_id)
            .ok_or_else(|| CoreError::Validation(format!("unknown Draft node {node_id:?}")))?;
        let definition = node_registry.definition(&node.node_type).ok_or_else(|| {
            CoreError::Validation(format!(
                "node type {:?} is not in the public Node Catalog",
                node.node_type
            ))
        })?;
        if !configuration.is_empty() && !definition.config_schema.is_object() {
            return Err(CoreError::Validation(
                "node configuration has no object schema".to_owned(),
            ));
        }
        node.parameters = configuration;
        touch(draft);
        Ok(())
    }

    pub fn set_runtime_policy(
        self,
        draft: &mut WorkflowDraft,
        policy_id: &str,
        configuration: serde_json::Value,
        node_registry: &NodeRegistry,
    ) -> CoreResult<()> {
        ensure_mutable(draft)?;
        if node_registry.runtime_policy(policy_id).is_none() {
            return Err(CoreError::Validation(format!(
                "runtime policy {policy_id:?} is not registered"
            )));
        }
        if !configuration.is_object() {
            return Err(CoreError::Validation(
                "runtime policy configuration must be an object".to_owned(),
            ));
        }
        draft
            .runtime_policies
            .insert(policy_id.to_owned(), configuration);
        touch(draft);
        Ok(())
    }

    pub fn bind_model(
        self,
        draft: &mut WorkflowDraft,
        node_id: &str,
        model_id: &str,
        node_registry: &NodeRegistry,
        model_registry: &ModelRegistry,
        enabled_skills: &BTreeSet<String>,
    ) -> CoreResult<()> {
        ensure_mutable(draft)?;
        let node_index = draft
            .nodes
            .iter()
            .position(|node| node.id == node_id)
            .ok_or_else(|| CoreError::Validation(format!("unknown Draft node {node_id:?}")))?;
        let mut candidate = draft.nodes[node_index].clone();
        candidate.model_binding = Some(model_id.to_owned());
        validate_node_binding(&candidate, node_registry, model_registry, enabled_skills)?;
        draft.nodes[node_index] = candidate;
        touch(draft);
        Ok(())
    }

    pub fn bind_model_profile(
        self,
        draft: &mut WorkflowDraft,
        node_id: &str,
        model: &ModelProfile,
        locked: bool,
        node_registry: &NodeRegistry,
    ) -> CoreResult<()> {
        ensure_mutable(draft)?;
        let node = draft
            .nodes
            .iter_mut()
            .find(|node| node.id == node_id)
            .ok_or_else(|| CoreError::Validation(format!("unknown Draft node {node_id:?}")))?;
        let definition = node_registry.definition(&node.node_type).ok_or_else(|| {
            CoreError::Validation(format!(
                "node type {:?} is not in the public Node Catalog",
                node.node_type
            ))
        })?;
        if !model.enabled || model.status != ModelProfileStatus::Available {
            return Err(CoreError::Validation(format!(
                "model_profile_unavailable: Model Profile {:?}@{} is not available",
                model.id, model.revision
            )));
        }
        if !model_profile_satisfies_node_contract(definition, model) {
            return Err(CoreError::Validation(format!(
                "incompatible_model_capability: Model Profile {:?}@{} does not satisfy the Node capability, modality, and protocol contract",
                model.id, model.revision
            )));
        }
        node.model_profile_binding = Some(WorkflowModelBinding {
            model_profile_id: model.id,
            locked,
        });
        node.unresolved_model_requirement = None;
        touch(draft);
        Ok(())
    }
}

fn ensure_mutable(draft: &WorkflowDraft) -> CoreResult<()> {
    if matches!(
        draft.status,
        WorkflowDraftStatus::Published | WorkflowDraftStatus::Archived
    ) {
        return Err(CoreError::Validation(
            "Pipeline Builder cannot mutate a Published or Archived Workflow".to_owned(),
        ));
    }
    Ok(())
}

fn touch(draft: &mut WorkflowDraft) {
    draft.status = WorkflowDraftStatus::Editing;
    draft.updated_at = Utc::now();
}

fn validate_node_binding(
    node: &WorkflowDraftNode,
    node_registry: &NodeRegistry,
    model_registry: &ModelRegistry,
    enabled_skills: &BTreeSet<String>,
) -> CoreResult<()> {
    let descriptor = node_registry.get(&node.node_type).ok_or_else(|| {
        CoreError::Validation(format!("node type {:?} is not registered", node.node_type))
    })?;
    if forbidden_node_type(&node.node_type) {
        return Err(CoreError::Validation(
            "code, Shell, package, download, and arbitrary URL nodes are forbidden".to_owned(),
        ));
    }
    if node
        .required_skills
        .iter()
        .any(|skill| !enabled_skills.contains(skill))
    {
        return Err(CoreError::Validation(
            "node requires a Skill that is not enabled for the Project".to_owned(),
        ));
    }
    if node.model_profile_binding.is_none()
        && let Some(model_id) = &node.model_binding
    {
        let model = model_registry
            .models()
            .into_iter()
            .find(|model| model.id == *model_id)
            .ok_or_else(|| {
                CoreError::Validation(format!("model {model_id:?} is not registered"))
            })?;
        if descriptor
            .required_capabilities
            .iter()
            .any(|required| !model.capabilities.contains(required))
        {
            return Err(CoreError::Validation(format!(
                "model {model_id:?} does not satisfy the node capability contract"
            )));
        }
    }
    Ok(())
}

fn forbidden_node_type(node_type: &str) -> bool {
    let normalized = node_type.to_ascii_lowercase();
    [
        "shell",
        "python",
        "execute_code",
        "install_package",
        "download_model",
        "arbitrary_url",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn contains_cycle(draft: &WorkflowDraft) -> bool {
    let mut remaining = draft
        .nodes
        .iter()
        .map(|node| {
            let mut incoming = draft
                .edges
                .iter()
                .filter(|edge| edge.to_node == node.id)
                .map(|edge| edge.from_node.clone())
                .collect::<BTreeSet<_>>();
            incoming.extend(node.depends_on.iter().cloned());
            (node.id.clone(), incoming)
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut emitted = BTreeSet::new();
    loop {
        let ready = remaining
            .iter()
            .filter(|(_, incoming)| incoming.iter().all(|parent| emitted.contains(parent)))
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return !remaining.is_empty();
        }
        for id in ready {
            remaining.remove(&id);
            emitted.insert(id);
        }
        if remaining.is_empty() {
            return false;
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PipelineGrammarValidator;

impl PipelineGrammarValidator {
    #[must_use]
    pub fn validate(
        self,
        draft: &WorkflowDraft,
        node_registry: &NodeRegistry,
        model_registry: &ModelRegistry,
        validation_catalog: &ValidationCatalog,
        enabled_skills: &BTreeSet<String>,
        constraints: &PipelineBuilderConstraints,
    ) -> WorkflowValidationReport {
        let mut report = WorkflowStaticValidator.validate_for_publish(
            draft,
            node_registry,
            model_registry,
            validation_catalog,
            enabled_skills,
            false,
        );
        let expected_commits = draft
            .label_pipeline
            .as_ref()
            .map_or(1, |composition| composition.label_pipelines.len().max(1));
        let commits = draft
            .nodes
            .iter()
            .filter(|node| node.kind == WorkflowNodeKind::Commit)
            .collect::<Vec<_>>();
        if commits.len() != expected_commits {
            report.issues.push(builder_issue(
                "builder_commit_count",
                "nodes",
                &format!(
                    "Pipeline Builder requires exactly one Commit per Label Pipeline (expected {expected_commits}, found {})",
                    commits.len()
                ),
            ));
        }
        let decisions = draft
            .nodes
            .iter()
            .filter(|node| {
                node.kind == WorkflowNodeKind::Gate
                    || matches!(
                        node.node_type.as_str(),
                        "core.confidence_gate" | "core.evidence_gate"
                    )
            })
            .map(|node| node.id.as_str())
            .collect::<BTreeSet<_>>();
        for commit in &commits {
            let ancestors = builder_ancestors(draft, &commit.id);
            if ancestors.is_disjoint(&decisions) {
                report.issues.push(builder_issue(
                    "builder_decision_required",
                    &format!("nodes.{}", commit.id),
                    "Commit must be downstream of a Decision",
                ));
            }
        }
        for (index, decision) in draft
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| decisions.contains(node.id.as_str()))
        {
            let has_bounded_uncertainty = decision.review_gate
                || decision.gate.required
                || draft.edges.iter().any(|edge| {
                    edge.from_node == decision.id
                        && edge
                            .route
                            .as_deref()
                            .is_some_and(|route| matches!(route, "review" | "reject" | "uncertain"))
                });
            if !has_bounded_uncertainty {
                report.issues.push(builder_issue(
                    "builder_uncertainty_route_required",
                    &format!("nodes[{index}]"),
                    "Decision uncertainty must route to Review or Reject",
                ));
            }
            if !constraints.allow_human_review
                && (decision.review_gate
                    || decision.gate.required
                    || draft.edges.iter().any(|edge| {
                        edge.from_node == decision.id && edge.route.as_deref() == Some("review")
                    }))
            {
                report.issues.push(builder_issue(
                    "builder_human_review_forbidden",
                    &format!("nodes[{index}]"),
                    "Human Review is disabled by the user's hard constraints; uncertainty must route to Reject",
                ));
            }
        }
        for (index, node) in draft.nodes.iter().enumerate() {
            if forbidden_node_type(&node.node_type) {
                report.issues.push(builder_issue(
                    "builder_forbidden_node",
                    &format!("nodes[{index}].node_type"),
                    "Pipeline Builder cannot use code, Shell, downloads, package installation, or arbitrary URLs",
                ));
            }
            if fallback_depth(draft, &node.id) > 2 {
                report.issues.push(builder_issue(
                    "builder_fallback_depth",
                    &format!("nodes[{index}].fallback"),
                    "Pipeline Builder fallback depth cannot exceed two",
                ));
            }
        }
        let model_calls = draft
            .nodes
            .iter()
            .filter(|node| node.model_binding.is_some() || node.model_profile_binding.is_some())
            .count() as u32;
        if constraints
            .max_model_calls_per_image
            .is_some_and(|maximum| model_calls > maximum)
        {
            report.issues.push(builder_issue(
                "builder_model_call_budget",
                "nodes",
                "statically estimated model calls exceed the hard per-image limit",
            ));
        }
        if !constraints.allow_external_models {
            let models = model_registry.models();
            for (index, node) in draft.nodes.iter().enumerate() {
                if let Some(model_id) = &node.model_binding
                    && models.iter().any(|model| {
                        model.id == *model_id
                            && matches!(
                                model.backend.kind,
                                Some(VisionBackendKind::OpenAiCompatible)
                            )
                    })
                {
                    report.issues.push(builder_issue(
                        "builder_external_model_forbidden",
                        &format!("nodes[{index}].model_binding"),
                        "external models are disabled by Pipeline Builder constraints",
                    ));
                }
            }
        }
        report.valid = report.issues.iter().all(|issue| !issue.blocking);
        report
    }
}

fn builder_issue(code: &str, path: &str, message: &str) -> WorkflowValidationIssue {
    WorkflowValidationIssue {
        code: code.to_owned(),
        path: path.to_owned(),
        message: message.to_owned(),
        blocking: true,
    }
}

fn builder_ancestors<'a>(draft: &'a WorkflowDraft, start: &str) -> BTreeSet<&'a str> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start.to_owned()]);
    while let Some(current) = queue.pop_front() {
        let mut parents = draft
            .edges
            .iter()
            .filter(|edge| edge.to_node == current)
            .map(|edge| edge.from_node.as_str())
            .collect::<Vec<_>>();
        if parents.is_empty()
            && let Some(node) = draft.nodes.iter().find(|node| node.id == current)
        {
            parents.extend(node.depends_on.iter().map(String::as_str));
        }
        for parent in parents {
            if seen.insert(parent)
                && let Some(node) = draft.nodes.iter().find(|node| node.id == parent)
            {
                queue.push_back(node.id.clone());
            }
        }
    }
    seen
}

fn fallback_depth(draft: &WorkflowDraft, start: &str) -> usize {
    let mut depth = 0;
    let mut current = start;
    let mut seen = BTreeSet::new();
    while seen.insert(current) {
        let Some(node) = draft.nodes.iter().find(|node| node.id == current) else {
            break;
        };
        let Some(next) = node
            .fallback_policy
            .target_node
            .as_deref()
            .or(node.fallback.as_deref())
        else {
            break;
        };
        depth += 1;
        current = next;
    }
    depth
}

/// Deterministic policy used by CI and classroom demos. The host still executes every tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptedMockPhase {
    InspectProject,
    InspectLabel,
    InspectRegistry,
    CreateDraft,
    MakeInvalidDraft,
    ValidateInvalidDraft,
    RepairDraft,
    ValidateRepairedDraft,
    FirstDryRun,
    InspectFirstDryRun,
    AddCropVerification,
    ValidateRevisedDraft,
    SecondDryRun,
    SubmitForHumanApproval,
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptedMockPipelineBuilder {
    pub phase: ScriptedMockPhase,
}

impl Default for ScriptedMockPipelineBuilder {
    fn default() -> Self {
        Self {
            phase: ScriptedMockPhase::InspectProject,
        }
    }
}

impl ScriptedMockPipelineBuilder {
    #[must_use]
    pub const fn next_tool(&self) -> Option<PipelineBuilderTool> {
        use PipelineBuilderTool as Tool;
        use ScriptedMockPhase as Phase;
        match self.phase {
            Phase::InspectProject => Some(Tool::InspectProject),
            Phase::InspectLabel => Some(Tool::InspectLabel),
            Phase::InspectRegistry => Some(Tool::ListCompatibleModels),
            Phase::CreateDraft => Some(Tool::CreateDraftFromTemplate),
            Phase::MakeInvalidDraft => Some(Tool::DisconnectPipelineNodes),
            Phase::ValidateInvalidDraft
            | Phase::ValidateRepairedDraft
            | Phase::ValidateRevisedDraft => Some(Tool::ValidatePipeline),
            Phase::RepairDraft => Some(Tool::ConnectPipelineNodes),
            Phase::FirstDryRun | Phase::SecondDryRun => Some(Tool::DryRunPipeline),
            Phase::InspectFirstDryRun => Some(Tool::InspectDryRunSummary),
            Phase::AddCropVerification => Some(Tool::AddPipelineNode),
            Phase::SubmitForHumanApproval => Some(Tool::SubmitDraftForHumanApproval),
            Phase::Finished => None,
        }
    }

    pub fn observe(&mut self, result: &AgentToolResult) -> CoreResult<()> {
        use ScriptedMockPhase as Phase;
        match self.phase {
            Phase::ValidateInvalidDraft
                if result
                    .model_payload
                    .get("valid")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false) =>
            {
                return Err(CoreError::Validation(
                    "ScriptedMock expected the first Draft to fail validation".to_owned(),
                ));
            }
            Phase::ValidateRepairedDraft | Phase::ValidateRevisedDraft
                if result
                    .model_payload
                    .get("valid")
                    .and_then(serde_json::Value::as_bool)
                    != Some(true) =>
            {
                return Err(CoreError::Validation(
                    "ScriptedMock expected the revised Draft to pass validation".to_owned(),
                ));
            }
            Phase::InspectFirstDryRun
                if result
                    .model_payload
                    .get("review_rate")
                    .and_then(serde_json::Value::as_f64)
                    .is_none_or(|rate| rate <= 0.25) =>
            {
                return Err(CoreError::Validation(
                    "ScriptedMock expected the first Dry Run review rate to exceed the target"
                        .to_owned(),
                ));
            }
            _ => {}
        }
        self.phase = match self.phase {
            Phase::InspectProject => Phase::InspectLabel,
            Phase::InspectLabel => Phase::InspectRegistry,
            Phase::InspectRegistry => Phase::CreateDraft,
            Phase::CreateDraft => Phase::MakeInvalidDraft,
            Phase::MakeInvalidDraft => Phase::ValidateInvalidDraft,
            Phase::ValidateInvalidDraft => Phase::RepairDraft,
            Phase::RepairDraft => Phase::ValidateRepairedDraft,
            Phase::ValidateRepairedDraft => Phase::FirstDryRun,
            Phase::FirstDryRun => Phase::InspectFirstDryRun,
            Phase::InspectFirstDryRun => Phase::AddCropVerification,
            Phase::AddCropVerification => Phase::ValidateRevisedDraft,
            Phase::ValidateRevisedDraft => Phase::SecondDryRun,
            Phase::SecondDryRun => Phase::SubmitForHumanApproval,
            Phase::SubmitForHumanApproval | Phase::Finished => Phase::Finished,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::{
        ArtifactKind, BackendDescriptor, ModelAvailabilityStatus, ModelVersionMetadata,
        RetryPolicy, VisionCapability, VisionInferenceRequest, VisionInferenceResponse,
        VisionModelBackend, VisionModelDescriptor, VisionNodeDescriptor,
    };

    struct MockBackend;

    #[async_trait]
    impl VisionModelBackend for MockBackend {
        fn id(&self) -> &str {
            "mock"
        }

        fn kind(&self) -> VisionBackendKind {
            VisionBackendKind::Mock
        }

        fn capabilities(&self) -> Vec<VisionCapability> {
            vec![VisionCapability::ObjectDetection]
        }

        async fn infer(
            &self,
            _request: VisionInferenceRequest,
            _cancellation: CancellationToken,
        ) -> CoreResult<VisionInferenceResponse> {
            unreachable!("registry-only fixture")
        }
    }

    fn registries() -> (NodeRegistry, ModelRegistry) {
        let mut nodes = NodeRegistry::new();
        for (id, accepts, produces, required) in [
            (
                "detect",
                vec![ArtifactKind::Image],
                vec![ArtifactKind::DetectionSet],
                vec![VisionCapability::ObjectDetection],
            ),
            (
                "core.confidence_gate",
                vec![ArtifactKind::DetectionSet],
                vec![ArtifactKind::DetectionSet],
                Vec::new(),
            ),
            (
                "commit",
                vec![ArtifactKind::DetectionSet],
                vec![ArtifactKind::DetectionSet],
                Vec::new(),
            ),
        ] {
            nodes
                .register(VisionNodeDescriptor {
                    id: id.to_owned(),
                    display_name: id.to_owned(),
                    required_capabilities: required,
                    accepts,
                    produces,
                    deterministic: id != "detect",
                })
                .expect("node");
        }
        nodes
            .register_definition(crate::NodeDefinition {
                id: "detect".to_owned(),
                display_name: "Detect".to_owned(),
                category: crate::NodeCategory::ModelInference,
                input_ports: Vec::new(),
                output_ports: vec![crate::PortDefinition {
                    name: "output".to_owned(),
                    artifact_type: ArtifactKind::DetectionSet,
                    required: true,
                    cardinality: crate::PortCardinality::Many,
                }],
                config_schema: serde_json::json!({"type": "object"}),
                required_model_capability: Some(crate::ModelCapability::ObjectDetection),
                cardinality: crate::NodeCardinality::OneToMany,
                side_effect: crate::NodeSideEffect::None,
                dry_run_supported: true,
                expert_only: false,
            })
            .expect("public definition");
        let mut models = ModelRegistry::new();
        models
            .register_backend(Arc::new(MockBackend))
            .expect("backend");
        models
            .register_model(VisionModelDescriptor {
                id: "mock-detector".to_owned(),
                display_name: "Mock Detector".to_owned(),
                backend_id: "mock".to_owned(),
                provider: "mock".to_owned(),
                backend: BackendDescriptor {
                    kind: Some(VisionBackendKind::Mock),
                    ..BackendDescriptor::default()
                },
                capabilities: vec![VisionCapability::ObjectDetection],
                model: "mock".to_owned(),
                model_version: "1".to_owned(),
                version: ModelVersionMetadata::default(),
                status: ModelAvailabilityStatus::Available,
                ..VisionModelDescriptor::default()
            })
            .expect("model");
        (nodes, models)
    }

    fn node(
        id: &str,
        node_type: &str,
        kind: WorkflowNodeKind,
        artifact: ArtifactKind,
    ) -> WorkflowDraftNode {
        WorkflowDraftNode {
            id: id.to_owned(),
            node_type: node_type.to_owned(),
            kind,
            inputs: if id == "detect" {
                Vec::new()
            } else {
                vec![crate::NodePort {
                    id: "input".to_owned(),
                    artifact_type: artifact,
                    required: true,
                    multiple: false,
                }]
            },
            outputs: vec![crate::NodePort {
                id: "output".to_owned(),
                artifact_type: artifact,
                required: true,
                multiple: false,
            }],
            retry_policy: RetryPolicy::default(),
            ..WorkflowDraftNode::default()
        }
    }

    fn draft() -> WorkflowDraft {
        let now = Utc::now();
        let mut detect = node(
            "detect",
            "detect",
            WorkflowNodeKind::VisionModel,
            ArtifactKind::DetectionSet,
        );
        detect.model_binding = Some("mock-detector".to_owned());
        let mut decision = node(
            "decision",
            "core.confidence_gate",
            WorkflowNodeKind::Gate,
            ArtifactKind::DetectionSet,
        );
        decision.depends_on = vec!["detect".to_owned()];
        decision.gate.required = true;
        let mut commit = node(
            "commit",
            "commit",
            WorkflowNodeKind::Commit,
            ArtifactKind::DetectionSet,
        );
        commit.depends_on = vec!["decision".to_owned()];
        WorkflowDraft {
            schema_version: 2,
            id: "draft".to_owned(),
            project_id: "project".to_owned(),
            name: "Draft".to_owned(),
            status: WorkflowDraftStatus::Editing,
            nodes: vec![detect, decision, commit],
            edges: vec![
                WorkflowEdge {
                    from_node: "detect".to_owned(),
                    from_port: "output".to_owned(),
                    to_node: "decision".to_owned(),
                    to_port: "input".to_owned(),
                    route: None,
                },
                WorkflowEdge {
                    from_node: "decision".to_owned(),
                    from_port: "output".to_owned(),
                    to_node: "commit".to_owned(),
                    to_port: "input".to_owned(),
                    route: Some("accept".to_owned()),
                },
            ],
            enabled_skills: BTreeMap::new(),
            resource_versions: BTreeMap::new(),
            runtime_policies: BTreeMap::new(),
            allow_unvalidated_commit: false,
            geometry_risk_acceptance: None,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn tool_registry_rejects_every_unbounded_escape_hatch() {
        let registry = PipelineBuilderToolRegistry;
        let tools = registry.tools();
        assert_eq!(tools.len(), 64);
        assert_eq!(tools.len(), PipelineBuilderTool::ALL.len());
        for forbidden in [
            "publish_pipeline",
            "start_full_dataset_run",
            "set_api_key",
            "create_provider",
            "delete_provider",
            "execute_shell",
            "execute_python",
            "download_model",
            "open_arbitrary_url",
            "replace_entire_workflow_json",
        ] {
            assert!(registry.resolve(forbidden).is_err(), "{forbidden}");
        }
        assert_eq!(
            registry
                .resolve("validate_pipeline")
                .expect("registered tool"),
            PipelineBuilderTool::ValidatePipeline
        );
        assert_eq!(
            registry
                .resolve("check_provider_availability")
                .expect("passive check")
                .permission(),
            PipelineBuilderPermission::PassiveProviderCheck
        );
        for name in [
            "inspect_model_quality_contract",
            "find_geometry_refinement_path",
        ] {
            assert_eq!(
                registry
                    .resolve(name)
                    .expect("geometry Registry Tool")
                    .permission(),
                PipelineBuilderPermission::ReadRegistry
            );
        }
        for name in [
            "inspect_project_geometry_policy",
            "inspect_geometry_correction_summary",
            "inspect_geometry_calibration",
        ] {
            assert_eq!(
                registry
                    .resolve(name)
                    .expect("geometry Project Tool")
                    .permission(),
                PipelineBuilderPermission::ReadProject
            );
        }
        assert!(tools.iter().all(|tool| !tool.name.contains("api_key")));
    }

    #[test]
    fn undo_and_runtime_policy_are_bounded_draft_mutations() {
        let mut draft = draft();
        let before = draft.clone();
        let mut history = PipelineDraftHistory::default();
        history
            .record_before_change(&draft)
            .expect("record previous Draft");
        PipelineDraftTools
            .set_parameter(&mut draft, "detect", "threshold", serde_json::json!(0.8))
            .expect("bounded mutation");
        history.undo_last(&mut draft).expect("undo");
        assert_eq!(draft.nodes, before.nodes);
        assert!(history.is_empty());

        let mut registry = NodeRegistry::new();
        registry
            .register_runtime_policy(crate::RuntimePolicyDefinition {
                id: "retry".to_owned(),
                display_name: "Retry".to_owned(),
                scope: crate::RuntimePolicyScope::Workflow,
                config_schema: serde_json::json!({"type": "object"}),
            })
            .expect("Runtime Policy");
        PipelineDraftTools
            .set_runtime_policy(
                &mut draft,
                "retry",
                serde_json::json!({"maximum_attempts": 2}),
                &registry,
            )
            .expect("Runtime Policy mutation");
        assert_eq!(
            draft.runtime_policies["retry"]["maximum_attempts"],
            serde_json::json!(2)
        );
        let diff = PipelineDraftDiff::between(&before, &draft).expect("Runtime Policy Diff");
        assert!(
            diff.policy_changes
                .iter()
                .any(|change| change.change_id == "workflow:runtime_policies")
        );
    }

    #[test]
    fn draft_tools_are_registry_bounded_and_preserve_publish_immutability() {
        let (nodes, models) = registries();
        let enabled = BTreeSet::new();
        let mut draft = draft();
        PipelineDraftTools
            .bind_model(
                &mut draft,
                "detect",
                "mock-detector",
                &nodes,
                &models,
                &enabled,
            )
            .expect("registry binding");
        assert!(
            PipelineDraftTools
                .bind_model(&mut draft, "detect", "unknown", &nodes, &models, &enabled,)
                .is_err()
        );
        draft.status = WorkflowDraftStatus::Published;
        assert!(
            PipelineDraftTools
                .set_parameter(&mut draft, "detect", "threshold", serde_json::json!(0.5))
                .is_err()
        );
    }

    #[test]
    fn model_profile_binding_is_capability_checked_and_revision_aware() {
        let (nodes, _models) = registries();
        let provider_id = crate::ProviderId::new();
        let now = Utc::now();
        let model = ModelProfile {
            id: crate::ModelProfileId::new(),
            revision: 3,
            provider_id,
            display_name: "Detector".to_owned(),
            remote_model_id: "detector-v3".to_owned(),
            input_modalities: BTreeSet::from([crate::InputModality::Image]),
            protocol_features: crate::ProtocolFeatures::default(),
            task_capabilities: BTreeSet::from([crate::ModelCapability::ObjectDetection]),
            capability_source: crate::CapabilityDeclarationSource::UserDeclared,
            limits: crate::ModelLimits::default(),
            generation_defaults: crate::GenerationDefaults::default(),
            pricing: crate::ModelPricing::default(),
            quality_contracts: Vec::new(),
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        };
        let mut draft = draft();
        PipelineDraftTools
            .bind_model_profile(&mut draft, "detect", &model, true, &nodes)
            .expect("compatible Model Profile");
        assert_eq!(
            draft.nodes[0]
                .model_profile_binding
                .as_ref()
                .map(|binding| (binding.model_profile_id, binding.locked)),
            Some((model.id, true))
        );

        let mut incompatible = model.clone();
        incompatible.id = crate::ModelProfileId::new();
        incompatible.task_capabilities =
            BTreeSet::from([crate::ModelCapability::ImageClassification]);
        assert!(
            PipelineDraftTools
                .bind_model_profile(&mut draft, "detect", &incompatible, true, &nodes)
                .is_err()
        );

        let mut qwen_vlm = model.clone();
        qwen_vlm.id = crate::ModelProfileId::new();
        qwen_vlm.task_capabilities = BTreeSet::from([
            crate::ModelCapability::VisionLanguage,
            crate::ModelCapability::ImageClassification,
        ]);
        qwen_vlm.input_modalities =
            BTreeSet::from([crate::InputModality::Text, crate::InputModality::Image]);
        qwen_vlm.protocol_features.structured_output = true;
        let vlm_detection = crate::NodeDefinition {
            id: "vlm_detection.detect".to_owned(),
            display_name: "Structured VLM Detection".to_owned(),
            category: crate::NodeCategory::ModelInference,
            input_ports: vec![crate::PortDefinition {
                name: "image".to_owned(),
                artifact_type: crate::ArtifactKind::Image,
                required: true,
                cardinality: crate::PortCardinality::One,
            }],
            output_ports: vec![crate::PortDefinition {
                name: "detections".to_owned(),
                artifact_type: crate::ArtifactKind::DetectionSet,
                required: true,
                cardinality: crate::PortCardinality::Many,
            }],
            config_schema: serde_json::json!({"type":"object"}),
            required_model_capability: Some(crate::ModelCapability::VisionLanguage),
            cardinality: crate::NodeCardinality::OneToMany,
            side_effect: crate::NodeSideEffect::None,
            dry_run_supported: true,
            expert_only: false,
        };
        assert!(model_profile_satisfies_node_contract(
            &vlm_detection,
            &qwen_vlm
        ));
        assert!(!model_profile_satisfies_node_contract(
            nodes.definition("detect").expect("native detector"),
            &qwen_vlm
        ));

        let mut text_only = qwen_vlm.clone();
        text_only
            .input_modalities
            .remove(&crate::InputModality::Image);
        assert!(!model_profile_satisfies_node_contract(
            &vlm_detection,
            &text_only
        ));
        let mut unstructured = qwen_vlm;
        unstructured.protocol_features.structured_output = false;
        unstructured.protocol_features.tool_calls = false;
        assert!(!model_profile_satisfies_node_contract(
            &vlm_detection,
            &unstructured
        ));
    }

    #[test]
    fn structured_draft_diff_supports_selective_apply_and_exact_undo_snapshot() {
        let base = draft();
        let mut proposed = base.clone();
        proposed.id = "proposal".to_owned();
        proposed.nodes[0]
            .parameters
            .insert("threshold".to_owned(), serde_json::json!(0.4));
        proposed.nodes[0].model_binding = None;
        let mut crop = node(
            "crop",
            "core.crop",
            WorkflowNodeKind::Transform,
            ArtifactKind::DetectionSet,
        );
        crop.parameters
            .insert("padding".to_owned(), serde_json::json!(0.08));
        proposed.nodes.insert(1, crop);
        proposed.edges.push(WorkflowEdge {
            from_node: "detect".to_owned(),
            from_port: "output".to_owned(),
            to_node: "crop".to_owned(),
            to_port: "input".to_owned(),
            route: None,
        });

        let diff = PipelineDraftDiff::between(&base, &proposed).expect("Draft Diff");
        assert_eq!(diff.added_nodes.len(), 1);
        assert_eq!(diff.modified_nodes.len(), 1);
        assert_eq!(diff.model_binding_changes.len(), 1);
        assert_eq!(diff.added_edges.len(), 1);

        let parameter_change = diff.modified_nodes[0].change_id.clone();
        let selected = BTreeSet::from([parameter_change]);
        let partially_applied = diff
            .apply_selected(&base, &proposed, &selected)
            .expect("selective apply");
        assert_eq!(partially_applied.id, base.id);
        assert_eq!(
            partially_applied.nodes[0].parameters.get("threshold"),
            Some(&serde_json::json!(0.4))
        );
        assert_eq!(partially_applied.nodes.len(), base.nodes.len());
        assert_eq!(
            partially_applied.nodes[0].model_binding,
            base.nodes[0].model_binding
        );
        assert_eq!(base.nodes[0].parameters.get("threshold"), None);

        let fully_applied = diff
            .apply_selected(&base, &proposed, &diff.all_change_ids())
            .expect("apply all");
        assert_eq!(fully_applied.id, base.id);
        assert_eq!(fully_applied.nodes.len(), proposed.nodes.len());
        assert_eq!(fully_applied.status, WorkflowDraftStatus::Editing);
        assert!(
            diff.apply_selected(
                &base,
                &proposed,
                &BTreeSet::from(["unknown-change".to_owned()])
            )
            .is_err()
        );
    }

    #[test]
    fn grammar_requires_decision_and_bounded_uncertainty_before_commit() {
        let (nodes, models) = registries();
        let enabled = BTreeSet::new();
        let constraints = PipelineBuilderConstraints::default();
        let valid = PipelineGrammarValidator.validate(
            &draft(),
            &nodes,
            &models,
            &ValidationCatalog::default(),
            &enabled,
            &constraints,
        );
        assert!(valid.valid, "{:?}", valid.issues);

        let mut unsafe_draft = draft();
        unsafe_draft.nodes[1].gate.required = false;
        unsafe_draft.nodes[1].review_gate = false;
        let unsafe_report = PipelineGrammarValidator.validate(
            &unsafe_draft,
            &nodes,
            &models,
            &ValidationCatalog::default(),
            &enabled,
            &constraints,
        );
        assert!(
            unsafe_report
                .issues
                .iter()
                .any(|issue| issue.code == "builder_uncertainty_route_required")
        );

        let mut no_decision = draft();
        no_decision.nodes[1].kind = WorkflowNodeKind::Transform;
        no_decision.nodes[1].node_type = "detect".to_owned();
        let report = PipelineGrammarValidator.validate(
            &no_decision,
            &nodes,
            &models,
            &ValidationCatalog::default(),
            &enabled,
            &constraints,
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "builder_decision_required")
        );
    }

    #[test]
    fn scripted_mock_proves_invalid_repair_dry_run_revision_and_human_stop() {
        let mut policy = ScriptedMockPipelineBuilder::default();
        let mut tools = Vec::new();
        while let Some(tool) = policy.next_tool() {
            tools.push(tool);
            let payload = match policy.phase {
                ScriptedMockPhase::ValidateInvalidDraft => serde_json::json!({"valid": false}),
                ScriptedMockPhase::ValidateRepairedDraft
                | ScriptedMockPhase::ValidateRevisedDraft => serde_json::json!({"valid": true}),
                ScriptedMockPhase::InspectFirstDryRun => serde_json::json!({"review_rate": 0.8}),
                _ => serde_json::json!({}),
            };
            policy
                .observe(&AgentToolResult::summary("scripted result", payload))
                .expect("scripted transition");
        }
        assert_eq!(policy.phase, ScriptedMockPhase::Finished);
        assert_eq!(
            tools,
            vec![
                PipelineBuilderTool::InspectProject,
                PipelineBuilderTool::InspectLabel,
                PipelineBuilderTool::ListCompatibleModels,
                PipelineBuilderTool::CreateDraftFromTemplate,
                PipelineBuilderTool::DisconnectPipelineNodes,
                PipelineBuilderTool::ValidatePipeline,
                PipelineBuilderTool::ConnectPipelineNodes,
                PipelineBuilderTool::ValidatePipeline,
                PipelineBuilderTool::DryRunPipeline,
                PipelineBuilderTool::InspectDryRunSummary,
                PipelineBuilderTool::AddPipelineNode,
                PipelineBuilderTool::ValidatePipeline,
                PipelineBuilderTool::DryRunPipeline,
                PipelineBuilderTool::SubmitDraftForHumanApproval,
            ]
        );
    }

    #[test]
    fn builder_session_enforces_turn_tool_dry_run_and_human_boundaries() {
        let bounded = PipelineBuilderConstraints {
            maximum_agent_turns: 1,
            maximum_tool_calls: 2,
            maximum_dry_runs: 1,
            ..PipelineBuilderConstraints::default()
        };
        assert_eq!(bounded.agent_budget().max_steps, 2);
        let mut session = PipelineBuilderSession::start(
            "project",
            "draft",
            "scripted-mock",
            PipelineAdvisorBackend::ScriptedMock,
            bounded,
        )
        .expect("session");
        session.begin_turn().expect("first turn");
        assert!(session.begin_turn().is_err());

        let mut approval = PipelineBuilderSession::start(
            "project",
            "draft",
            "scripted-mock",
            PipelineAdvisorBackend::ScriptedMock,
            PipelineBuilderConstraints::default(),
        )
        .expect("session");
        approval
            .record_tool(
                "dry_run_pipeline",
                serde_json::json!({}),
                AgentToolResult::summary("sandbox", serde_json::json!({})),
                true,
            )
            .expect("Dry Run");
        approval.request_human_approval();
        assert_eq!(approval.status, PipelineBuilderStatus::WaitingForHuman);
        assert_eq!(
            approval.stop_reason,
            Some(PipelineBuilderStopReason::DraftReadyForHumanReview)
        );
        assert_eq!(approval.audit.status, AgentSessionStatus::WaitingForHuman);
    }

    #[test]
    fn phased_budget_preserves_finalization_and_rejects_phase_regression() {
        let budget =
            PipelineBuilderBudget::from_constraints(&PipelineBuilderConstraints::default());
        budget.validate().expect("valid phased budget");
        assert_eq!(budget.max_discovery_tool_calls, 10);
        assert_eq!(budget.reserved_finalization_calls, 6);
        assert_eq!(budget.remaining(42), 6);

        let mut session = AgentSession::start(
            AgentKind::PipelineBuilder,
            PipelineBuilderConstraints::default().agent_budget(),
        )
        .with_builder_progress(budget, BuilderProgressInvariant::default());
        session
            .transition_builder_phase(
                PipelineBuilderPhase::FeasibilityAnalysis,
                "Resolve feasibility",
            )
            .expect("context to feasibility");
        session
            .transition_builder_phase(PipelineBuilderPhase::Drafting, "Create a Draft")
            .expect("feasibility to drafting");
        assert!(
            session
                .transition_builder_phase(
                    PipelineBuilderPhase::ContextLoading,
                    "invalid regression"
                )
                .is_err()
        );
        session.set_builder_draft("draft-1");
        session.complete_builder(
            PipelineBuilderOutcome::ProviderSetupRequired,
            BuilderStopReason::SetupRequired,
            "Configure a compatible model",
        );
        assert_eq!(session.phase, Some(PipelineBuilderPhase::WaitingForHuman));
        assert_eq!(session.status, AgentSessionStatus::WaitingForHuman);
        assert_eq!(session.draft_id.as_deref(), Some("draft-1"));
        assert_eq!(
            session.outcome,
            Some(PipelineBuilderOutcome::ProviderSetupRequired)
        );
    }
}
