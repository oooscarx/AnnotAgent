export type RunStatus =
  | "pending"
  | "running"
  | "paused"
  | "awaiting_review"
  | "completed"
  | "cancelled"
  | "budget_exceeded"
  | "failed";

export interface HistoryRun {
  id: string;
  project_name: string;
  skill_id: string;
  provider: string;
  model: string;
  status: RunStatus;
  terminal_reason?: string;
  created_at: string;
}

export interface ProjectSummary {
  id: string;
  name: string;
  skill_id: string;
  image_count: number;
  recent_run?: HistoryRun;
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
  | { kind: "keypoints"; points: { name: string; point: Point; visible: boolean }[] }
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
  project_template?: string;
}
