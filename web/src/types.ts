export type RunStatus =
  | "pending"
  | "running"
  | "paused"
  | "awaiting_review"
  | "completed"
  | "completed_with_review"
  | "partial"
  | "cancelled"
  | "budget_exceeded"
  | "failed"
  | "interrupted";

export type ProjectReadiness = "incomplete" | "ready" | "configuration_issue";
export type ProjectStage =
  | "needs_data"
  | "needs_labels"
  | "needs_automation"
  | "needs_model_binding"
  | "ready_for_sample_test"
  | "sample_test_needs_attention"
  | "ready_to_activate"
  | "ready_to_run"
  | "running"
  | "needs_review"
  | "ready_to_export"
  | "configuration_issue";

export interface GuidedAction {
  kind:
    | "add_images"
    | "define_labels"
    | "choose_automation"
    | "connect_model"
    | "fix_automation"
    | "test_samples"
    | "review_test_results"
    | "activate_automation"
    | "run_dataset"
    | "open_active_run"
    | "review_results"
    | "export_dataset"
    | "view_automation"
    | "view_runs";
  label: string;
  destination?: string;
  enabled: boolean;
  disabled_reason?: string;
}

export interface GuidanceBlocker {
  code: string;
  title: string;
  explanation: string;
  repair_action?: GuidedAction;
}

export interface ProjectGuidance {
  project_id: string;
  stage: ProjectStage;
  completed_steps: number;
  total_steps: number;
  headline: string;
  explanation: string;
  primary_action: GuidedAction;
  secondary_actions: GuidedAction[];
  blockers: GuidanceBlocker[];
  journey: {
    id: string;
    label: string;
    state: "complete" | "current" | "upcoming" | "needs_attention" | "ready";
    detail: string;
    destination?: string;
  }[];
  updated_at: string;
}
export type ReviewStatus =
  | "needs_review"
  | "auto_accepted"
  | "human_accepted"
  | "rejected"
  | "draft";

export interface HistoryRun {
  id: string;
  project_name: string;
  workflow_name: string;
  workflow_version: string;
  skill_versions: string[];
  model_bindings: ModelBinding[];
  provider: string;
  model: string;
  status: RunStatus;
  controllable: boolean;
  input_tokens: number;
  output_tokens: number;
  cost: string;
  current_node?: string;
  current_node_status?: string;
  artifact_count: number;
  validation_issue_codes: string[];
  retry_count: number;
  fallback_nodes: string[];
  model_identity: string;
  timed_out: boolean;
  checkpoint_present: boolean;
  review_suspended: boolean;
  terminal_reason?: string;
  created_at: string;
  updated_at: string;
}

export type WorkflowStatus =
  | "draft"
  | "invalid"
  | "valid"
  | "tested"
  | "published"
  | "archived";

export interface EnabledSkill {
  id: string;
  display_name: string;
  version: string;
}

export interface ModelBinding {
  id: string;
  provider: string;
  model: string;
  role: string;
  scope: string;
  health_status: "healthy" | "degraded" | "unavailable" | "unknown";
  health_detail?: string;
  availability_group: "ready" | "configured_unavailable" | "labs" | "disabled";
  capabilities?: string[];
  score_semantics?: string;
  model_version?: string;
  endpoint?: string;
  enabled?: boolean;
  license_summary?: string;
  architecture?: string;
  checkpoint_sha256?: string;
  label_space?: string[];
  cost_per_request?: string;
}

export interface DetectionWorkerTestResult {
  model_id: string;
  health: {
    status: "healthy" | "degraded" | "unavailable" | "unknown";
    detail?: string;
  };
  capabilities: {
    capabilities: string[];
    score_semantics: string;
    supports_visual_prompt: boolean;
    supports_batch: boolean;
    label_space: string[];
  };
}

