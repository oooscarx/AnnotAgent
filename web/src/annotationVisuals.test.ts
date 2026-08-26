import { describe, expect, it } from "vitest";
import type { Annotation } from "./types";
import { annotationVisual } from "./annotationVisuals";

const annotation = (label: string): Annotation => ({
  id: "annotation-1",
  image_id: "image-1",
  task_id: "objects",
  label,
  value: { kind: "bounding_box", rect: [0.1, 0.1, 0.2, 0.2] },
  attributes: {},
  source: "model",
  review_status: "needs_review",
  provenance: {},
  created_at: "2026-08-27T00:00:00Z",
});

describe("generic annotation visuals", () => {
  it("uses a stable fallback for projects without Skills", () => {
    expect(annotationVisual(annotation("generic_object"))).toEqual(annotationVisual(annotation("generic_object")));
    expect(annotationVisual(annotation("generic_object")).source).toBe("fallback");
  });

  it("applies project, Skill, and schema priority deterministically", () => {
    const visual = annotationVisual(annotation("item"), {
      projectOverrides: { item: { slot: 8, pattern: "crosshair" } },
      skillProfiles: [{ skillId: "z-skill", displayName: "Z", labelVisuals: { item: { slot: 2 } } }],
      schemaVisuals: { item: { slot: 4 } },
    });
    expect(visual).toMatchObject({ slot: 8, pattern: "crosshair", source: "project" });
  });

  it("resolves multi-Skill conflicts by Skill id instead of array order", () => {
    const z = { skillId: "z-skill", displayName: "Z", labelVisuals: { item: { slot: 7 as const } } };
    const a = { skillId: "a-skill", displayName: "A", labelVisuals: { item: { slot: 2 as const } } };
    expect(annotationVisual(annotation("item"), { skillProfiles: [z, a] })).toMatchObject({
      slot: 2,
      sourceId: "a-skill",
    });
  });
});
