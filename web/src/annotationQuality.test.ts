import { expect, it } from "vitest";
import { hasUnverifiedBoundary } from "./annotationQuality";
import type { Annotation } from "./types";

const annotation: Annotation = { id: "a", image_id: "image", task_id: "objects", label: "cup", value: { kind: "bounding_box", rect: [0.1, 0.1, 0.2, 0.2] }, attributes: {}, confidence: 0.99, source: "runtime", review_status: "needs_review", provenance: {}, created_at: "" };

it("does not treat a high semantic score or automatic acceptance as verified geometry", () => {
  expect(hasUnverifiedBoundary([annotation])).toBe(true);
  expect(hasUnverifiedBoundary([{ ...annotation, review_status: "auto_accepted" }])).toBe(true);
  expect(hasUnverifiedBoundary([{ ...annotation, review_status: "human_accepted" }])).toBe(false);
  expect(hasUnverifiedBoundary([{ ...annotation, provenance: { geometry_calibration_status: "passed" } }])).toBe(false);
  expect(hasUnverifiedBoundary([{ ...annotation, value: { kind: "classification", labels: ["cup"] } }])).toBe(false);
});