export type ProviderAdapterKind = "open_ai_compatible" | "mock";
export type CredentialSource =
  | "system_keyring"
  | "environment_variable"
  | "workspace_file"
  | "session_only"
  | "legacy_workspace_file";
export type ProviderHealthStatus =
  | "unknown"
  | "configured"
  | "available"
  | "unreachable"
  | "invalid_credential"
  | "rate_limited"
  | "incompatible_protocol"
  | "disabled";

export interface ProviderPresetProfile {
  id: string;
  display_name: string;
  adapter: ProviderAdapterKind;
  base_url: string;
  description: string;
  suggested_models: string[];
}

export interface ProviderProfile {
  id: string;
  display_name: string;
  preset_id?: string;
  adapter: ProviderAdapterKind;
  base_url: string;
  endpoint_summary: string;
  organization?: string;
  workspace?: string;
  safe_headers: Record<string, string>;
  connection_policy: {
    request_timeout_seconds: number;
    maximum_retries: number;
    maximum_concurrency: number;
    minimum_retry_delay_ms: number;
    maximum_retry_delay_ms: number;
    allow_remote_http: boolean;
    allowed_redirects: number;
  };
  enabled: boolean;
  health: {
    status: ProviderHealthStatus;
    safe_message?: string;
    checked_at?: string;
  };
  credential_configured: boolean;
  credential_source?: CredentialSource;
  model_count: number;
  created_at: string;
  updated_at: string;
}

export interface LegacyRegistryImportPreview {
  fingerprint: string;
  provider_id: string;
  provider_display_name: string;
  provider_adapter: ProviderAdapterKind;
  endpoint_summary: string;
  model_profile_id: string;
  model_display_name: string;
  remote_model_id: string;
  capability_source: "user_declared" | "provider_discovered" | "preset" | "unknown";
  credential_source?: CredentialSource;
  project_binding_count: number;
  already_applied: boolean;
  moves_secret: false;
  modifies_historical_runs: false;
}

export interface LegacyRegistryImportReport {
  fingerprint: string;
  provider_id: string;
  model_profile_id: string;
  provider_created: boolean;
  model_created: boolean;
  bindings_created: number;
  bindings_preserved: number;
  already_applied: boolean;
  credential_source?: string;
  historical_runs_modified: 0;
}

export type InputModality = "text" | "image" | "video";
export type ModelCapability =
  | "text_generation"
  | "vision_language"
  | "image_classification"
  | "object_detection"
  | "open_vocabulary_detection"
  | "phrase_grounding"
  | "semantic_segmentation"
  | "prompted_segmentation"
  | "instance_segmentation"
  | "keypoint_detection";

export interface RegistryModelProfile {
  id: string;
  revision: number;
  provider_id: string;
  display_name: string;
  remote_model_id: string;
  input_modalities: InputModality[];
  protocol_features: {
    tool_calls: boolean;
    parallel_tool_calls: boolean;
    structured_output: boolean;
    json_schema: boolean;
    usage_reporting: boolean;
    streaming: boolean;
    reasoning_controls: boolean;
  };
  task_capabilities: ModelCapability[];
  capability_source: "user_declared" | "provider_discovered" | "preset" | "unknown";
  limits: {
    context_tokens?: number;
    maximum_output_tokens?: number;
    maximum_images_per_request?: number;
    maximum_image_pixels?: number;
  };
  generation_defaults: Record<string, string | number | undefined>;
  pricing: {
    currency: string;
    input_per_million_tokens?: string;
    output_per_million_tokens?: string;
    cached_input_per_million_tokens?: string;
    per_image?: string;
    per_request?: string;
    source: "user_configured" | "provider_discovered" | "preset" | "unknown";
    updated_at?: string;
  };
  status: "unknown" | "unverified" | "available" | "unavailable" | "disabled";
  enabled: boolean;
  locked: boolean;
  created_at: string;
  updated_at: string;
}

