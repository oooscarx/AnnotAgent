//! Constrained, auditable primitives for the Lean Pipeline Builder Agent.

use std::collections::{BTreeSet, VecDeque};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AgentBudget, AgentKind, AgentSession, AgentSessionStatus, AgentUsage, CoreError, CoreResult,
    ModelRegistry, NodeRegistry, StoredPayloadRef, ValidationCatalog, VisionBackendKind,
    WorkflowDraft, WorkflowDraftNode, WorkflowDraftStatus, WorkflowEdge, WorkflowNodeKind,
    WorkflowStaticValidator, WorkflowValidationIssue, WorkflowValidationReport,
};

pub const PIPELINE_BUILDER_PROTOCOL_VERSION: u32 = 1;

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
            max_steps: self.maximum_tool_calls,
            max_tool_calls: self.maximum_tool_calls,
            max_tokens: None,
            max_cost: Some(self.maximum_agent_cost),
        }
    }
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
    InspectProject,
    InspectLabelSchema,
    InspectLabel,
    SampleImages,
    InspectSampleImage,
    ListEnabledSkills,
    LoadSkillResource,
    ListAvailableCapabilities,
    ListAvailableNodes,
    ListAvailableModels,
    InspectModel,
    ListPipelineTemplates,
    CreateDraftFromTemplate,
    CreateEmptyDraft,
    AddPipelineNode,
    RemovePipelineNode,
    ConnectPipelineNodes,
    DisconnectPipelineNodes,
    SetNodeParameter,
    BindModel,
    SetLabelMapping,
    SetDecisionPolicy,
    ValidatePipeline,
    EstimatePipelineCost,
    DryRunPipeline,
    InspectDryRunSummary,
    InspectFailedSamples,
    InspectReviewSamples,
    InspectNodeArtifacts,
    SubmitDraftForHumanApproval,
    FinishAdvisorSession,
}

impl PipelineBuilderTool {
    pub const ALL: [Self; 31] = [
        Self::InspectProject,
        Self::InspectLabelSchema,
        Self::InspectLabel,
        Self::SampleImages,
        Self::InspectSampleImage,
        Self::ListEnabledSkills,
        Self::LoadSkillResource,
        Self::ListAvailableCapabilities,
        Self::ListAvailableNodes,
        Self::ListAvailableModels,
        Self::InspectModel,
        Self::ListPipelineTemplates,
        Self::CreateDraftFromTemplate,
        Self::CreateEmptyDraft,
        Self::AddPipelineNode,
        Self::RemovePipelineNode,
        Self::ConnectPipelineNodes,
        Self::DisconnectPipelineNodes,
        Self::SetNodeParameter,
        Self::BindModel,
        Self::SetLabelMapping,
        Self::SetDecisionPolicy,
        Self::ValidatePipeline,
        Self::EstimatePipelineCost,
        Self::DryRunPipeline,
        Self::InspectDryRunSummary,
        Self::InspectFailedSamples,
        Self::InspectReviewSamples,
        Self::InspectNodeArtifacts,
        Self::SubmitDraftForHumanApproval,
        Self::FinishAdvisorSession,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InspectProject => "inspect_project",
            Self::InspectLabelSchema => "inspect_label_schema",
            Self::InspectLabel => "inspect_label",
            Self::SampleImages => "sample_images",
            Self::InspectSampleImage => "inspect_sample_image",
            Self::ListEnabledSkills => "list_enabled_skills",
            Self::LoadSkillResource => "load_skill_resource",
            Self::ListAvailableCapabilities => "list_available_capabilities",
            Self::ListAvailableNodes => "list_available_nodes",
            Self::ListAvailableModels => "list_available_models",
            Self::InspectModel => "inspect_model",
            Self::ListPipelineTemplates => "list_pipeline_templates",
            Self::CreateDraftFromTemplate => "create_draft_from_template",
            Self::CreateEmptyDraft => "create_empty_draft",
            Self::AddPipelineNode => "add_pipeline_node",
            Self::RemovePipelineNode => "remove_pipeline_node",
            Self::ConnectPipelineNodes => "connect_pipeline_nodes",
            Self::DisconnectPipelineNodes => "disconnect_pipeline_nodes",
            Self::SetNodeParameter => "set_node_parameter",
            Self::BindModel => "bind_model",
            Self::SetLabelMapping => "set_label_mapping",
            Self::SetDecisionPolicy => "set_decision_policy",
            Self::ValidatePipeline => "validate_pipeline",
            Self::EstimatePipelineCost => "estimate_pipeline_cost",
            Self::DryRunPipeline => "dry_run_pipeline",
            Self::InspectDryRunSummary => "inspect_dry_run_summary",
            Self::InspectFailedSamples => "inspect_failed_samples",
            Self::InspectReviewSamples => "inspect_review_samples",
            Self::InspectNodeArtifacts => "inspect_node_artifacts",
            Self::SubmitDraftForHumanApproval => "submit_draft_for_human_approval",
            Self::FinishAdvisorSession => "finish_advisor_session",
        }
    }

    #[must_use]
    pub const fn mutates_draft(self) -> bool {
        matches!(
            self,
            Self::CreateDraftFromTemplate
                | Self::CreateEmptyDraft
                | Self::AddPipelineNode
                | Self::RemovePipelineNode
                | Self::ConnectPipelineNodes
                | Self::DisconnectPipelineNodes
                | Self::SetNodeParameter
                | Self::BindModel
                | Self::SetLabelMapping
                | Self::SetDecisionPolicy
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineBuilderToolDescriptor {
    pub name: String,
    pub mutates_draft: bool,
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
    if let Some(model_id) = &node.model_binding {
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
            .filter(|node| node.model_binding.is_some())
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
            Phase::InspectRegistry => Some(Tool::ListAvailableModels),
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
            allow_unvalidated_commit: false,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn tool_registry_rejects_every_unbounded_escape_hatch() {
        let registry = PipelineBuilderToolRegistry;
        assert_eq!(registry.tools().len(), PipelineBuilderTool::ALL.len());
        for forbidden in [
            "run_shell",
            "write_python",
            "install_package",
            "download_model",
            "open_arbitrary_url",
            "execute_code",
        ] {
            assert!(registry.resolve(forbidden).is_err(), "{forbidden}");
        }
        assert_eq!(
            registry
                .resolve("validate_pipeline")
                .expect("registered tool"),
            PipelineBuilderTool::ValidatePipeline
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
                PipelineBuilderTool::ListAvailableModels,
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
        let mut session = PipelineBuilderSession::start(
            "project",
            "draft",
            "scripted-mock",
            PipelineAdvisorBackend::ScriptedMock,
            PipelineBuilderConstraints {
                maximum_agent_turns: 1,
                maximum_tool_calls: 2,
                maximum_dry_runs: 1,
                ..PipelineBuilderConstraints::default()
            },
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
}
