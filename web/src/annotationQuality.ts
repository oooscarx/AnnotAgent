import type { Annotation } from "./types";

/** A presentation warning, not a replacement for Core's geometry/acceptance policy. */
export function hasUnverifiedBoundary(annotations: Annotation[]): boolean {
  return annotations.some((annotation) => annotation.value.kind !== "classification"
    && annotation.review_status !== "human_accepted"
    && annotation.source !== "human"
    && annotation.provenance.geometry_semantics !== "human_verified"
    && annotation.provenance.geometry_calibration_status !== "passed");
}