export type ModelBindingRole =
  | "pipeline_builder"
  | "primary_inference"
  | "detection"
  | "classification"
  | "segmentation"
  | "verification"
  | "fallback";

export interface ProjectModelBinding {
  id: string;
  project_id: string;
  capability: ModelCapability;
  role: ModelBindingRole;
  match_kind: "capability" | "role";
  model_profile_id: string;
  locked: boolean;
  created_at: string;
}

export interface GlobalModelDefaults {
  pipeline_builder?: string;
  vision_language?: string;
  text_generation?: string;
}

export interface ProviderProbeUsage {
  id: string;
  provider_id: string;
  model_profile_id: string;
  model_profile_revision: number;
  request_id?: string;
  input_tokens?: number;
  output_tokens?: number;
  total_tokens?: number;
  cost: string;
  currency: string;
  duration_ms: number;
  succeeded: boolean;
  safe_message: string;
  created_at: string;
}

export interface WorkflowNodeSummary {
  id: string;
  node_type: string;
  depends_on: string[];
  model_binding?: string;
  validators: string[];
  refiners: string[];
  human_review_gate: boolean;
  fallback?: string;
}

export interface WorkflowVersion {
  workflow_id: string;
  name: string;
  version: string;
  status: WorkflowStatus;
  validation_status: string;
  is_default: boolean;
  source: string;
  nodes: WorkflowNodeSummary[];
}

export interface WorkflowSummary {
  id: string;
  name: string;
  current_version: string;
  status: WorkflowStatus;
  validation_status: string;
  is_default: boolean;
  node_count: number;
}

export interface ProjectSummary {
  id: string;
  name: string;
  description?: string;
  dataset: {
    root: string;
    include: string[];
    recursive: boolean;
    image_count: number;
  };
  annotation_schema: {
    id: string;
    display_name: string;
    kind: string;
    labels: string[];
    required: boolean;
  }[];
  enabled_skills: EnabledSkill[];
  workflows: WorkflowSummary[];
  active_workflow: WorkflowVersion;
  available_workflow_versions: WorkflowVersion[];
  model_bindings: ModelBinding[];
  export_formats: string[];
  /** Compatibility field from ProjectSchema v1. */
  skill_id: string;
  image_count: number;
  task_count: number;
  review_count: number;
  readiness: ProjectReadiness;
  blocking_issues: {
    code: string;
    message: string;
    next_step: "data" | "labels" | "pipeline" | "test";
  }[];
  annotation_visuals?: Record<
    string,
    { slot: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8; pattern?: string }
  >;
  default_workflow_version?: WorkflowVersion;
  active_batch?: {
    id: string;
    status: RunStatus;
    event_sequence: number;
    max_concurrency: number;
    budget_ledger: {
      consumed: {
        image_count: number;
        request_count: number;
        total_tokens: number;
        cost: string;
      };
      reserved: {
        image_count: number;
        request_count: number;
        total_tokens: number;
        cost: string;
      };
    };
  };
  active_batch_progress?: {
    total_images: number;
    pending_images: number;
    running_images: number;
    completed_images: number;
    failed_images: number;
    review_images: number;
    cancelled_images: number;
  };
  active_run?: {
    id: string;
    provider: string;
    model: string;
    status: RunStatus;
    created_at: string;
    updated_at: string;
  };
  last_run?: {
    id: string;
    provider: string;
    model: string;
    status: RunStatus;
    terminal_reason?: string;
    created_at: string;
    updated_at: string;
  };
}

export interface ProjectWorkspaceSummary {
  project: ProjectSummary;
  guidance: ProjectGuidance;
  readiness: {
    project_id: string;
    readiness: ProjectReadiness;
    stage: ProjectStage;
    blockers: GuidanceBlocker[];
    updated_at: string;
  };
}

export interface DashboardData {
  projects: ProjectSummary[];
  runs: HistoryRun[];
  models: ModelBinding[];
  installed_skills: EnabledSkill[];
  review_queue: number;
}

