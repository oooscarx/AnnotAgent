import { describe, expect, it } from "vitest";
import { comparisonBoxes } from "./SampleGeometryComparison";

describe("saved geometry comparison", () => {
  it("deduplicates repeated artifacts per stage without hiding rejected refinements", () => {
    const value = { kind: "bounding_box", rect: [0.1, 0.2, 0.3, 0.4] };
    const stage = { stage: "refined", label: "target", value, detail: "refiner drift" };
    const sample = { projection: { debug_stages: [stage, { ...stage }, { ...stage, stage: "coarse" }, { stage: "mask", value: null }] }, outcomes: [{ label: "target", value, status: "needs_review" }] } as unknown as Parameters<typeof comparisonBoxes>[0];
    const boxes = comparisonBoxes(sample);
    expect(boxes.map((box) => box.group)).toEqual(["refined", "retained"]);
    expect(boxes[0].detail).toBe("refiner drift");
    expect(boxes[1].detail).toBe("needs_review");
  });
  it("does not invent geometry for missing projection evidence", () => {
    expect(comparisonBoxes({ outcomes: [] } as unknown as Parameters<typeof comparisonBoxes>[0])).toEqual([]);
  });
  it("only overlays coarse detections proven to use the root coordinate frame", () => {
    const stage = { stage: "coarse", label: "target", value: { kind: "bounding_box", rect: [0.1, 0.2, 0.3, 0.4] } };
    const sample = { width: 1280, height: 720, nodes: [
      { node_id: "whole", metadata: { model_input_trace: { source_region_pixels: [0, 0, 1280, 720] } } },
      { node_id: "crop", metadata: { model_input_trace: { source_region_pixels: [100, 100, 96, 96] } } },
    ], projection: { debug_stages: [{ ...stage, node_id: "whole" }, { ...stage, node_id: "crop" }] }, outcomes: [] } as unknown as Parameters<typeof comparisonBoxes>[0];
    expect(comparisonBoxes(sample)).toHaveLength(1);
  });
});
