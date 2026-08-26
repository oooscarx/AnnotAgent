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
  terminal_reason?: string;
  created_at: string;
  updated_at: string;
}

export type WorkflowStatus = "draft" | "valid" | "published" | "archived";

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
    | "relations";
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
  allow_unvalidated_commit?: boolean;
  created_at: string;
  updated_at: string;
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
    image_name: string;
    width: number;
    height: number;
    nodes: {
      node_id: string;
      status: string;
      output_types: string[];
      latency_ms: number;
      estimated_cost: string;
      issues: WorkflowValidationReport["issues"];
    }[];
  }[];
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
  enabled_skills: string[];
  node_catalog: { id: string; display_name: string; produces: string[] }[];
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

export interface ImageItem {
  index: number;
  name: string;
  url: string;
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
  | { kind: "instance_mask"; mask: { kind: "polygon"; rings: Point[][] } };

export interface Annotation {
  id: string;
  image_id: string;
  task_id: string;
  label?: string;
  value: AnnotationValue;
  attributes: Record<string, unknown>;
  confidence?: number;
  source: string;
  review_status: string;
  provenance: Record<string, unknown>;
  created_at: string;
}

export interface ReviewItem {
  id: string;
  run_id: string;
  annotation: Annotation;
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
  description: string;
  tasks: { id: string; description: string }[];
  tools: string[];
  validators: string[];
  refiners: string[];
  correction_taxonomy: string[];
  resources: string[];
  workflow_templates: {
    id: string;
    name: string;
    description: string;
    node_count: number;
  }[];
  project_template?: string;
}
