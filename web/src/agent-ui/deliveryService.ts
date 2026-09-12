import type { Annotation } from "../types";
import type { DeliveryFormalResult, FormalReviewWorkItem } from "./deliveryVisualSelection";

/** Formal annotations only. Sample overlays are never whole-image review receipts. */
export type DeliveryReviewInput = {
  command_id: string; intent_revision: number; intent_sha256: string;
  image_id: string; source_run_id: string | null;
  expected_snapshot_sha256: string; expected_review_revision: number;
  decision: "positive_complete" | "negative_confirmed" | "excluded";
  reason: string | null; confirmed: boolean;
};
export type DeliveryObjectEdit = {
  command_id:string;intent_revision:number;intent_sha256:string;source_run_id:string;
  annotation_id:string;expected_snapshot_sha256:string;label:string;value:Annotation["value"];
  review_status:"needs_review"|"human_accepted"|"rejected";reason:string;
};
export type DeliveryObjectCreate = Omit<DeliveryObjectEdit,"annotation_id"|"review_status">;
export type DeliveryImageSnapshot = {
  image_id: string; content_sha256: string; source_run_id: string | null;
  annotations: Annotation[]; sha256: string;
};
export type DeliveryReview = {
  revision: number; input: DeliveryReviewInput;
  snapshot: DeliveryImageSnapshot; created_at: string;
};
export type DeliveryImageView = {
  sources: {run_id: string; model: string; status: string; created_at: string}[];
  intent_revision: number; intent_sha256: string; snapshot: DeliveryImageSnapshot;
  review: DeliveryReview | null; confirmation_current: boolean;
  accepted_objects: number; unresolved_objects: number; notice: string;
};
export type DemoAnnotationOrigin = {
  kind:"preset_candidate"|"live_model_prediction"|"human_revision";
  source_id:string;
  source_artifact_id:string|null;
  model_display_name:string|null;
  actor_display_name:string|null;
  created_at:string|null;
};
export type DemoImageReviewState = "pending"|"review_required"|"positive_complete"|"negative_confirmed"|"excluded"|"failed";
export type DemoReviewPanelRead = {
  contract_version:"demo-review-v1";
  project_id:string;
  task_id:string;
  review_id:string|null;
  read_model_revision:string;
  demo:{id:string;version:string;source_mode:"preset_candidates"|"live_model";live_inference_occurred:boolean};
  images:{
    image_id:string;
    name:string;
    url:string;
    thumbnail_url:string|null;
    state:DemoImageReviewState;
    source_artifact_id:string|null;
    annotation_origins:Record<string,DemoAnnotationOrigin>;
  }[];
  labels:{stable_id:string;display_name:string}[];
  sample_result:import("./deliveryVisualSelection").DeliverySampleResult|null;
  formal_result:import("./deliveryVisualSelection").DeliveryFormalResult|null;
};
export type DeliveryPackageInput = {
  command_id: string; intent_revision: number; intent_sha256: string;
  image_reviews: Record<string, number>; confirmed: boolean;
};
export type DeliveryPackageStatus = {
  id: string; phase: "preparing" | "exporting" | "validating" | "ready" | "failed" | "cancelled";
  intent_revision: number; snapshot_sha256: string;
  result: { sha256: string; bytes: number; images: number; objects: number; negatives: number; excluded: number; summary?:{
    labels:string[];
    splits:Partial<Record<"train"|"val"|"test",number>>;
    warnings:string[];
    exclusions:Record<string,string>;
    demo?:{id:string;version:string;data_sha256:string}|null;
    source_mode?:"preset_candidates"|"live_model"|null;
    live_inference_occurred?:boolean|null;
    source_counts?:Partial<Record<DemoAnnotationOrigin["kind"],number>>|null;
    review_sources?:string[]|null;
  }|null } | null;
  error: string | null;
};
export type DeliveryPackageRead = { job: DeliveryPackageStatus; active: boolean; interrupted: boolean };
export type DeliveryPackageStart = { job: DeliveryPackageStatus; active: boolean; dispatched: boolean };
export type DeliveryReviewSummary = {
  intent_revision:number;intent_sha256:string;formal_result:DeliveryFormalResult|null;
  counts:{total:number;complete:number;positive:number;negative:number;excluded:number;unresolved:number;failed:number};
  items:{image_id:string;image_sha256:string;state:"positive_complete"|"negative_confirmed"|"excluded"|"unresolved"|"failed";review_revision:number|null;child_run_id:string|null;error:string|null;formal_selections:Record<string,import("./mainline").FormalVisualSelection>}[];
  next_cursor:string|null;
};
export type PackageConsent = {input:{id:string;intent_revision:number;intent_sha256:string;confirmed:true};state:"armed"|"consumed"|"cancelled"};
export type PackageReady = {
  intent_revision:number;intent_sha256:string;ready:boolean;
  counts:DeliveryReviewSummary["counts"];review_revisions:Record<string,number>;
  blockers:{code:string;message:string;image_ids:string[]}[];
  consent:PackageConsent|null;package:DeliveryPackageRead|null;
};
export type DeliveryPackageConsent = PackageConsent;
export type DeliveryPackageReadiness = PackageReady;
export type FormalReviewWorkPage = {
  formal_result:DeliveryFormalResult;
  items:FormalReviewWorkItem[];
  next_cursor:string|null;
};
export type DemoDeliveryPanelRead = {
  contract_version:"demo-delivery-v1";
  project_id:string;
  task_id:string;
  delivery_id:string|null;
  read_model_revision:string;
  scope:{revision:number;content_sha256:string;image_ids:string[]};
  demo:{id:string;version:string;source_mode:"preset_candidates"|"live_model";live_inference_occurred:boolean};
};