export interface ProjectWorkflow {
  project_id: string;
  project_name: string;
  workflow: WorkflowVersion;
}

export interface WorkflowDraftNode {
  id: string;
  node_type: string;
  kind?:
    | "image_input"
    | "transform"
    | "vision_model"
    | "vision_language_model"
    | "deterministic_tool"
    | "candidate_merge"
    | "validator"
    | "refiner"
    | "gate"
    | "human_review"
    | "commit"
    | "export";
  depends_on: string[];
  inputs?: WorkflowNodePort[];
  outputs?: WorkflowNodePort[];
  model_binding?: string;
  required_skills?: string[];
  validators: string[];
  refiners: string[];
  fallback?: string;
  max_retries: number;
  review_gate: boolean;
  parameters: Record<string, unknown>;
  retry_policy?: { max_attempts: number };
  fallback_policy?: {
    target_node?: string;
    on_timeout: boolean;
    on_error: boolean;
  };
  gate?: { required: boolean; allow_manual_override: boolean };
  resources?: {
    timeout_seconds?: number;
    max_memory_mb?: number;
    accelerator?: string;
  };
}

export interface WorkflowNodePort {
  id: string;
  artifact_type:
    | "classification"
    | "bounding_box"
    | "keypoints"
    | "polyline"
    | "polygon"
    | "semantic_mask"
    | "instance_mask"
    | "attributes"
    | "relations"
    | "image"
    | "detection_set"
    | "box_prompt_set"
    | "point_prompt_set"
    | "mask_set"
    | "polygon_set"
    | "candidate_cluster_set"
    | "crop_set"
    | "classification_set"
    | "annotation_candidate_set";
  required: boolean;
  multiple: boolean;
}

export interface WorkflowEdge {
  from_node: string;
  from_port: string;
  to_node: string;
  to_port: string;
  route?: string;
}

export interface WorkflowDraft {
  schema_version?: number;
  id: string;
  project_id: string;
  name: string;
  status: "suggested" | "editing" | "validated" | "published" | "archived";
  nodes: WorkflowDraftNode[];
  edges?: WorkflowEdge[];
  enabled_skills?: Record<string, string>;
  resource_versions?: Record<string, string>;
  runtime_policies?: Record<string, unknown>;
  allow_unvalidated_commit?: boolean;
  label_pipeline?: LabelWorkflowComposition;
  created_at: string;
  updated_at: string;
}

export interface WorkflowSuggestion {
  draft: WorkflowDraft;
  rationale: string[];
  estimated_model_calls_per_image: number;
  estimated_latency_ms?: number;
  estimated_cost_tier: "low" | "medium" | "high" | string;
  unresolved_model_bindings: string[];
  warnings: string[];
  alternatives: string[];
  agent_session?: AgentSession;
  agent_validation?: WorkflowValidationReport;
  agent_dry_run?: WorkflowDryRunReport;
  approval_required?: boolean;
}

export type OptimizationPriority = "fast" | "balanced" | "accurate" | "low_cost";

export interface PipelineBuilderConstraints {
  priority: OptimizationPriority;
  max_cost_per_image?: string;
  max_model_calls_per_image?: number;
  max_expected_latency_ms?: number;
  target_review_rate?: number;
  allow_external_models: boolean;
  allow_human_review: boolean;
  maximum_agent_turns: number;
  maximum_tool_calls: number;
  maximum_dry_runs: number;
  maximum_agent_cost: string;
}

export interface PipelineDraftDiff {
  added_nodes: { change_id: string; node_id: string; node_type: string }[];
  removed_nodes: { change_id: string; node_id: string; node_type: string }[];
  modified_nodes: {
    change_id: string;
    node_id: string;
    before: Record<string, unknown>;
    after: Record<string, unknown>;
  }[];
  added_edges: { change_id: string; edge: WorkflowEdge }[];
  removed_edges: { change_id: string; edge: WorkflowEdge }[];
  model_binding_changes: {
    change_id: string;
    node_id: string;
    before?: string;
    after?: string;
  }[];
  policy_changes: {
    change_id: string;
    node_id: string;
    before: unknown;
    after: unknown;
  }[];
}

