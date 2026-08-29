import { describe, expect, it } from "vitest";
import { clampCanvasZoom, zoomAroundPoint } from "./canvasViewport";

describe("annotation canvas viewport", () => {
  it("clamps zoom to the supported range", () => {
    expect(clampCanvasZoom(0.1)).toBe(0.5);
    expect(clampCanvasZoom(8)).toBe(4);
  });

  it("keeps the image point under the cursor fixed while zooming", () => {
    const anchor: [number, number] = [400, 300];
    const beforePan: [number, number] = [20, -10];
    const imagePoint = [
      (anchor[0] - beforePan[0]) / 1.5,
      (anchor[1] - beforePan[1]) / 1.5,
    ];
    const next = zoomAroundPoint(1.5, beforePan, anchor, 2.25);
    expect(next.pan[0] + imagePoint[0] * next.zoom).toBeCloseTo(anchor[0]);
    expect(next.pan[1] + imagePoint[1] * next.zoom).toBeCloseTo(anchor[1]);
  });
});
