import { describe, expect, it } from "vitest";
import { annotationVisual } from "./annotationVisuals";
import type { Annotation } from "./types";

const annotation = (label: string): Annotation => ({
  id: "annotation",
  image_id: "image",
  task_id: label,
  label,
  value: { kind: "bounding_box", rect: [0.1, 0.1, 0.2, 0.2] },
  attributes: {},
  source: "model",
  review_status: "needs_review",
  provenance: {},
  created_at: "2026-08-27T00:00:00Z",
});

describe("annotation visual mapping", () => {
  it("loads RoboCup labels from the packaged mapping and keeps generic fallback slots", () => {
    expect(annotationVisual(annotation("ball"), "robocup")).toEqual({
      slot: 4,
      pattern: "circle-label",
    });
    expect(annotationVisual(annotation("custom-label"), "other").slot).toBeGreaterThanOrEqual(1);
    expect(annotationVisual(annotation("custom-label"), "other").slot).toBeLessThanOrEqual(8);
  });
});