export interface WorkflowDraftApplyReport {
  draft: WorkflowDraft;
  previous_draft: WorkflowDraft;
  diff: PipelineDraftDiff;
  selected_change_ids: string[];
}

export interface AgentSession {
  id: string;
  project_id?: string;
  run_id?: string;
  kind: "pipeline_builder" | "workflow_advisor" | "annotation_recovery";
  status:
    | "running"
    | "waiting_for_human"
    | "succeeded"
    | "failed"
    | "budget_exceeded"
    | "cancelled";
  budget: {
    max_steps: number;
    max_tool_calls: number;
    max_tokens?: number;
    max_cost?: string;
  };
  builder_constraints?: PipelineBuilderConstraints;
  model_selection?: {
    provider_profile_id: string;
    provider_display_name: string;
    provider_adapter: ProviderAdapterKind;
    endpoint_summary: string;
    model_profile_id: string;
    model_profile_revision: number;
    model_display_name: string;
    remote_model_id: string;
    binding_source:
      | "workflow_node"
      | "project_capability"
      | "project_role"
      | "global_default";
    locked: boolean;
  };
  model_calls: {
    sequence: number;
    provider_profile_id?: string;
    model_profile_id?: string;
    model_profile_revision?: number;
    provider_name: string;
    remote_model_id: string;
    request_id?: string;
    input_tokens: number;
    output_tokens: number;
    usage_source: string;
    duration_ms: number;
    cost: string;
    currency: string;
    retry_count: number;
    succeeded: boolean;
    safe_error?: string;
    created_at: string;
  }[];
  usage: {
    steps: number;
    tool_calls: number;
    input_tokens: number;
    output_tokens: number;
    cost: string;
  };
  steps: {
    sequence: number;
    call_id: string;
    tool_name: string;
    arguments: Record<string, unknown>;
    result: unknown;
    success: boolean;
    started_at: string;
    finished_at: string;
  }[];
  stop_reason?: string;
  pending_human_action?: string;
  created_at: string;
  updated_at: string;
}

export interface CorrectionMemoryRecord {
  id: string;
  project_id: string;
  skill_id: string;
  task_id: string;
  predicted_label?: string;
  corrected_label?: string;
  reason_code: string;
  note?: string;
  created_at: string;
}

export type PipelineArtifactType =
  | "image"
  | "detection_set"
  | "box_prompt_set"
  | "point_prompt_set"
  | "mask_set"
  | "polygon_set"
  | "candidate_cluster_set"
  | "crop_set"
  | "classification_set"
  | "annotation_candidate_set";

export type PipelineSource =
  | { source: "image" }
  | {
      source: "shared_stage";
      stage_id: string;
      step_id: string;
      port: string;
      artifact_type: PipelineArtifactType;
    }
  | {
      source: "step";
      step_id: string;
      port: string;
      artifact_type: PipelineArtifactType;
    };

export interface PipelineStep {
  id: string;
  node_type: string;
  kind: NonNullable<WorkflowDraftNode["kind"]>;
  inputs: Record<string, PipelineSource>;
  outputs: Record<string, PipelineArtifactType>;
  model_binding?: {
    model_id: string;
    capability: string;
    configuration: Record<string, unknown>;
  };
  skill_binding?: { skill_id: string; version: string; operation: string };
  parameters: Record<string, unknown>;
  validators: string[];
  refiners: string[];
  fallback?: string;
  retry_policy: { max_attempts: number };
  review_gate: { required: boolean; allow_manual_override: boolean };
  resources: {
    timeout_seconds?: number;
    max_memory_mb?: number;
    accelerator?: string;
  };
}

