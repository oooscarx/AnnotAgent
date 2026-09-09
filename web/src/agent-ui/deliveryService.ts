import type { Annotation } from "../types";

/** Formal annotations only. Sample overlays are never whole-image review receipts. */
export type DeliveryReviewInput = {
  command_id: string; intent_revision: number; intent_sha256: string;
  image_id: string; source_run_id: string | null;
  expected_snapshot_sha256: string; expected_review_revision: number;
  decision: "positive_complete" | "negative_confirmed" | "excluded";
  reason: string | null; confirmed: boolean;
};
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
export type DeliveryPackageInput = {
  command_id: string; intent_revision: number; intent_sha256: string;
  image_reviews: Record<string, number>; confirmed: boolean;
};
export type DeliveryPackageStatus = {
  id: string; phase: "preparing" | "exporting" | "validating" | "ready" | "failed" | "cancelled";
  intent_revision: number; snapshot_sha256: string;
  result: { sha256: string; bytes: number; images: number; objects: number; negatives: number; excluded: number; summary?:{labels:string[];splits:Partial<Record<"train"|"val"|"test",number>>;warnings:string[];exclusions:Record<string,string>}|null } | null;
  error: string | null;
};
export type DeliveryPackageRead = { job: DeliveryPackageStatus; active: boolean; interrupted: boolean };
export type DeliveryPackageStart = { job: DeliveryPackageStatus; active: boolean; dispatched: boolean };

/** Explicit commands retain caller-owned idempotency keys; reads never start jobs. */
export interface DeliveryService {
  history(project: string, task: string, before?:string, signal?:AbortSignal): Promise<{items:{id:string;created_at:string}[];next_cursor:string|null}>;
  image(project: string, task: string, image: string, run: string | null, signal?: AbortSignal): Promise<DeliveryImageView>;
  confirmImage(project: string, task: string, input: DeliveryReviewInput): Promise<DeliveryReview>;
  startPackage(project: string, task: string, input: DeliveryPackageInput): Promise<DeliveryPackageStart>;
  packageStatus(project: string, task: string, id: string, signal?: AbortSignal): Promise<DeliveryPackageRead>;
  cancelPackage(project: string, task: string, id: string): Promise<DeliveryPackageStatus>;
  downloadUrl(project: string, task: string, id: string): string;
}
