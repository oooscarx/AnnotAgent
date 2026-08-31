//! Versioned, registry-bound workflow schemas and static validation.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    AgentBudget, ArtifactKind, DetectionFallbackQuery, DetectionRecoveryPolicy, EvidenceAcceptRule,
    EvidenceFallbackRule, LabelId, ModelAvailabilityStatus, ModelRegistry, NodeRegistry,
    ProjectSchema, TaskId, TaskKind, ValidationCatalog, VisionCapability, VisionModelDescriptor,
};

pub const WORKFLOW_SCHEMA_VERSION: u32 = 2;
pub const MAX_WORKFLOW_RETRIES: u32 = 10;
pub type ArtifactType = ArtifactKind;
pub type WorkflowNode = WorkflowDraftNode;
pub type WorkflowVersion = PublishedWorkflowVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDraftStatus {
    Suggested,
    Editing,
    Validated,
    Published,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowNodeKind {
    ImageInput,
    #[default]
    Transform,
    VisionModel,
    VisionLanguageModel,
    DeterministicTool,
    CandidateMerge,
    Validator,
    Refiner,
    Gate,
    HumanReview,
    Commit,
    Export,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodePort {
    pub id: String,
    pub artifact_type: ArtifactType,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub multiple: bool,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub from_node: String,
    pub from_port: String,
    pub to_node: String,
    pub to_port: String,
    /// Optional route selected by a Gate node. `None` means an unconditional data edge.
    #[serde(default)]
    pub route: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct NodeConfig(pub BTreeMap<String, serde_json::Value>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    #[serde(default)]
    pub max_attempts: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 1 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FallbackPolicy {
    pub target_node: Option<String>,
    #[serde(default)]
    pub on_timeout: bool,
    #[serde(default = "default_true")]
    pub on_error: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReviewGate {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub allow_manual_override: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ResourceRequirements {
    pub timeout_seconds: Option<u64>,
    pub max_memory_mb: Option<u64>,
    pub accelerator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkflowDraftNode {
    pub id: String,
    /// Registry operation id; concrete model and tool names are never Core enum variants.
    pub node_type: String,
    #[serde(default)]
    pub kind: WorkflowNodeKind,
    /// Compatibility projection for v1 clients. For v2, `edges` are authoritative.
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub inputs: Vec<NodePort>,
    #[serde(default)]
    pub outputs: Vec<NodePort>,
    pub model_binding: Option<String>,
    /// Durable user-facing binding. `model_binding` remains the runtime-registry compatibility
    /// projection until legacy Projects are migrated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_profile_binding: Option<crate::WorkflowModelBinding>,
    #[serde(default)]
    pub required_skills: Vec<String>,
    #[serde(default)]
    pub validators: Vec<String>,
    #[serde(default)]
    pub refiners: Vec<String>,
    /// Compatibility target used by the existing editor.
    pub fallback: Option<String>,
    #[serde(default)]
    pub max_retries: u32,
    #[serde(default)]
    pub review_gate: bool,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub retry_policy: RetryPolicy,
    #[serde(default)]
    pub fallback_policy: FallbackPolicy,
    #[serde(default)]
    pub gate: ReviewGate,
    #[serde(default)]
    pub resources: ResourceRequirements,
}

impl WorkflowDraftNode {
    fn effective_retry_limit(&self) -> u32 {
        self.retry_policy.max_attempts.max(self.max_retries)
    }

    fn effective_fallback(&self) -> Option<&str> {
        self.fallback_policy
            .target_node
            .as_deref()
            .or(self.fallback.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDraft {
    #[serde(default = "default_workflow_schema_version")]
    pub schema_version: u32,
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: WorkflowDraftStatus,
    pub nodes: Vec<WorkflowDraftNode>,
    #[serde(default)]
    pub edges: Vec<WorkflowEdge>,
    #[serde(default)]
    pub enabled_skills: BTreeMap<String, String>,
    #[serde(default)]
    pub resource_versions: BTreeMap<String, String>,
    /// Cross-cutting execution behavior. Runtime Policies are intentionally stored outside the
    /// graph so the Builder cannot disguise cache, retry, budget, or Run control as Nodes.
    #[serde(default)]
    pub runtime_policies: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub allow_unvalidated_commit: bool,
    /// Optional authoring projection for label-oriented workflows. Runtime execution remains the
    /// compiled, flat `nodes`/`edges` DAG so shared stages execute once per image.
    #[serde(default)]
    pub label_pipeline: Option<crate::LabelWorkflowComposition>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A domain extension's project-independent starting graph. Applications instantiate the
/// template as a new mutable draft and attach the selected project's immutable Skill versions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub nodes: Vec<WorkflowDraftNode>,
    #[serde(default)]
    pub edges: Vec<WorkflowEdge>,
    #[serde(default)]
    pub resource_versions: BTreeMap<String, String>,
    #[serde(default)]
    pub allow_unvalidated_commit: bool,
}

impl WorkflowTemplate {
    #[must_use]
    pub fn instantiate(
        &self,
        project_id: impl Into<String>,
        enabled_skills: BTreeMap<String, String>,
        now: DateTime<Utc>,
    ) -> WorkflowDraft {
        WorkflowDraft {
            schema_version: WORKFLOW_SCHEMA_VERSION,
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.into(),
            name: self.name.clone(),
            status: WorkflowDraftStatus::Editing,
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
            enabled_skills,
            resource_versions: self.resource_versions.clone(),
            runtime_policies: BTreeMap::new(),
            allow_unvalidated_commit: self.allow_unvalidated_commit,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        }
    }
}

const fn default_workflow_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowValidationIssue {
    pub code: String,
    pub path: String,
    pub message: String,
    pub blocking: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowValidationReport {
    pub valid: bool,
    pub issues: Vec<WorkflowValidationIssue>,
    pub execution_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowSuggestion {
    pub draft: WorkflowDraft,
    pub rationale: Vec<String>,
    pub estimated_model_calls_per_image: usize,
    pub estimated_latency_ms: Option<u64>,
    pub estimated_cost_tier: String,
    pub unresolved_model_bindings: Vec<String>,
    pub warnings: Vec<String>,
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowAdvisorAgentReport {
    pub session: crate::AgentSession,
    pub suggestion: Option<WorkflowSuggestion>,
    pub validation: Option<WorkflowValidationReport>,
    pub dry_run: Option<WorkflowDryRunReport>,
    pub approval_required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkflowConstraints {
    pub preferred_model_id: Option<String>,
    #[serde(default)]
    pub require_review_gate: bool,
    pub max_nodes: Option<usize>,
    pub max_cost_per_image: Option<String>,
    pub max_latency_ms: Option<u64>,
    pub minimum_accuracy: Option<f64>,
}

/// Bounded dataset facts exposed to an Advisor. Paths, image bytes, and arbitrary URLs are
/// deliberately excluded from the contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkflowDataProfile {
    pub image_count: usize,
    pub sample_width: Option<u32>,
    pub sample_height: Option<u32>,
    #[serde(default)]
    pub mime_types: BTreeSet<String>,
}

/// Complete registry-bound context supplied to either a deterministic or LLM Advisor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowAdvisorInput {
    pub project_id: String,
    pub project_schema: ProjectSchema,
    /// When present, the Advisor is constrained to proposing one editable Label Pipeline for this
    /// exact Project task/Label pair instead of an unrestricted project-wide graph.
    #[serde(default)]
    pub target_task_id: Option<TaskId>,
    #[serde(default)]
    pub target_label: Option<LabelId>,
    pub enabled_skills: Vec<String>,
    pub node_catalog: Vec<crate::NodeDefinition>,
    /// Cross-cutting behavior is configured separately and cannot be inserted into the graph.
    #[serde(default)]
    pub runtime_policies: Vec<crate::RuntimePolicyDefinition>,
    /// Credential-safe Provider summaries available to the constrained Builder.
    #[serde(default)]
    pub provider_profiles: Vec<crate::PipelineBuilderProviderProfile>,
    /// Model Profiles contain semantic configuration and pricing, never Provider credentials.
    #[serde(default)]
    pub model_profiles: Vec<crate::ModelProfile>,
    pub model_registry: Vec<VisionModelDescriptor>,
    pub validator_ids: Vec<String>,
    pub refiner_ids: Vec<String>,
    pub resource_ids: Vec<String>,
    #[serde(default)]
    pub workflow_templates: Vec<WorkflowTemplate>,
    pub constraints: WorkflowConstraints,
    pub data_profile: WorkflowDataProfile,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDryRunNodeResult {
    pub node_id: String,
    pub status: String,
    pub output_types: Vec<ArtifactType>,
    pub latency_ms: u64,
    pub estimated_cost: String,
    pub issues: Vec<WorkflowValidationIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDryRunSampleResult {
    #[serde(default)]
    pub image_index: usize,
    pub image_name: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub result_count: usize,
    #[serde(default)]
    pub auto_accepted_count: usize,
    #[serde(default)]
    pub review_count: usize,
    #[serde(default)]
    pub failed: bool,
    #[serde(default)]
    pub empty: bool,
    #[serde(default)]
    pub outcomes: Vec<SampleTestOutcome>,
    pub nodes: Vec<WorkflowDryRunNodeResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleTestOutcomeStatus {
    ReadyToAccept,
    NeedsReview,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleTestOutcome {
    pub id: String,
    pub label: String,
    pub confidence: Option<f32>,
    pub status: SampleTestOutcomeStatus,
    pub value: Option<crate::VisionArtifactValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDryRunReport {
    pub sandbox: bool,
    pub validation: WorkflowValidationReport,
    pub samples: Vec<WorkflowDryRunSampleResult>,
    #[serde(default)]
    pub summary: SampleTestSummary,
    pub total_latency_ms: u64,
    pub estimated_cost: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SampleTestSummary {
    pub image_count: usize,
    pub detection_count: usize,
    pub candidate_count: usize,
    pub auto_accepted_count: usize,
    pub needs_review_count: usize,
    pub failed_count: usize,
    #[serde(default)]
    pub empty_count: usize,
    #[serde(default)]
    pub fallback_count: usize,
    #[serde(default)]
    pub cache_hit_count: usize,
    #[serde(default)]
    pub duration_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub usage: UsageSummary,
    #[serde(default)]
    pub estimated_full_run: Option<FullRunEstimate>,
}

/// Compatibility name retained for persisted sample-test records and downstream clients.
pub type WorkflowDryRunSummary = SampleTestSummary;

/// Bounded quality/cost observation returned to Pipeline Builder policies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AgentDryRunSummary {
    pub image_count: u32,
    pub successful_images: u32,
    pub empty_images: u32,
    pub failed_images: u32,
    pub detection_count: u32,
    pub auto_accepted_count: u32,
    pub review_count: u32,
    pub rejected_count: u32,
    #[serde(default)]
    pub warning_counts: BTreeMap<String, u32>,
    pub model_calls: u32,
    pub duration_ms: u64,
    pub cost: Decimal,
}

impl AgentDryRunSummary {
    #[must_use]
    pub fn review_rate(&self) -> f32 {
        let decided = self
            .auto_accepted_count
            .saturating_add(self.review_count)
            .saturating_add(self.rejected_count);
        if decided == 0 {
            0.0
        } else {
            self.review_count as f32 / decided as f32
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UsageSummary {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub estimated_cost: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullRunEstimate {
    pub image_count: usize,
    pub duration_ms: u64,
    pub estimated_cost: String,
    pub review_count_min: usize,
    pub review_count_max: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowVersionComparison {
    pub left_workflow_id: String,
    pub left_version: u32,
    pub right_workflow_id: String,
    pub right_version: u32,
    pub added_nodes: Vec<String>,
    pub removed_nodes: Vec<String>,
    pub changed_nodes: Vec<String>,
    pub same_content: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkflowSnapshot {
    pub schema_version: u32,
    pub draft: Option<WorkflowDraft>,
    #[serde(default)]
    pub enabled_skills: BTreeMap<String, String>,
    #[serde(default)]
    pub models: Vec<VisionModelDescriptor>,
    #[serde(default)]
    pub model_profiles: Vec<crate::ModelProfileSnapshot>,
    #[serde(default)]
    pub prompt_resources: BTreeMap<String, String>,
}

impl WorkflowSnapshot {
    #[must_use]
    pub fn frozen(
        draft: &WorkflowDraft,
        models: &ModelRegistry,
        enabled_skills: BTreeMap<String, String>,
    ) -> Self {
        let referenced = draft
            .nodes
            .iter()
            .filter_map(|node| node.model_binding.as_deref())
            .collect::<BTreeSet<_>>();
        let mut model_snapshots = models
            .models()
            .into_iter()
            .filter(|model| referenced.contains(model.id.as_str()))
            .collect::<Vec<_>>();
        model_snapshots.sort_by(|left, right| left.id.cmp(&right.id));
        Self {
            schema_version: WORKFLOW_SCHEMA_VERSION,
            draft: Some(draft.clone()),
            enabled_skills,
            models: model_snapshots,
            model_profiles: Vec::new(),
            prompt_resources: draft.resource_versions.clone(),
        }
    }

    #[must_use]
    pub fn with_model_profiles(mut self, mut profiles: Vec<crate::ModelProfileSnapshot>) -> Self {
        profiles.sort_by(|left, right| {
            left.model_profile_id
                .cmp(&right.model_profile_id)
                .then_with(|| left.revision.cmp(&right.revision))
        });
        profiles.dedup_by_key(|profile| (profile.model_profile_id, profile.revision));
        self.model_profiles = profiles;
        self
    }

    pub fn stable_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Canonical semantic material for content-addressing. Lifecycle state and timestamps are
    /// deliberately excluded so validating or publishing an unchanged graph keeps the same hash.
    pub fn content_hash_material(&self) -> Result<Vec<u8>, serde_json::Error> {
        #[derive(Serialize)]
        struct Material<'a> {
            schema_version: u32,
            name: &'a str,
            nodes: &'a [WorkflowDraftNode],
            edges: &'a [WorkflowEdge],
            enabled_skills: &'a BTreeMap<String, String>,
            resource_versions: &'a BTreeMap<String, String>,
            runtime_policies: &'a BTreeMap<String, serde_json::Value>,
            allow_unvalidated_commit: bool,
            label_pipeline: &'a Option<crate::LabelWorkflowComposition>,
            models: &'a [VisionModelDescriptor],
            model_profiles: &'a [crate::ModelProfileSnapshot],
            prompt_resources: &'a BTreeMap<String, String>,
        }
        let Some(draft) = self.draft.as_ref() else {
            return serde_json::to_vec(self);
        };
        serde_json::to_vec(&Material {
            schema_version: self.schema_version,
            name: &draft.name,
            nodes: &draft.nodes,
            edges: &draft.edges,
            enabled_skills: &self.enabled_skills,
            resource_versions: &draft.resource_versions,
            runtime_policies: &draft.runtime_policies,
            allow_unvalidated_commit: draft.allow_unvalidated_commit,
            label_pipeline: &draft.label_pipeline,
            models: &self.models,
            model_profiles: &self.model_profiles,
            prompt_resources: &self.prompt_resources,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublishedWorkflowVersion {
    pub workflow_id: String,
    pub version: u32,
    pub project_id: String,
    pub source_draft_id: String,
    pub content_hash: String,
    pub draft: WorkflowDraft,
    #[serde(default)]
    pub snapshot: WorkflowSnapshot,
    pub published_at: DateTime<Utc>,
}

pub trait WorkflowAdvisor: Send + Sync {
    fn suggest_workflow(
        &self,
        project_id: &str,
        project_schema: &ProjectSchema,
        enabled_skills: &[String],
        node_catalog: &NodeRegistry,
        model_registry: &ModelRegistry,
        constraints: &WorkflowConstraints,
    ) -> WorkflowSuggestion;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RegistryWorkflowAdvisor;

impl WorkflowAdvisor for RegistryWorkflowAdvisor {
    fn suggest_workflow(
        &self,
        project_id: &str,
        project_schema: &ProjectSchema,
        enabled_skills: &[String],
        node_catalog: &NodeRegistry,
        model_registry: &ModelRegistry,
        constraints: &WorkflowConstraints,
    ) -> WorkflowSuggestion {
        if let Some(suggestion) = suggest_detection_workflow(
            project_id,
            project_schema,
            enabled_skills,
            node_catalog,
            model_registry,
            constraints,
        ) {
            return suggestion;
        }
        let preferred = constraints.preferred_model_id.clone().or_else(|| {
            model_registry
                .models()
                .into_iter()
                .find(|model| {
                    model
                        .capabilities
                        .contains(&VisionCapability::VisionLanguage)
                })
                .map(|model| model.id)
        });
        let operation = if node_catalog.get("vision_language").is_some() {
            "vision_language".to_owned()
        } else {
            node_catalog
                .nodes()
                .first()
                .map_or_else(|| "unresolved".to_owned(), |node| node.id.clone())
        };
        let skill_versions = project_schema.project.enabled_skill_versions();
        let artifact_by_task = project_schema
            .tasks
            .iter()
            .map(|task| (task.id.to_string(), artifact_for_task(task.kind)))
            .collect::<BTreeMap<_, _>>();
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for task in &project_schema.tasks {
            let task_id = task.id.to_string();
            let output_type = artifact_for_task(task.kind);
            let inputs = task
                .depends_on
                .iter()
                .map(|dependency| NodePort {
                    id: format!("from_{dependency}"),
                    artifact_type: artifact_by_task
                        .get(dependency.as_str())
                        .copied()
                        .unwrap_or(output_type),
                    required: true,
                    multiple: false,
                })
                .collect::<Vec<_>>();
            for dependency in &task.depends_on {
                edges.push(WorkflowEdge {
                    from_node: dependency.to_string(),
                    from_port: "candidates".to_owned(),
                    to_node: task_id.clone(),
                    to_port: format!("from_{dependency}"),
                    route: None,
                });
            }
            nodes.push(WorkflowDraftNode {
                id: task_id,
                node_type: operation.clone(),
                kind: WorkflowNodeKind::VisionLanguageModel,
                depends_on: task.depends_on.iter().map(ToString::to_string).collect(),
                inputs,
                outputs: vec![NodePort {
                    id: "candidates".to_owned(),
                    artifact_type: output_type,
                    required: true,
                    multiple: true,
                }],
                model_binding: preferred.clone(),
                model_profile_binding: None,
                required_skills: enabled_skills.to_vec(),
                validators: task.validators.clone(),
                refiners: task.refiners.clone(),
                fallback: None,
                max_retries: project_schema.runtime.max_retries,
                review_gate: false,
                parameters: BTreeMap::from([
                    ("task_kind".to_owned(), serde_json::json!(task.kind)),
                    ("required".to_owned(), serde_json::json!(task.required)),
                ]),
                retry_policy: RetryPolicy {
                    max_attempts: project_schema.runtime.max_retries.saturating_add(1),
                },
                fallback_policy: FallbackPolicy::default(),
                gate: ReviewGate::default(),
                resources: ResourceRequirements {
                    timeout_seconds: Some(project_schema.runtime.task_timeout_seconds),
                    ..ResourceRequirements::default()
                },
            });
        }

        let task_ids = nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>();
        let validation_inputs = task_ids
            .iter()
            .filter_map(|id| artifact_by_task.get(id).map(|kind| (id, kind)))
            .map(|(id, kind)| NodePort {
                id: format!("from_{id}"),
                artifact_type: *kind,
                required: true,
                multiple: true,
            })
            .collect::<Vec<_>>();
        for id in &task_ids {
            edges.push(WorkflowEdge {
                from_node: id.clone(),
                from_port: "candidates".to_owned(),
                to_node: "validate_candidates".to_owned(),
                to_port: format!("from_{id}"),
                route: None,
            });
        }
        nodes.push(system_node(
            "validate_candidates",
            "static_validator",
            WorkflowNodeKind::Validator,
            task_ids.clone(),
            validation_inputs.clone(),
            validation_inputs.clone(),
        ));

        let mut commit_dependency = "validate_candidates".to_owned();
        if constraints.require_review_gate && node_catalog.get("review_gate").is_some() {
            nodes.push(system_node(
                "review_gate",
                "review_gate",
                WorkflowNodeKind::HumanReview,
                vec![commit_dependency.clone()],
                validation_inputs.clone(),
                validation_inputs.clone(),
            ));
            edges.extend(validation_inputs.iter().map(|port| WorkflowEdge {
                from_node: "validate_candidates".to_owned(),
                from_port: port.id.clone(),
                to_node: "review_gate".to_owned(),
                to_port: port.id.clone(),
                route: None,
            }));
            "review_gate".clone_into(&mut commit_dependency);
        }
        nodes.push(system_node(
            "commit",
            "commit",
            WorkflowNodeKind::Commit,
            vec![commit_dependency.clone()],
            validation_inputs.clone(),
            Vec::new(),
        ));
        edges.extend(validation_inputs.into_iter().map(|port| WorkflowEdge {
            from_node: commit_dependency.clone(),
            from_port: port.id.clone(),
            to_node: "commit".to_owned(),
            to_port: port.id,
            route: None,
        }));

        let mut warnings = Vec::new();
        if let Some(max_nodes) = constraints.max_nodes
            && nodes.len() > max_nodes
        {
            warnings.push(format!(
                "suggestion has {} nodes, above configured maximum {max_nodes}",
                nodes.len()
            ));
        }
        let unresolved_model_bindings = if preferred.is_none() {
            nodes
                .iter()
                .filter(|node| node.kind == WorkflowNodeKind::VisionLanguageModel)
                .map(|node| node.id.clone())
                .collect()
        } else {
            Vec::new()
        };
        let now = Utc::now();
        let estimated_model_calls_per_image = nodes
            .iter()
            .filter(|node| node.model_binding.is_some())
            .count();
        WorkflowSuggestion {
            draft: WorkflowDraft {
                schema_version: WORKFLOW_SCHEMA_VERSION,
                id: uuid::Uuid::new_v4().to_string(),
                project_id: project_id.to_owned(),
                name: format!("{} suggested workflow", project_schema.project.name),
                status: WorkflowDraftStatus::Suggested,
                nodes,
                edges,
                enabled_skills: skill_versions,
                resource_versions: BTreeMap::new(),
                runtime_policies: BTreeMap::new(),
                allow_unvalidated_commit: false,
                label_pipeline: None,
                created_at: now,
                updated_at: now,
            },
            rationale: vec![
                "Mapped configured annotation tasks to registry operations with typed ports."
                    .to_owned(),
                format!(
                    "Bound the graph to enabled Skills: {}.",
                    enabled_skills.join(", ")
                ),
            ],
            estimated_model_calls_per_image,
            estimated_latency_ms: Some(estimated_model_calls_per_image as u64 * 1_200),
            estimated_cost_tier: if estimated_model_calls_per_image <= 1 {
                "low"
            } else if estimated_model_calls_per_image <= 3 {
                "medium"
            } else {
                "high"
            }
            .to_owned(),
            unresolved_model_bindings,
            warnings,
            alternatives: vec![
                "Bind detection or segmentation tasks to registered specialist backends."
                    .to_owned(),
                "Add a human review gate after validation for conservative publishing.".to_owned(),
            ],
        }
    }
}

fn suggest_detection_workflow(
    project_id: &str,
    project_schema: &ProjectSchema,
    enabled_skills: &[String],
    node_catalog: &NodeRegistry,
    model_registry: &ModelRegistry,
    constraints: &WorkflowConstraints,
) -> Option<WorkflowSuggestion> {
    let detection_tasks = project_schema
        .tasks
        .iter()
        .filter(|task| task.kind == TaskKind::BoundingBox)
        .collect::<Vec<_>>();
    if detection_tasks.len() != project_schema.tasks.len() {
        return None;
    }
    if detection_tasks
        .iter()
        .any(|task| !task.validators.is_empty() || !task.refiners.is_empty())
    {
        return None;
    }
    let first_task = detection_tasks.first()?;
    let target_labels = detection_tasks
        .iter()
        .flat_map(|task| task.labels.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if target_labels.is_empty() {
        return None;
    }
    let image_operation = find_node(node_catalog, None, None, ArtifactKind::Image, Some(false))?;
    let open_operation = find_node(
        node_catalog,
        Some(VisionCapability::OpenVocabularyDetection),
        Some(ArtifactKind::Image),
        ArtifactKind::DetectionSet,
        None,
    )?;
    let specialist_operation = find_node(
        node_catalog,
        Some(VisionCapability::ObjectDetection),
        Some(ArtifactKind::Image),
        ArtifactKind::DetectionSet,
        None,
    );
    let recovery_operation = find_node(
        node_catalog,
        Some(VisionCapability::OpenVocabularyDetection),
        Some(ArtifactKind::DetectionSet),
        ArtifactKind::CandidateClusterSet,
        None,
    );
    let models = model_registry.models();
    let specialist_model = select_detection_model(
        &models,
        VisionCapability::ObjectDetection,
        &target_labels,
        constraints.preferred_model_id.as_deref(),
    );
    let open_model = select_detection_model(
        &models,
        VisionCapability::OpenVocabularyDetection,
        &target_labels,
        constraints.preferred_model_id.as_deref(),
    );
    let queries = target_labels
        .iter()
        .enumerate()
        .map(|(index, label)| DetectionFallbackQuery {
            id: format!("label-{index}"),
            text: label.replace(['_', '-'], " "),
            target_label: LabelId::from(label.as_str()),
        })
        .collect::<Vec<_>>();
    let now = Utc::now();
    let mut nodes = vec![WorkflowDraftNode {
        id: "image".to_owned(),
        node_type: image_operation.id.clone(),
        kind: WorkflowNodeKind::ImageInput,
        outputs: vec![port("image", ArtifactKind::Image, false)],
        ..WorkflowDraftNode::default()
    }];
    let mut edges = Vec::new();
    let mut rationale = vec![
        "Selected a controlled detection Pipeline from registered capabilities and Project Labels."
            .to_owned(),
    ];
    let mut warnings = vec![
        "This recommendation is an editable Draft; it is never published automatically.".to_owned(),
    ];
    let mut unresolved_model_bindings = Vec::new();
    let estimated_model_calls_per_image;
    let final_artifact;
    let final_node;

    if let (Some(specialist_operation), Some(recovery_operation), Some(specialist)) = (
        specialist_operation,
        recovery_operation,
        specialist_model.as_ref(),
    ) {
        let mut detector = detection_model_node(
            "specialist",
            &specialist_operation.id,
            Some(specialist.id.clone()),
            ArtifactKind::DetectionSet,
            enabled_skills,
        );
        detector
            .parameters
            .insert("target_labels".to_owned(), serde_json::json!(target_labels));
        let mut policy = DetectionRecoveryPolicy::default();
        policy.initial_gate.accept_when = vec![EvidenceAcceptRule {
            source: Some(specialist.id.clone()),
            minimum_score: Some(project_schema.review.auto_accept_confidence.clamp(0.0, 1.0)),
            no_domain_issue: true,
            ..EvidenceAcceptRule::default()
        }];
        policy.initial_gate.fallback_when = vec![
            EvidenceFallbackRule {
                source: Some(specialist.id.clone()),
                empty_specialist_result: true,
                ..EvidenceFallbackRule::default()
            },
            EvidenceFallbackRule {
                source: Some(specialist.id.clone()),
                specialist_score_below: Some(
                    project_schema.review.force_review_below.clamp(0.0, 1.0),
                ),
                ..EvidenceFallbackRule::default()
            },
            EvidenceFallbackRule {
                source: Some(specialist.id.clone()),
                domain_issue: true,
                ..EvidenceFallbackRule::default()
            },
            EvidenceFallbackRule {
                source: Some(specialist.id.clone()),
                correction_risk_above: Some(0.7),
                ..EvidenceFallbackRule::default()
            },
        ];
        policy.fallback_estimated_cost = open_model
            .as_ref()
            .and_then(|model| model.pricing.per_request)
            .unwrap_or_default();
        let agent_budget = AgentBudget {
            max_steps: 4,
            max_tool_calls: 4,
            max_tokens: None,
            max_cost: constraints
                .max_cost_per_image
                .as_deref()
                .and_then(|value| value.parse().ok()),
        };
        let mut recovery = WorkflowDraftNode {
            id: "recovery".to_owned(),
            node_type: recovery_operation.id.clone(),
            kind: WorkflowNodeKind::Gate,
            inputs: vec![
                port("image", ArtifactKind::Image, false),
                port("primary", ArtifactKind::DetectionSet, false),
            ],
            outputs: vec![port("candidates", ArtifactKind::CandidateClusterSet, false)],
            model_binding: open_model.as_ref().map(|model| model.id.clone()),
            required_skills: enabled_skills.to_vec(),
            review_gate: true,
            gate: ReviewGate {
                required: true,
                allow_manual_override: true,
            },
            resources: ResourceRequirements {
                timeout_seconds: Some(project_schema.runtime.provider_request_timeout_seconds),
                ..ResourceRequirements::default()
            },
            ..WorkflowDraftNode::default()
        };
        recovery.parameters.insert(
            "queries".to_owned(),
            serde_json::to_value(&queries).unwrap_or_else(|_| serde_json::json!([])),
        );
        recovery.parameters.insert(
            "recovery_policy".to_owned(),
            serde_json::to_value(policy).unwrap_or_else(|_| serde_json::json!({})),
        );
        recovery.parameters.insert(
            "agent_budget".to_owned(),
            serde_json::to_value(agent_budget).unwrap_or_else(|_| serde_json::json!({})),
        );
        if recovery.model_binding.is_none() {
            unresolved_model_bindings.push(recovery.id.clone());
            warnings.push(
                "No available open-vocabulary fallback Model matches the Project Labels; bind one before publish."
                    .to_owned(),
            );
        }
        nodes.extend([detector, recovery]);
        edges.extend([
            edge("image", "image", "specialist", "image", None),
            edge("image", "image", "recovery", "image", None),
            edge("specialist", "detections", "recovery", "primary", None),
        ]);
        rationale.push(format!(
            "Uses specialist Model {:?} first and reserves open-vocabulary Model {} only for empty, low-score, domain-risk, or correction-risk evidence.",
            specialist.id,
            open_model
                .as_ref()
                .map_or("<unresolved>", |model| model.id.as_str())
        ));
        rationale.push(
            "High-confidence specialist evidence can finish without paying for the fallback call."
                .to_owned(),
        );
        estimated_model_calls_per_image = 1;
        final_artifact = ArtifactKind::CandidateClusterSet;
        final_node = "recovery";
    } else {
        let mut grounding = detection_model_node(
            "open_vocabulary",
            &open_operation.id,
            open_model.as_ref().map(|model| model.id.clone()),
            ArtifactKind::DetectionSet,
            enabled_skills,
        );
        grounding.parameters.insert(
            "queries".to_owned(),
            serde_json::to_value(&queries).unwrap_or_else(|_| serde_json::json!([])),
        );
        if grounding.model_binding.is_none() {
            unresolved_model_bindings.push(grounding.id.clone());
            warnings.push(
                "No available open-vocabulary Model is bound; resolve the binding before publish."
                    .to_owned(),
            );
        }
        nodes.push(grounding);
        edges.push(edge("image", "image", "open_vocabulary", "image", None));
        rationale.push(
            "No label-compatible specialist Model is available, so the Draft starts with open-vocabulary detection and requires review."
                .to_owned(),
        );
        let verification = add_crop_verification(
            &mut nodes,
            &mut edges,
            node_catalog,
            &models,
            enabled_skills,
            first_task.id.as_str(),
            &target_labels,
            project_schema.review.force_review_below,
        );
        if verification {
            rationale.push(
                "Adds crop classification as bounded verification before human review.".to_owned(),
            );
            estimated_model_calls_per_image = 2;
            final_artifact = ArtifactKind::AnnotationCandidateSet;
            final_node = "attach_verification";
        } else {
            warnings.push(
                "Crop verification was not added because a compatible Crop, Classification, Model, or Attach Result binding is unavailable."
                    .to_owned(),
            );
            estimated_model_calls_per_image = 1;
            final_artifact = ArtifactKind::DetectionSet;
            final_node = "open_vocabulary";
        }
    }

    let review = WorkflowDraftNode {
        id: "review".to_owned(),
        node_type: "review_gate".to_owned(),
        kind: WorkflowNodeKind::HumanReview,
        inputs: vec![port("candidates", final_artifact, true)],
        outputs: vec![port("candidates", final_artifact, true)],
        review_gate: true,
        gate: ReviewGate {
            required: true,
            allow_manual_override: true,
        },
        ..WorkflowDraftNode::default()
    };
    let commit = WorkflowDraftNode {
        id: "commit".to_owned(),
        node_type: "commit".to_owned(),
        kind: WorkflowNodeKind::Commit,
        inputs: vec![port("candidates", final_artifact, true)],
        ..WorkflowDraftNode::default()
    };
    nodes.extend([review, commit]);
    if final_node == "recovery" {
        edges.extend([
            edge(
                "recovery",
                "candidates",
                "commit",
                "candidates",
                Some("accept"),
            ),
            edge(
                "recovery",
                "candidates",
                "review",
                "candidates",
                Some("review"),
            ),
            edge(
                "recovery",
                "candidates",
                "review",
                "candidates",
                Some("verify"),
            ),
            edge(
                "recovery",
                "candidates",
                "review",
                "candidates",
                Some("reject"),
            ),
        ]);
    } else {
        edges.push(edge(
            final_node,
            artifact_port(final_artifact),
            "review",
            "candidates",
            None,
        ));
    }
    edges.push(edge("review", "candidates", "commit", "candidates", None));
    if detection_tasks.len() > 1 {
        warnings.push(
            "The deterministic baseline uses the first bounding-box task for crop verification; split or edit the Draft for task-specific mappings."
                .to_owned(),
        );
    }
    if let Some(max_nodes) = constraints.max_nodes
        && nodes.len() > max_nodes
    {
        warnings.push(format!(
            "suggestion has {} nodes, above configured maximum {max_nodes}",
            nodes.len()
        ));
    }
    Some(WorkflowSuggestion {
        draft: WorkflowDraft {
            schema_version: WORKFLOW_SCHEMA_VERSION,
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.to_owned(),
            name: format!("{} detection workflow", project_schema.project.name),
            status: WorkflowDraftStatus::Suggested,
            nodes,
            edges,
            enabled_skills: project_schema.project.enabled_skill_versions(),
            resource_versions: BTreeMap::new(),
            runtime_policies: BTreeMap::new(),
            allow_unvalidated_commit: false,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        },
        rationale,
        estimated_model_calls_per_image,
        estimated_latency_ms: Some(estimated_model_calls_per_image as u64 * 1_200),
        estimated_cost_tier: if estimated_model_calls_per_image == 1 {
            "low"
        } else {
            "medium"
        }
        .to_owned(),
        unresolved_model_bindings,
        warnings,
        alternatives: vec![
            "Accuracy-first: audit selected samples with both detector capabilities and compare evidence."
                .to_owned(),
            "Cost-first: keep specialist-first routing and lower fallback frequency only after Dry Run evidence."
                .to_owned(),
        ],
    })
}

fn find_node(
    catalog: &NodeRegistry,
    capability: Option<VisionCapability>,
    accepts: Option<ArtifactKind>,
    produces: ArtifactKind,
    accepts_empty: Option<bool>,
) -> Option<VisionModelDescriptorNode> {
    catalog.nodes().into_iter().find_map(|node| {
        let capability_matches =
            capability.is_none_or(|capability| node.required_capabilities.contains(&capability));
        let input_matches = accepts.is_none_or(|kind| node.accepts.contains(&kind));
        let empty_matches =
            accepts_empty.is_none_or(|expected| node.accepts.is_empty() == expected);
        (capability_matches && input_matches && empty_matches && node.produces.contains(&produces))
            .then_some(VisionModelDescriptorNode { id: node.id })
    })
}

struct VisionModelDescriptorNode {
    id: String,
}

fn select_detection_model<'a>(
    models: &'a [VisionModelDescriptor],
    capability: VisionCapability,
    target_labels: &[String],
    preferred: Option<&str>,
) -> Option<&'a VisionModelDescriptor> {
    let compatible = |model: &&VisionModelDescriptor| {
        model.status == ModelAvailabilityStatus::Available
            && model.capabilities.contains(&capability)
            && (capability != VisionCapability::ObjectDetection
                || model.output_contract.label_space.is_empty()
                || target_labels
                    .iter()
                    .all(|label| model.output_contract.label_space.contains(label)))
    };
    models
        .iter()
        .filter(compatible)
        .find(|model| preferred == Some(model.id.as_str()))
        .or_else(|| models.iter().find(compatible))
}

fn detection_model_node(
    id: &str,
    operation: &str,
    model_binding: Option<String>,
    output: ArtifactKind,
    enabled_skills: &[String],
) -> WorkflowDraftNode {
    WorkflowDraftNode {
        id: id.to_owned(),
        node_type: operation.to_owned(),
        kind: WorkflowNodeKind::VisionModel,
        inputs: vec![port("image", ArtifactKind::Image, false)],
        outputs: vec![port(artifact_port(output), output, false)],
        model_binding,
        required_skills: enabled_skills.to_vec(),
        retry_policy: RetryPolicy { max_attempts: 1 },
        ..WorkflowDraftNode::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn add_crop_verification(
    nodes: &mut Vec<WorkflowDraftNode>,
    edges: &mut Vec<WorkflowEdge>,
    node_catalog: &NodeRegistry,
    models: &[VisionModelDescriptor],
    enabled_skills: &[String],
    task_id: &str,
    labels: &[String],
    minimum_confidence: f32,
) -> bool {
    let crop = find_node(
        node_catalog,
        None,
        Some(ArtifactKind::DetectionSet),
        ArtifactKind::CropSet,
        None,
    );
    let classifier = find_node(
        node_catalog,
        Some(VisionCapability::Classification),
        Some(ArtifactKind::CropSet),
        ArtifactKind::ClassificationSet,
        None,
    );
    let attach = find_node(
        node_catalog,
        None,
        Some(ArtifactKind::ClassificationSet),
        ArtifactKind::AnnotationCandidateSet,
        None,
    );
    let classifier_model = models.iter().find(|model| {
        model.status == ModelAvailabilityStatus::Available
            && model
                .capabilities
                .contains(&VisionCapability::Classification)
    });
    let (Some(crop), Some(classifier), Some(attach), Some(classifier_model)) =
        (crop, classifier, attach, classifier_model)
    else {
        return false;
    };
    nodes.push(WorkflowDraftNode {
        id: "crop_verification".to_owned(),
        node_type: crop.id,
        kind: WorkflowNodeKind::Transform,
        inputs: vec![
            port("image", ArtifactKind::Image, false),
            port("detections", ArtifactKind::DetectionSet, false),
        ],
        outputs: vec![port("crops", ArtifactKind::CropSet, false)],
        parameters: BTreeMap::from([("padding".to_owned(), serde_json::json!(0.1))]),
        ..WorkflowDraftNode::default()
    });
    nodes.push(WorkflowDraftNode {
        id: "classify_crops".to_owned(),
        node_type: classifier.id,
        kind: WorkflowNodeKind::VisionModel,
        inputs: vec![port("crops", ArtifactKind::CropSet, false)],
        outputs: vec![port(
            "classifications",
            ArtifactKind::ClassificationSet,
            false,
        )],
        model_binding: Some(classifier_model.id.clone()),
        required_skills: enabled_skills.to_vec(),
        parameters: BTreeMap::from([
            ("labels".to_owned(), serde_json::json!(labels)),
            (
                "minimum_confidence".to_owned(),
                serde_json::json!(minimum_confidence.clamp(0.0, 1.0)),
            ),
        ]),
        retry_policy: RetryPolicy { max_attempts: 1 },
        ..WorkflowDraftNode::default()
    });
    nodes.push(WorkflowDraftNode {
        id: "attach_verification".to_owned(),
        node_type: attach.id,
        kind: WorkflowNodeKind::CandidateMerge,
        inputs: vec![
            port("detections", ArtifactKind::DetectionSet, false),
            port("classifications", ArtifactKind::ClassificationSet, false),
        ],
        outputs: vec![port(
            "candidates",
            ArtifactKind::AnnotationCandidateSet,
            false,
        )],
        parameters: BTreeMap::from([
            ("task_id".to_owned(), serde_json::json!(task_id)),
            (
                "class_mapping".to_owned(),
                serde_json::Value::Object(
                    labels
                        .iter()
                        .map(|label| (label.clone(), serde_json::json!(label)))
                        .collect(),
                ),
            ),
        ]),
        ..WorkflowDraftNode::default()
    });
    edges.extend([
        edge("image", "image", "crop_verification", "image", None),
        edge(
            "open_vocabulary",
            "detections",
            "crop_verification",
            "detections",
            None,
        ),
        edge(
            "crop_verification",
            "crops",
            "classify_crops",
            "crops",
            None,
        ),
        edge(
            "open_vocabulary",
            "detections",
            "attach_verification",
            "detections",
            None,
        ),
        edge(
            "classify_crops",
            "classifications",
            "attach_verification",
            "classifications",
            None,
        ),
    ]);
    true
}

fn port(id: impl Into<String>, artifact_type: ArtifactKind, multiple: bool) -> NodePort {
    NodePort {
        id: id.into(),
        artifact_type,
        required: true,
        multiple,
    }
}

fn edge(
    from_node: &str,
    from_port: &str,
    to_node: &str,
    to_port: &str,
    route: Option<&str>,
) -> WorkflowEdge {
    WorkflowEdge {
        from_node: from_node.to_owned(),
        from_port: from_port.to_owned(),
        to_node: to_node.to_owned(),
        to_port: to_port.to_owned(),
        route: route.map(ToOwned::to_owned),
    }
}

const fn artifact_port(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Image => "image",
        ArtifactKind::DetectionSet => "detections",
        ArtifactKind::BoxPromptSet | ArtifactKind::PointPromptSet => "prompts",
        ArtifactKind::MaskSet => "masks",
        ArtifactKind::PolygonSet => "polygons",
        ArtifactKind::CandidateClusterSet | ArtifactKind::AnnotationCandidateSet => "candidates",
        ArtifactKind::CropSet => "crops",
        ArtifactKind::ClassificationSet => "classifications",
        ArtifactKind::Classification
        | ArtifactKind::BoundingBox
        | ArtifactKind::Keypoints
        | ArtifactKind::Polyline
        | ArtifactKind::Polygon
        | ArtifactKind::SemanticMask
        | ArtifactKind::InstanceMask
        | ArtifactKind::Attributes
        | ArtifactKind::Relations => "artifacts",
    }
}

fn system_node(
    id: &str,
    operation: &str,
    kind: WorkflowNodeKind,
    depends_on: Vec<String>,
    inputs: Vec<NodePort>,
    outputs: Vec<NodePort>,
) -> WorkflowDraftNode {
    WorkflowDraftNode {
        id: id.to_owned(),
        node_type: operation.to_owned(),
        kind,
        depends_on,
        inputs,
        outputs,
        model_binding: None,
        model_profile_binding: None,
        required_skills: Vec::new(),
        validators: Vec::new(),
        refiners: Vec::new(),
        fallback: None,
        max_retries: 0,
        review_gate: kind == WorkflowNodeKind::HumanReview,
        parameters: BTreeMap::new(),
        retry_policy: RetryPolicy::default(),
        fallback_policy: FallbackPolicy::default(),
        gate: ReviewGate {
            required: kind == WorkflowNodeKind::HumanReview,
            allow_manual_override: false,
        },
        resources: ResourceRequirements::default(),
    }
}

const fn artifact_for_task(kind: TaskKind) -> ArtifactKind {
    match kind {
        TaskKind::Classification => ArtifactKind::Classification,
        TaskKind::BoundingBox => ArtifactKind::BoundingBox,
        TaskKind::Keypoints => ArtifactKind::Keypoints,
        TaskKind::Polyline => ArtifactKind::Polyline,
        TaskKind::Polygon => ArtifactKind::Polygon,
        TaskKind::SemanticMask => ArtifactKind::SemanticMask,
        TaskKind::InstanceMask => ArtifactKind::InstanceMask,
        TaskKind::Attributes => ArtifactKind::Attributes,
        TaskKind::Relations => ArtifactKind::Relations,
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WorkflowStaticValidator;

impl WorkflowStaticValidator {
    #[must_use]
    pub fn validate(
        &self,
        draft: &WorkflowDraft,
        node_catalog: &NodeRegistry,
        model_registry: &ModelRegistry,
    ) -> WorkflowValidationReport {
        self.validate_for_publish(
            draft,
            node_catalog,
            model_registry,
            &ValidationCatalog::default(),
            &draft
                .enabled_skills
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            false,
        )
    }

    #[must_use]
    pub fn validate_for_publish(
        &self,
        draft: &WorkflowDraft,
        node_catalog: &NodeRegistry,
        model_registry: &ModelRegistry,
        extensions: &ValidationCatalog,
        enabled_skills: &BTreeSet<String>,
        publishing: bool,
    ) -> WorkflowValidationReport {
        let mut issues = Vec::new();
        let ids = draft
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<BTreeSet<_>>();
        if ids.len() != draft.nodes.len() {
            issues.push(issue(
                "duplicate_node_id",
                "nodes",
                "node ids must be unique",
            ));
        }
        let indexes = draft
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id.as_str(), index))
            .collect::<BTreeMap<_, _>>();

        for resource_id in draft.resource_versions.keys() {
            if !extensions.resources.contains(resource_id) {
                issues.push(issue(
                    "unknown_skill_resource",
                    &format!("resource_versions.{resource_id}"),
                    &format!("Skill resource {resource_id:?} is not registered"),
                ));
            }
        }

        for (index, node) in draft.nodes.iter().enumerate() {
            let path = format!("nodes[{index}]");
            let input_ids = node
                .inputs
                .iter()
                .map(|port| port.id.as_str())
                .collect::<BTreeSet<_>>();
            let output_ids = node
                .outputs
                .iter()
                .map(|port| port.id.as_str())
                .collect::<BTreeSet<_>>();
            if input_ids.len() != node.inputs.len() {
                issues.push(issue(
                    "duplicate_input_port",
                    &format!("{path}.inputs"),
                    "input port ids must be unique",
                ));
            }
            if output_ids.len() != node.outputs.len() {
                issues.push(issue(
                    "duplicate_output_port",
                    &format!("{path}.outputs"),
                    "output port ids must be unique",
                ));
            }
            let Some(descriptor) = node_catalog.get(&node.node_type) else {
                issues.push(issue(
                    "unknown_node",
                    &format!("{path}.node_type"),
                    &format!("node operation {:?} is not registered", node.node_type),
                ));
                continue;
            };
            for input in &node.inputs {
                if !descriptor.accepts.is_empty()
                    && !descriptor.accepts.contains(&input.artifact_type)
                {
                    issues.push(issue(
                        "node_input_type_unsupported",
                        &format!("{path}.inputs.{}", input.id),
                        &format!(
                            "operation {:?} does not accept {:?}",
                            node.node_type, input.artifact_type
                        ),
                    ));
                }
            }
            for output in &node.outputs {
                if !descriptor.produces.contains(&output.artifact_type) {
                    issues.push(issue(
                        "node_output_type_unsupported",
                        &format!("{path}.outputs.{}", output.id),
                        &format!(
                            "operation {:?} does not produce {:?}",
                            node.node_type, output.artifact_type
                        ),
                    ));
                }
            }
            for (skill_index, skill) in node.required_skills.iter().enumerate() {
                if !enabled_skills.contains(skill) {
                    issues.push(issue(
                        "required_skill_not_enabled",
                        &format!("{path}.required_skills[{skill_index}]"),
                        &format!("required Skill {skill:?} is not enabled"),
                    ));
                }
            }
            for (validator_index, validator) in node.validators.iter().enumerate() {
                if !extensions.validators.contains(validator) {
                    issues.push(issue(
                        "unknown_validator",
                        &format!("{path}.validators[{validator_index}]"),
                        &format!("Validator {validator:?} is not registered"),
                    ));
                }
            }
            for (refiner_index, refiner) in node.refiners.iter().enumerate() {
                if !extensions.refiners.contains(refiner) {
                    issues.push(issue(
                        "unknown_refiner",
                        &format!("{path}.refiners[{refiner_index}]"),
                        &format!("Refiner {refiner:?} is not registered"),
                    ));
                }
            }
            if node.effective_retry_limit() > MAX_WORKFLOW_RETRIES {
                issues.push(issue(
                    "retry_limit_exceeded",
                    &format!("{path}.retry_policy.max_attempts"),
                    &format!("retry limit must be at most {MAX_WORKFLOW_RETRIES}"),
                ));
            }
            if descriptor.required_capabilities.is_empty() {
                continue;
            }
            let Some(model_id) = node.model_binding.as_deref() else {
                issues.push(issue(
                    "unresolved_model_binding",
                    &format!("{path}.model_binding"),
                    "this operation requires a registered model binding",
                ));
                continue;
            };
            match model_registry.resolve(model_id) {
                Ok((model, _)) => {
                    for capability in &descriptor.required_capabilities {
                        if !model.capabilities.contains(capability) {
                            issues.push(issue(
                                "model_capability_mismatch",
                                &format!("{path}.model_binding"),
                                &format!("model {model_id:?} lacks {capability:?}"),
                            ));
                        }
                    }
                    if requests_visual_prompt(&node.parameters)
                        && !model.input_contract.supports_visual_prompt
                    {
                        issues.push(issue(
                            "visual_prompt_unsupported",
                            &format!("{path}.parameters"),
                            &format!("model {model_id:?} does not advertise visual prompt support"),
                        ));
                    }
                }
                Err(error) => issues.push(issue(
                    "unknown_model",
                    &format!("{path}.model_binding"),
                    &error.to_string(),
                )),
            }
        }

        validate_edges(draft, &indexes, &mut issues);
        validate_required_inputs(draft, &mut issues);
        validate_fallbacks(draft, &ids, &mut issues);
        let execution_order = topological_order(draft).unwrap_or_else(|cycle| {
            issues.push(issue("workflow_cycle", "edges", &cycle));
            Vec::new()
        });
        if draft.schema_version >= WORKFLOW_SCHEMA_VERSION {
            validate_reachability_and_terminals(draft, &mut issues);
            validate_commit_safety(draft, &mut issues);
        }
        if publishing && draft.schema_version != WORKFLOW_SCHEMA_VERSION {
            issues.push(issue(
                "unsupported_workflow_schema",
                "schema_version",
                &format!("publishing requires schema version {WORKFLOW_SCHEMA_VERSION}"),
            ));
        }
        WorkflowValidationReport {
            valid: issues.iter().all(|issue| !issue.blocking),
            issues,
            execution_order,
        }
    }
}

fn requests_visual_prompt(parameters: &BTreeMap<String, serde_json::Value>) -> bool {
    parameters.iter().any(|(name, value)| {
        matches!(
            name.as_str(),
            "visual_prompt" | "visual_prompt_box" | "visual_exemplar" | "exemplar_image"
        ) && !value.is_null()
    })
}

fn validate_edges(
    draft: &WorkflowDraft,
    indexes: &BTreeMap<&str, usize>,
    issues: &mut Vec<WorkflowValidationIssue>,
) {
    for (edge_index, edge) in draft.edges.iter().enumerate() {
        let Some(&from_index) = indexes.get(edge.from_node.as_str()) else {
            issues.push(issue(
                "unknown_edge_node",
                &format!("edges[{edge_index}].from_node"),
                &format!("node {:?} does not exist", edge.from_node),
            ));
            continue;
        };
        let Some(&to_index) = indexes.get(edge.to_node.as_str()) else {
            issues.push(issue(
                "unknown_edge_node",
                &format!("edges[{edge_index}].to_node"),
                &format!("node {:?} does not exist", edge.to_node),
            ));
            continue;
        };
        let output = draft.nodes[from_index]
            .outputs
            .iter()
            .find(|port| port.id == edge.from_port);
        let input = draft.nodes[to_index]
            .inputs
            .iter()
            .find(|port| port.id == edge.to_port);
        if draft.nodes[from_index].outputs.is_empty() && draft.nodes[to_index].inputs.is_empty() {
            continue;
        }
        let Some(output) = output else {
            issues.push(issue(
                "unknown_output_port",
                &format!("edges[{edge_index}].from_port"),
                &format!(
                    "output port {:?} does not exist on node {:?}",
                    edge.from_port, edge.from_node
                ),
            ));
            continue;
        };
        let Some(input) = input else {
            issues.push(issue(
                "unknown_input_port",
                &format!("edges[{edge_index}].to_port"),
                &format!(
                    "input port {:?} does not exist on node {:?}",
                    edge.to_port, edge.to_node
                ),
            ));
            continue;
        };
        if output.artifact_type != input.artifact_type {
            issues.push(issue(
                "artifact_type_mismatch",
                &format!("nodes[{to_index}].inputs.{}", input.id),
                &format!(
                    "edge from {}.{} produces {:?}, but this port accepts {:?}",
                    edge.from_node, edge.from_port, output.artifact_type, input.artifact_type
                ),
            ));
        }
    }
}

fn validate_required_inputs(draft: &WorkflowDraft, issues: &mut Vec<WorkflowValidationIssue>) {
    for (node_index, node) in draft.nodes.iter().enumerate() {
        for input in node.inputs.iter().filter(|port| port.required) {
            let count = draft
                .edges
                .iter()
                .filter(|edge| edge.to_node == node.id && edge.to_port == input.id)
                .count();
            if count == 0 {
                issues.push(issue(
                    "missing_required_input",
                    &format!("nodes[{node_index}].inputs.{}", input.id),
                    "required input port is not connected",
                ));
            } else if count > 1 && !input.multiple {
                issues.push(issue(
                    "multiple_edges_to_single_input",
                    &format!("nodes[{node_index}].inputs.{}", input.id),
                    "input port does not accept multiple edges",
                ));
            }
        }
    }
}

fn validate_fallbacks(
    draft: &WorkflowDraft,
    ids: &BTreeSet<&str>,
    issues: &mut Vec<WorkflowValidationIssue>,
) {
    let fallbacks = draft
        .nodes
        .iter()
        .filter_map(|node| {
            node.effective_fallback()
                .map(|target| (node.id.as_str(), target))
        })
        .collect::<BTreeMap<_, _>>();
    for (index, node) in draft.nodes.iter().enumerate() {
        if let Some(target) = node.effective_fallback()
            && !ids.contains(target)
        {
            issues.push(issue(
                "unknown_fallback",
                &format!("nodes[{index}].fallback"),
                &format!("fallback {target:?} is not a draft node"),
            ));
        }
        let mut seen = BTreeSet::new();
        let mut current = node.id.as_str();
        while let Some(next) = fallbacks.get(current).copied() {
            if !seen.insert(current) {
                issues.push(issue(
                    "fallback_cycle",
                    &format!("nodes[{index}].fallback"),
                    "fallback path contains a cycle",
                ));
                break;
            }
            current = next;
        }
    }
}

fn validate_reachability_and_terminals(
    draft: &WorkflowDraft,
    issues: &mut Vec<WorkflowValidationIssue>,
) {
    let explicit_inputs = draft
        .nodes
        .iter()
        .filter(|node| node.kind == WorkflowNodeKind::ImageInput)
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    let roots = if explicit_inputs.is_empty() {
        draft
            .nodes
            .iter()
            .filter(|node| incoming_nodes(draft, &node.id).is_empty())
            .map(|node| node.id.clone())
            .collect::<Vec<_>>()
    } else {
        explicit_inputs
    };
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::from(roots);
    while let Some(current) = queue.pop_front() {
        if !reachable.insert(current.clone()) {
            continue;
        }
        for next in outgoing_nodes(draft, &current) {
            queue.push_back(next);
        }
    }
    for (index, node) in draft.nodes.iter().enumerate() {
        if !reachable.contains(&node.id) {
            issues.push(issue(
                "unreachable_node",
                &format!("nodes[{index}]"),
                &format!("node {:?} is unreachable from workflow input", node.id),
            ));
        }
    }
    let terminals = draft
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.kind,
                WorkflowNodeKind::Commit | WorkflowNodeKind::Export
            )
        })
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    if terminals.is_empty() {
        issues.push(issue(
            "no_terminal_path",
            "nodes",
            "workflow must contain a Commit or Export terminal node",
        ));
        return;
    }
    for (index, node) in draft.nodes.iter().enumerate() {
        if !can_reach_any(draft, &node.id, &terminals) {
            issues.push(issue(
                "no_terminal_path",
                &format!("nodes[{index}]"),
                &format!("node {:?} has no path to Commit or Export", node.id),
            ));
        }
    }
}

fn validate_commit_safety(draft: &WorkflowDraft, issues: &mut Vec<WorkflowValidationIssue>) {
    if draft.allow_unvalidated_commit {
        return;
    }
    let safe_nodes = draft
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.kind,
                WorkflowNodeKind::Validator | WorkflowNodeKind::HumanReview
            ) || node.review_gate
                || node.gate.required
        })
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    for (index, node) in draft
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == WorkflowNodeKind::Commit)
    {
        let ancestors = ancestors_of(draft, &node.id);
        if ancestors.is_disjoint(&safe_nodes) {
            issues.push(issue(
                "unsafe_commit",
                &format!("nodes[{index}]"),
                "Commit requires an upstream Validator or HumanReview gate, or an explicit allow_unvalidated_commit policy",
            ));
        }
    }
}

fn incoming_nodes(draft: &WorkflowDraft, id: &str) -> Vec<String> {
    let mut incoming = draft
        .edges
        .iter()
        .filter(|edge| edge.to_node == id)
        .map(|edge| edge.from_node.clone())
        .collect::<Vec<_>>();
    if incoming.is_empty() {
        incoming.extend(
            draft
                .nodes
                .iter()
                .find(|node| node.id == id)
                .into_iter()
                .flat_map(|node| node.depends_on.clone()),
        );
    }
    incoming
}

fn outgoing_nodes(draft: &WorkflowDraft, id: &str) -> Vec<String> {
    let mut outgoing = draft
        .edges
        .iter()
        .filter(|edge| edge.from_node == id)
        .map(|edge| edge.to_node.clone())
        .collect::<Vec<_>>();
    let compatibility_edges = draft
        .nodes
        .iter()
        .filter(|node| node.depends_on.iter().any(|dependency| dependency == id))
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    for node_id in compatibility_edges {
        if !outgoing.contains(&node_id) {
            outgoing.push(node_id);
        }
    }
    outgoing
}

fn can_reach_any(draft: &WorkflowDraft, start: &str, targets: &BTreeSet<&str>) -> bool {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start.to_owned()]);
    while let Some(current) = queue.pop_front() {
        if targets.contains(current.as_str()) {
            return true;
        }
        if seen.insert(current.clone()) {
            queue.extend(outgoing_nodes(draft, &current));
        }
    }
    false
}

fn ancestors_of<'a>(draft: &'a WorkflowDraft, start: &str) -> BTreeSet<&'a str> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start.to_owned()]);
    while let Some(current) = queue.pop_front() {
        for parent in incoming_nodes(draft, &current) {
            if let Some(node) = draft.nodes.iter().find(|node| node.id == parent)
                && seen.insert(node.id.as_str())
            {
                queue.push_back(node.id.clone());
            }
        }
    }
    seen
}

fn issue(code: &str, path: &str, message: &str) -> WorkflowValidationIssue {
    WorkflowValidationIssue {
        code: code.to_owned(),
        path: path.to_owned(),
        message: message.to_owned(),
        blocking: true,
    }
}

fn topological_order(draft: &WorkflowDraft) -> Result<Vec<String>, String> {
    let mut remaining = draft
        .nodes
        .iter()
        .map(|node| {
            (
                node.id.clone(),
                incoming_nodes(draft, &node.id)
                    .into_iter()
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut order = Vec::with_capacity(draft.nodes.len());
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .filter(|(_, dependencies)| dependencies.iter().all(|id| order.contains(id)))
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return Err("workflow contains a dependency cycle".to_owned());
        }
        for id in ready {
            remaining.remove(&id);
            order.push(id);
        }
    }
    Ok(order)
}

#[must_use]
pub const fn all_artifact_kinds() -> [ArtifactKind; 19] {
    [
        ArtifactKind::Image,
        ArtifactKind::DetectionSet,
        ArtifactKind::BoxPromptSet,
        ArtifactKind::PointPromptSet,
        ArtifactKind::MaskSet,
        ArtifactKind::PolygonSet,
        ArtifactKind::CandidateClusterSet,
        ArtifactKind::CropSet,
        ArtifactKind::ClassificationSet,
        ArtifactKind::AnnotationCandidateSet,
        ArtifactKind::Classification,
        ArtifactKind::BoundingBox,
        ArtifactKind::Keypoints,
        ArtifactKind::Polyline,
        ArtifactKind::Polygon,
        ArtifactKind::SemanticMask,
        ArtifactKind::InstanceMask,
        ArtifactKind::Attributes,
        ArtifactKind::Relations,
    ]
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::{
        CoreResult, VisionBackendKind, VisionInferenceRequest, VisionInferenceResponse,
        VisionModelBackend, VisionNodeDescriptor,
    };

    struct ClassificationBackend;

    #[async_trait]
    impl VisionModelBackend for ClassificationBackend {
        fn id(&self) -> &str {
            "classification-backend"
        }

        fn kind(&self) -> VisionBackendKind {
            VisionBackendKind::Mock
        }

        fn capabilities(&self) -> Vec<VisionCapability> {
            vec![VisionCapability::Classification]
        }

        async fn infer(
            &self,
            _request: VisionInferenceRequest,
            _cancellation: CancellationToken,
        ) -> CoreResult<VisionInferenceResponse> {
            Ok(VisionInferenceResponse::default())
        }
    }

    struct AdvisorBackend;

    #[async_trait]
    impl VisionModelBackend for AdvisorBackend {
        fn id(&self) -> &str {
            "advisor-backend"
        }

        fn kind(&self) -> VisionBackendKind {
            VisionBackendKind::Mock
        }

        fn capabilities(&self) -> Vec<VisionCapability> {
            vec![
                VisionCapability::OpenVocabularyDetection,
                VisionCapability::ObjectDetection,
                VisionCapability::Classification,
            ]
        }

        async fn infer(
            &self,
            _request: VisionInferenceRequest,
            _cancellation: CancellationToken,
        ) -> CoreResult<VisionInferenceResponse> {
            Ok(VisionInferenceResponse::default())
        }
    }

    fn node(id: &str, kind: WorkflowNodeKind) -> WorkflowDraftNode {
        WorkflowDraftNode {
            id: id.to_owned(),
            node_type: id.to_owned(),
            kind,
            ..WorkflowDraftNode::default()
        }
    }

    fn draft(nodes: Vec<WorkflowDraftNode>, edges: Vec<WorkflowEdge>) -> WorkflowDraft {
        let now = Utc::now();
        WorkflowDraft {
            schema_version: WORKFLOW_SCHEMA_VERSION,
            id: "workflow".to_owned(),
            project_id: "project".to_owned(),
            name: "test".to_owned(),
            status: WorkflowDraftStatus::Editing,
            nodes,
            edges,
            enabled_skills: BTreeMap::new(),
            resource_versions: BTreeMap::new(),
            runtime_policies: BTreeMap::new(),
            allow_unvalidated_commit: true,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn catalog(ids: &[(&str, Vec<VisionCapability>)]) -> NodeRegistry {
        let mut registry = NodeRegistry::new();
        for (id, capabilities) in ids {
            registry
                .register(VisionNodeDescriptor {
                    id: (*id).to_owned(),
                    display_name: (*id).to_owned(),
                    required_capabilities: capabilities.clone(),
                    accepts: all_artifact_kinds().to_vec(),
                    produces: all_artifact_kinds().to_vec(),
                    deterministic: capabilities.is_empty(),
                })
                .expect("node registration");
        }
        registry
    }

    fn detection_project() -> ProjectSchema {
        ProjectSchema::from_yaml(
            r"
version: 1
project:
  name: Detection advisor fixture
  language: en
dataset:
  root: images
runtime: {}
tasks:
  - id: objects
    kind: bounding_box
    labels: [target]
review:
  auto_accept_confidence: 0.85
  force_review_below: 0.55
export:
  formats: [native]
",
        )
        .expect("Project Schema")
    }

    fn detection_catalog() -> NodeRegistry {
        let mut nodes = NodeRegistry::new();
        for descriptor in [
            VisionNodeDescriptor {
                id: "image_input".to_owned(),
                display_name: "Image Input".to_owned(),
                required_capabilities: Vec::new(),
                accepts: Vec::new(),
                produces: vec![ArtifactKind::Image],
                deterministic: true,
            },
            VisionNodeDescriptor {
                id: "open_detect".to_owned(),
                display_name: "Open Detection".to_owned(),
                required_capabilities: vec![VisionCapability::OpenVocabularyDetection],
                accepts: vec![ArtifactKind::Image],
                produces: vec![ArtifactKind::DetectionSet],
                deterministic: false,
            },
            VisionNodeDescriptor {
                id: "specialist_detect".to_owned(),
                display_name: "Specialist Detection".to_owned(),
                required_capabilities: vec![VisionCapability::ObjectDetection],
                accepts: vec![ArtifactKind::Image],
                produces: vec![ArtifactKind::DetectionSet],
                deterministic: false,
            },
            VisionNodeDescriptor {
                id: "detection_recovery".to_owned(),
                display_name: "Detection Recovery".to_owned(),
                required_capabilities: vec![VisionCapability::OpenVocabularyDetection],
                accepts: vec![ArtifactKind::Image, ArtifactKind::DetectionSet],
                produces: vec![ArtifactKind::CandidateClusterSet],
                deterministic: false,
            },
            VisionNodeDescriptor {
                id: "crop".to_owned(),
                display_name: "Crop".to_owned(),
                required_capabilities: Vec::new(),
                accepts: vec![ArtifactKind::Image, ArtifactKind::DetectionSet],
                produces: vec![ArtifactKind::CropSet],
                deterministic: true,
            },
            VisionNodeDescriptor {
                id: "classify".to_owned(),
                display_name: "Classify".to_owned(),
                required_capabilities: vec![VisionCapability::Classification],
                accepts: vec![ArtifactKind::CropSet],
                produces: vec![ArtifactKind::ClassificationSet],
                deterministic: false,
            },
            VisionNodeDescriptor {
                id: "attach".to_owned(),
                display_name: "Attach".to_owned(),
                required_capabilities: Vec::new(),
                accepts: vec![ArtifactKind::DetectionSet, ArtifactKind::ClassificationSet],
                produces: vec![ArtifactKind::AnnotationCandidateSet],
                deterministic: true,
            },
            VisionNodeDescriptor {
                id: "review_gate".to_owned(),
                display_name: "Review".to_owned(),
                required_capabilities: Vec::new(),
                accepts: all_artifact_kinds().to_vec(),
                produces: all_artifact_kinds().to_vec(),
                deterministic: true,
            },
            VisionNodeDescriptor {
                id: "commit".to_owned(),
                display_name: "Commit".to_owned(),
                required_capabilities: Vec::new(),
                accepts: all_artifact_kinds().to_vec(),
                produces: all_artifact_kinds().to_vec(),
                deterministic: true,
            },
        ] {
            nodes.register(descriptor).expect("node");
        }
        nodes
    }

    fn detection_models(include_specialist: bool) -> ModelRegistry {
        let mut models = ModelRegistry::new();
        models
            .register_backend(Arc::new(AdvisorBackend))
            .expect("backend");
        for (id, capability) in [
            ("open-model", VisionCapability::OpenVocabularyDetection),
            ("classifier-model", VisionCapability::Classification),
        ] {
            models
                .register_model(VisionModelDescriptor {
                    id: id.to_owned(),
                    backend_id: "advisor-backend".to_owned(),
                    capabilities: vec![capability],
                    ..VisionModelDescriptor::default()
                })
                .expect("model");
        }
        if include_specialist {
            models
                .register_model(VisionModelDescriptor {
                    id: "specialist-model".to_owned(),
                    backend_id: "advisor-backend".to_owned(),
                    capabilities: vec![VisionCapability::ObjectDetection],
                    output_contract: crate::ModelOutputContract {
                        label_space: vec!["target".to_owned()],
                        ..crate::ModelOutputContract::default()
                    },
                    ..VisionModelDescriptor::default()
                })
                .expect("specialist model");
        }
        models
    }

    #[test]
    fn advisor_suggests_open_vocabulary_crop_verification_for_cold_start() {
        let project = detection_project();
        let nodes = detection_catalog();
        let models = detection_models(false);
        let suggestion = RegistryWorkflowAdvisor.suggest_workflow(
            "project",
            &project,
            &[],
            &nodes,
            &models,
            &WorkflowConstraints::default(),
        );
        assert_eq!(suggestion.draft.status, WorkflowDraftStatus::Suggested);
        assert!(
            suggestion
                .draft
                .nodes
                .iter()
                .any(|node| node.id == "open_vocabulary")
        );
        assert!(
            suggestion
                .draft
                .nodes
                .iter()
                .any(|node| node.id == "crop_verification")
        );
        assert!(
            suggestion
                .rationale
                .iter()
                .any(|item| item.contains("No label-compatible specialist"))
        );
        let report = WorkflowStaticValidator.validate(&suggestion.draft, &nodes, &models);
        assert!(report.valid, "{:#?}", report.issues);
    }

    #[test]
    fn advisor_suggests_specialist_first_with_bounded_recovery() {
        let project = detection_project();
        let nodes = detection_catalog();
        let models = detection_models(true);
        let suggestion = RegistryWorkflowAdvisor.suggest_workflow(
            "project",
            &project,
            &[],
            &nodes,
            &models,
            &WorkflowConstraints {
                max_cost_per_image: Some("0.10".to_owned()),
                ..WorkflowConstraints::default()
            },
        );
        let specialist = suggestion
            .draft
            .nodes
            .iter()
            .find(|node| node.id == "specialist")
            .expect("specialist-first node");
        assert_eq!(
            specialist.model_binding.as_deref(),
            Some("specialist-model")
        );
        let recovery = suggestion
            .draft
            .nodes
            .iter()
            .find(|node| node.id == "recovery")
            .expect("Recovery Agent node");
        assert_eq!(recovery.model_binding.as_deref(), Some("open-model"));
        assert_eq!(suggestion.estimated_model_calls_per_image, 1);
        assert_eq!(suggestion.draft.status, WorkflowDraftStatus::Suggested);
        let policy: DetectionRecoveryPolicy =
            serde_json::from_value(recovery.parameters["recovery_policy"].clone())
                .expect("Recovery policy");
        assert_eq!(policy.max_fallback_calls, 1);
        let report = WorkflowStaticValidator.validate(&suggestion.draft, &nodes, &models);
        assert!(report.valid, "{:#?}", report.issues);
    }

    #[test]
    fn dependency_cycle_is_rejected() {
        let mut first = node("first", WorkflowNodeKind::Transform);
        first.depends_on = vec!["commit".to_owned()];
        let mut commit = node("commit", WorkflowNodeKind::Commit);
        commit.depends_on = vec!["first".to_owned()];
        let workflow = draft(vec![first, commit], Vec::new());
        let report = WorkflowStaticValidator.validate(
            &workflow,
            &catalog(&[("first", Vec::new()), ("commit", Vec::new())]),
            &ModelRegistry::new(),
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "workflow_cycle")
        );
    }

    #[test]
    fn port_type_error_has_exact_input_path() {
        let mut source = node("source", WorkflowNodeKind::Transform);
        source.outputs.push(NodePort {
            id: "candidates".to_owned(),
            artifact_type: ArtifactKind::BoundingBox,
            required: true,
            multiple: true,
        });
        let mut commit = node("commit", WorkflowNodeKind::Commit);
        commit.inputs.push(NodePort {
            id: "candidates".to_owned(),
            artifact_type: ArtifactKind::SemanticMask,
            required: true,
            multiple: true,
        });
        let workflow = draft(
            vec![source, commit],
            vec![WorkflowEdge {
                from_node: "source".to_owned(),
                from_port: "candidates".to_owned(),
                to_node: "commit".to_owned(),
                to_port: "candidates".to_owned(),
                route: None,
            }],
        );
        let report = WorkflowStaticValidator.validate(
            &workflow,
            &catalog(&[("source", Vec::new()), ("commit", Vec::new())]),
            &ModelRegistry::new(),
        );
        assert!(report.issues.iter().any(|issue| {
            issue.code == "artifact_type_mismatch" && issue.path == "nodes[1].inputs.candidates"
        }));
    }

    #[test]
    fn unresolved_model_binding_blocks_publish() {
        let mut vision = node("vision", WorkflowNodeKind::VisionLanguageModel);
        vision.outputs.push(NodePort {
            id: "result".to_owned(),
            artifact_type: ArtifactKind::Classification,
            required: true,
            multiple: false,
        });
        let mut commit = node("commit", WorkflowNodeKind::Commit);
        commit.depends_on.push("vision".to_owned());
        let workflow = draft(vec![vision, commit], Vec::new());
        let report = WorkflowStaticValidator.validate_for_publish(
            &workflow,
            &catalog(&[
                ("vision", vec![VisionCapability::VisionLanguage]),
                ("commit", Vec::new()),
            ]),
            &ModelRegistry::new(),
            &ValidationCatalog::default(),
            &BTreeSet::new(),
            true,
        );
        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "unresolved_model_binding")
        );
    }

    #[test]
    fn model_capability_mismatch_blocks_publish() {
        let mut vision = node("vision", WorkflowNodeKind::VisionLanguageModel);
        vision.model_binding = Some("classifier".to_owned());
        let mut commit = node("commit", WorkflowNodeKind::Commit);
        commit.depends_on.push("vision".to_owned());
        let workflow = draft(vec![vision, commit], Vec::new());
        let mut models = ModelRegistry::new();
        models
            .register_backend(Arc::new(ClassificationBackend))
            .expect("backend");
        models
            .register_model(VisionModelDescriptor {
                id: "classifier".to_owned(),
                backend_id: "classification-backend".to_owned(),
                capabilities: vec![VisionCapability::Classification],
                ..VisionModelDescriptor::default()
            })
            .expect("model");
        let report = WorkflowStaticValidator.validate_for_publish(
            &workflow,
            &catalog(&[
                ("vision", vec![VisionCapability::VisionLanguage]),
                ("commit", Vec::new()),
            ]),
            &models,
            &ValidationCatalog::default(),
            &BTreeSet::new(),
            true,
        );
        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "model_capability_mismatch")
        );
    }

    #[test]
    fn unsupported_visual_prompt_is_blocked_before_publish() {
        let mut vision = node("vision", WorkflowNodeKind::VisionModel);
        vision.model_binding = Some("classifier".to_owned());
        vision.parameters.insert(
            "visual_prompt_box".to_owned(),
            serde_json::json!([0.1, 0.1, 0.2, 0.2]),
        );
        let workflow = draft(vec![vision], Vec::new());
        let mut models = ModelRegistry::new();
        models
            .register_backend(Arc::new(ClassificationBackend))
            .expect("backend");
        models
            .register_model(VisionModelDescriptor {
                id: "classifier".to_owned(),
                backend_id: "classification-backend".to_owned(),
                capabilities: vec![VisionCapability::Classification],
                input_contract: crate::ModelInputContract {
                    input_types: vec![crate::VisionInputType::Image],
                    supports_multiple_queries: false,
                    supports_visual_prompt: false,
                    max_queries: None,
                },
                ..VisionModelDescriptor::default()
            })
            .expect("model");
        let report = WorkflowStaticValidator.validate_for_publish(
            &workflow,
            &catalog(&[("vision", vec![VisionCapability::Classification])]),
            &models,
            &ValidationCatalog::default(),
            &BTreeSet::new(),
            true,
        );
        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "visual_prompt_unsupported")
        );
    }

    #[test]
    fn model_registry_rejects_plaintext_secret_material() {
        let mut models = ModelRegistry::new();
        models
            .register_backend(Arc::new(ClassificationBackend))
            .expect("backend");
        let error = models
            .register_model(VisionModelDescriptor {
                id: "unsafe-model".to_owned(),
                backend_id: "classification-backend".to_owned(),
                capabilities: vec![VisionCapability::Classification],
                secret_reference: Some("plaintext-secret".to_owned()),
                ..VisionModelDescriptor::default()
            })
            .expect_err("plaintext secret must be rejected");
        assert!(error.to_string().contains("never secret material"));

        let error = models
            .register_model(VisionModelDescriptor {
                id: "unsafe-configuration".to_owned(),
                backend_id: "classification-backend".to_owned(),
                capabilities: vec![VisionCapability::Classification],
                configuration: BTreeMap::from([(
                    "transport".to_owned(),
                    serde_json::json!({"api_key": "plaintext-secret"}),
                )]),
                ..VisionModelDescriptor::default()
            })
            .expect_err("secret configuration must be rejected");
        assert!(error.to_string().contains("secret_reference"));
    }

    #[test]
    fn snapshot_serialization_is_stable_and_frozen() {
        let workflow = draft(vec![node("commit", WorkflowNodeKind::Commit)], Vec::new());
        let snapshot = WorkflowSnapshot::frozen(
            &workflow,
            &ModelRegistry::new(),
            BTreeMap::from([("dummy".to_owned(), "1.0".to_owned())]),
        );
        assert_eq!(
            snapshot.stable_json().expect("first serialization"),
            snapshot.stable_json().expect("second serialization")
        );
        let hash_material = snapshot
            .content_hash_material()
            .expect("content hash material");
        let mut lifecycle_change = snapshot.clone();
        let lifecycle_draft = lifecycle_change.draft.as_mut().expect("draft");
        lifecycle_draft.status = WorkflowDraftStatus::Validated;
        lifecycle_draft.updated_at = Utc::now() + chrono::Duration::seconds(5);
        assert_eq!(
            hash_material,
            lifecycle_change
                .content_hash_material()
                .expect("stable lifecycle hash")
        );
        let mut edited = workflow;
        edited.name = "edited later".to_owned();
        assert_ne!(
            snapshot.draft.as_ref().expect("frozen draft").name,
            edited.name
        );
    }

    #[test]
    fn agent_dry_run_summary_reports_a_bounded_review_rate() {
        let summary = AgentDryRunSummary {
            auto_accepted_count: 1,
            review_count: 2,
            rejected_count: 1,
            ..AgentDryRunSummary::default()
        };
        assert!((summary.review_rate() - 0.5).abs() < f32::EPSILON);
        assert!(AgentDryRunSummary::default().review_rate().abs() < f32::EPSILON);
    }
}
