import { describe, expect, it } from "vitest";
import { canAddRunAnnotation, manualAnnotationValue } from "./RunAnnotationEditor";
import type { ProjectSummary } from "../types";

describe("explicit human Run additions", () => {
  it("offers only project-defined labels and implemented shape editors", () => {
    const project = (kind: string, labels: string[]) => ({ annotation_schema: [{ id: "task", kind, labels }] }) as ProjectSummary;
    expect(canAddRunAnnotation(project("bounding_box", ["cup"]))).toBe(true);
    expect(canAddRunAnnotation(project("bounding_box", []))).toBe(false);
    expect(canAddRunAnnotation(project("relation", ["near"]))).toBe(false);
    expect(manualAnnotationValue("unsupported", "cup")).toBeUndefined();
  });
  it("creates editable values without model predictions or confidence", () => {
    expect(manualAnnotationValue("classification", "cup")).toEqual({ kind: "classification", labels: ["cup"] });
    expect(manualAnnotationValue("bounding_box", "cup")).toEqual({ kind: "bounding_box", rect: [0.35, 0.35, 0.3, 0.3] });
    for (const kind of ["polygon", "semantic_mask", "instance_mask", "keypoints", "polyline"]) expect(manualAnnotationValue(kind, "cup")?.kind).toBe(kind);
  });
});