export interface LabelWorkflowComposition {
  schema_version: number;
  shared_stages: { id: string; name: string; steps: PipelineStep[] }[];
  label_pipelines: {
    id: string;
    target_task_id: string;
    target_label: string;
    steps: PipelineStep[];
  }[];
}

export interface WorkflowValidationReport {
  valid: boolean;
  issues: { code: string; path: string; message: string; blocking: boolean }[];
  execution_order: string[];
}

export interface WorkflowDryRunReport {
  sandbox: boolean;
  validation: WorkflowValidationReport;
  samples: {
    image_index: number;
    image_name: string;
    width: number;
    height: number;
    result_count: number;
    auto_accepted_count: number;
    review_count: number;
    failed: boolean;
    empty: boolean;
    outcomes: {
      id: string;
      label: string;
      confidence?: number | null;
      status: "ready_to_accept" | "needs_review" | "invalid";
      value?: AnnotationValue | null;
    }[];
    nodes: {
      node_id: string;
      status: string;
      output_types: string[];
      latency_ms: number;
      estimated_cost: string;
      issues: WorkflowValidationReport["issues"];
    }[];
  }[];
  summary: {
    image_count: number;
    detection_count: number;
    candidate_count: number;
    auto_accepted_count: number;
    needs_review_count: number;
    failed_count: number;
    empty_count: number;
    fallback_count: number;
    cache_hit_count: number;
    duration_ms: number;
    input_tokens: number;
    output_tokens: number;
    usage: {
      input_tokens: number;
      output_tokens: number;
      estimated_cost: string;
    };
    estimated_full_run?: {
      image_count: number;
      duration_ms: number;
      estimated_cost: string;
      review_count_min: number;
      review_count_max: number;
    } | null;
  };
  total_latency_ms: number;
  estimated_cost: string;
}

export interface WorkflowVersionComparison {
  left_workflow_id: string;
  left_version: number;
  right_workflow_id: string;
  right_version: number;
  added_nodes: string[];
  removed_nodes: string[];
  changed_nodes: string[];
  same_content: boolean;
}

export interface WorkflowCatalog {
  project_id: string;
  project_schema: unknown;
  target_task_id?: string;
  target_label?: string;
  enabled_skills: string[];
  node_catalog: {
    id: string;
    display_name: string;
    category:
      | "input"
      | "image_preparation"
      | "model_inference"
      | "result_transform"
      | "evidence_and_validation"
      | "human_and_output";
    input_ports: {
      name: string;
      artifact_type: string;
      required: boolean;
      cardinality: "one" | "many";
    }[];
    output_ports: {
      name: string;
      artifact_type: string;
      required: boolean;
      cardinality: "one" | "many";
    }[];
    config_schema: Record<string, unknown>;
    required_model_capability?: ModelCapability;
    cardinality: "one_to_one" | "one_to_many" | "many_to_one" | "many_to_many";
    side_effect: "none" | "human_suspension" | "annotation_commit";
    dry_run_supported: boolean;
    expert_only: boolean;
  }[];
  runtime_policies: {
    id: string;
    display_name: string;
    scope: "node" | "workflow" | "runtime";
    config_schema: Record<string, unknown>;
  }[];
  model_registry: {
    id: string;
    display_name: string;
    capabilities: string[];
  }[];
  validator_ids: string[];
  refiner_ids: string[];
  resource_ids: string[];
  workflow_templates: {
    id: string;
    name: string;
    description: string;
    nodes: WorkflowDraftNode[];
    edges: NonNullable<WorkflowDraft["edges"]>;
  }[];
  constraints: Record<string, unknown>;
  data_profile: {
    image_count: number;
    sample_width?: number;
    sample_height?: number;
    mime_types: string[];
  };
}

export interface PipelineArtifact {
  kind: PipelineArtifactType;
  artifact: Record<string, unknown>;
}