/** Explicit commands retain caller-owned idempotency keys; reads never start jobs. */
export interface DeliveryService {
  createObject?(project:string,task:string,image:string,input:DeliveryObjectCreate):Promise<unknown>;
  editObject(project:string,task:string,image:string,input:DeliveryObjectEdit):Promise<unknown>;
  pendingPackage(project:string,task:string):DeliveryPackageInput|undefined;
  history(project: string, task: string, before?:string, signal?:AbortSignal): Promise<{items:{id:string;created_at:string}[];next_cursor:string|null}>;
  image(project: string, task: string, image: string, run: string | null, signal?: AbortSignal): Promise<DeliveryImageView>;
  confirmImage(project: string, task: string, input: DeliveryReviewInput): Promise<DeliveryReview>;
  startPackage(project: string, task: string, input: DeliveryPackageInput): Promise<DeliveryPackageStart>;
  packageStatus(project: string, task: string, id: string, signal?: AbortSignal): Promise<DeliveryPackageRead>;
  cancelPackage(project: string, task: string, id: string): Promise<DeliveryPackageStatus>;
  downloadUrl(project: string, task: string, id: string): string;
  /** Server-derived task lineage. Never substitute project-latest Runs. */
  formalResult?(project:string,task:string,signal?:AbortSignal):Promise<DeliveryFormalResult|null>;
  reviewWorkItems?(project:string,task:string,cursor?:string,signal?:AbortSignal):Promise<FormalReviewWorkPage>;
  reviewSummary?(project:string,task:string,cursor?:string,signal?:AbortSignal):Promise<DeliveryReviewSummary>;
  packageReadiness?(project:string,task:string,signal?:AbortSignal):Promise<DeliveryPackageReadiness>;
  authorizePackage?(project:string,task:string,input:DeliveryPackageConsent["input"]):Promise<DeliveryPackageConsent>;
  cancelPackageAuthorization?(project:string,task:string,id:string):Promise<DeliveryPackageConsent>;
}

export interface DemoReviewPanelService extends DeliveryService {
  demoReviewPanel(project:string,task:string,reviewId?:string,signal?:AbortSignal):Promise<DemoReviewPanelRead>;
}

export interface DemoDeliveryPanelService extends DeliveryService {
  demoDeliveryPanel(project:string,task:string,deliveryId?:string,signal?:AbortSignal):Promise<DemoDeliveryPanelRead>;
}