export type DetectionScoreSemantics =
  | "calibrated_probability"
  | "relative_confidence"
  | "ranking_score"
  | "not_provided"
  | "unknown";

export interface DetectionScoreDto {
  value?: number | null;
  semantics: DetectionScoreSemantics;
}

export interface StoredPayloadRefDto {
  id: string;
  media_type: string;
  sha256: string;
  size_bytes: number;
}

export interface DetectionEvidenceDto {
  source_model_id: string;
  source_artifact_id: string;
  bbox: [number, number, number, number];
  score: DetectionScoreDto;
  query_id?: string | null;
  model_label?: string | null;
  project_label?: string | null;
  source_capability: string;
  raw_output_ref?: StoredPayloadRefDto | null;
}

export interface DetectionArtifactItemDto {
  detection_id: string;
  query_id?: string | null;
  model_label?: string | null;
  project_label?: string | null;
  bbox: [number, number, number, number];
  score: DetectionScoreDto;
  source_model_id: string;
  source_capability: string;
  evidence: DetectionEvidenceDto[];
  attributes: Record<string, unknown>;
}

export interface CandidateClusterDto {
  id: string;
  target_label: string;
  representative_bbox: [number, number, number, number];
  members: DetectionEvidenceDto[];
  agreement:
    | "single_source"
    | "geometry_conflict"
    | "label_conflict"
    | { multi_source_agreement: { minimum_iou: number; mean_iou: number } };
}

export interface EvidenceGateReasonDto {
  code: string;
  message: string;
  candidate_id?: string | null;
  source_model_ids: string[];
  metrics: Record<string, number>;
}

export interface EvidenceGateReportDto {
  decision: "accept" | "fallback" | "review" | "reject";
  reasons: EvidenceGateReasonDto[];
  candidate_count: number;
  validation_issue_count: number;
}

export interface RunNodeArtifactInspection {
  run_id: string;
  workflow_id: string;
  workflow_version: number;
  content_hash: string;
  project_id: string;
  image_index?: number;
  nodes: {
    node_id: string;
    operation: string;
    status: string;
    configuration: WorkflowDraftNode;
    inputs: PipelineArtifact[];
    outputs: PipelineArtifact[];
    latency_ms: number;
    attempts: number;
    cache_hit: boolean;
    usage: { input_tokens: number; output_tokens: number; cost: string };
    route?: string | null;
    metadata?: Record<string, unknown>;
    error?: { code: string; summary: string; retryable: boolean };
  }[];
}

export interface NodeReplayReport {
  source_run_id: string;
  replayed_from: string;
  reexecuted_nodes: string[];
  preserved_upstream_nodes: string[];
  inspection: RunNodeArtifactInspection;
  sandbox: boolean;
}

export interface ImageItem {
  index: number;
  name: string;
  path: string;
  size_bytes: number;
  url: string;
}

export interface RunAnnotationInspection {
  run_id: string;
  project_id: string;
  image_index?: number;
  annotations: Annotation[];
}

export interface RunResultSummary {
  run_id: string;
  project_id: string;
  status: RunStatus;
  image_count: number;
  result_count: number;
  ready_count: number;
  needs_review_count: number;
  no_target_count: number;
  failed_count: number;
  fallback_count: number;
  cache_hit_count: number;
  duration_ms: number;
  usage: { input_tokens: number; output_tokens: number; estimated_cost: string };
  image_index?: number;
  labels: { label: string; count: number }[];
}

export interface RunDebugSummary {
  run_id: string;
  workflow_id?: string;
  workflow_version?: number;
  node_count: number;
  succeeded_node_count: number;
  failed_node_count: number;
  current_node?: string;
  issues: { node_id: string; code: string; summary: string; retryable: boolean }[];
  duration_ms: number;
  usage: { input_tokens: number; output_tokens: number; estimated_cost: string };
}

export interface ExportBlocker {
  code: string;
  title: string;
  explanation: string;
  repair_destination: string;
}

export interface ExportFormatCompatibility {
  format: string;
  display_name: string;
  supported: boolean;
  recommended: boolean;
  summary: string;
  warnings: string[];
  unsupported_task_kinds: string[];
}

export interface ExportReport {
  exported_count: number;
  skipped_count: number;
  warnings: string[];
  unsupported_task_kinds: string[];
  output_files: string[];
}

export interface ProjectExportResult {
  format: string;
  output_path: string;
  completed_at: string;
  source_fingerprint: string;
  report: ExportReport;
}

export interface ExportReadiness {
  project_id: string;
  ready: boolean;
  image_count: number;
  processed_image_count: number;
  accepted_annotations: number;
  unresolved_reviews: number;
  blocking_issues: ExportBlocker[];
  recommended_format?: string;
  formats: ExportFormatCompatibility[];
  output_root: string;
  last_export?: ProjectExportResult;
}

export type Point = [number, number];

export type AnnotationValue =
  | { kind: "classification"; labels: string[] }
  | { kind: "bounding_box"; rect: [number, number, number, number] }
  | {
      kind: "keypoints";
      points: { name: string; point: Point; visible: boolean }[];
    }
  | { kind: "polyline"; points: Point[] }
  | { kind: "polygon"; rings: Point[][] }
  | {
      kind: "semantic_mask" | "instance_mask";
      mask:
        | { encoding: "polygon"; rings: Point[][] }
        | { encoding: "coco_rle"; width: number; height: number; counts: string };
    }
  | { kind: "attributes"; values: Record<string, unknown> }
  | { kind: "relations"; relations: unknown[] };

export interface Annotation {
  id: string;
  image_id: string;
  task_id: string;
  label?: string;
  value: AnnotationValue;
  attributes: Record<string, unknown>;
  confidence?: number;
  source: string;
  review_status: ReviewStatus;
  provenance: Record<string, unknown>;
  created_at: string;
}

export interface ReviewItem {
  id: string;
  run_id: string;
  project_id?: string;
  project_name: string;
  annotation: Annotation;
  workflow_id?: string;
  workflow_version: number;
  image_index?: number;
  source_node?: string;
  source_skill_id?: string;
  source_artifact_id?: string;
  refinement_chain: string[];
  review_reason: string;
  confidence?: number;
  validation_issues: string[];
  detection_evidence: DetectionEvidenceDto[];
  candidate_agreement?: CandidateClusterDto["agreement"];
  evidence_decision?: EvidenceGateReportDto | Record<string, unknown>;
  review_explanation?: {
    code: string;
    title: string;
    summary: string;
    details: string[];
  };
}

export interface ReviewQueueProgress {
  reviewed_count: number;
  total_count: number;
  remaining_count: number;
  current_position?: number;
}

export interface ReviewNavigation {
  previous_review?: ReviewItem;
  next_review?: ReviewItem;
  progress: ReviewQueueProgress;
}

export interface ReviewDecisionOutcome {
  annotation: Annotation;
  next_review?: ReviewItem;
  progress: ReviewQueueProgress;
}

export interface RunEvent {
  event_id: string;
  run_id: string;
  image_id?: string;
  task_id?: string;
  occurred_at: string;
  kind: string;
  payload: { type: string; data: Record<string, unknown> };
}

export interface SkillDetail {
  id: string;
  display_name: string;
  version: string;
  kind: "capability" | "domain" | "pack";
  description: string;
  nodes: string[];
  tools: string[];
  validators: string[];
  refiners: string[];
  policies: string[];
  capabilities: string[];
  capability_requirements: string[];
  correction_taxonomy: string[];
  resources: string[];
  workflow_templates: {
    id: string;
    name: string;
    description: string;
    node_count: number;
  }[];
  projects: string[];
  project_template?: string;
}
