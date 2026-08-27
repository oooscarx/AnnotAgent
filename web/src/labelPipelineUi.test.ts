import { describe, expect, it } from "vitest";
import {
  artifactCrops,
  artifactCropMarks,
  artifactDetectionMarks,
  artifactRects,
  pipelineNodeKind,
  pipelineNodeOutput,
  pipelineNodeParameters,
} from "./App";
import type { PipelineArtifact } from "./types";

describe("Label Pipeline product helpers", () => {
  it("keeps Crop in Core and classification/detection outputs typed", () => {
    expect(pipelineNodeOutput("core.crop")).toEqual({
      port: "crops",
      type: "crop_set",
    });
    expect(pipelineNodeOutput("classification.classify")).toEqual({
      port: "classifications",
      type: "classification_set",
    });
    expect(pipelineNodeOutput("yolo_detection.detect")).toEqual({
      port: "detections",
      type: "detection_set",
    });
    expect(pipelineNodeOutput("vlm_detection.detect")).toEqual({
      port: "detections",
      type: "detection_set",
    });
    expect(pipelineNodeKind("core.crop")).toBe("transform");
    expect(pipelineNodeKind("yolo_detection.detect")).toBe("vision_model");
    expect(pipelineNodeKind("vlm_detection.detect")).toBe("vision_model");
    expect(pipelineNodeParameters("core.crop", "person")).toEqual({
      padding: 0.05,
    });
  });

  it("extracts real Detection and Crop geometry for previews", () => {
    const detection: PipelineArtifact = {
      kind: "detection_set",
      artifact: {
        detections: [
          { rect: { x: 0.1, y: 0.2, width: 0.3, height: 0.4 } },
        ],
      },
    };
    const crop: PipelineArtifact = {
      kind: "crop_set",
      artifact: { crops: [{ rect: [0.05, 0.1, 0.5, 0.6] }] },
    };
    expect(artifactRects([detection])).toEqual([
      { x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
    ]);
    expect(artifactCrops([crop])).toEqual([
      { x: 0.05, y: 0.1, width: 0.5, height: 0.6 },
    ]);
  });

  it("keeps bbox labels, confidence, and Crop parent linkage", () => {
    const detection: PipelineArtifact = {
      kind: "detection_set",
      artifact: {
        reference: { artifact_id: "set-1", source_node: "detector" },
        detections: [
          {
            id: "ball-1",
            class_id: "football",
            label: "football",
            confidence: 0.93,
            rect: [0.1, 0.2, 0.3, 0.4],
          },
        ],
      },
    };
    const crop: PipelineArtifact = {
      kind: "crop_set",
      artifact: {
        reference: { artifact_id: "crops-1", source_node: "crop" },
        crops: [
          {
            id: "crop:ball-1",
            parent: { artifact_id: "set-1", item_id: "ball-1" },
            rect: [0.1, 0.2, 0.3, 0.4],
          },
        ],
      },
    };
    const detections = artifactDetectionMarks([detection]);
    expect(detections[0]).toMatchObject({
      id: "ball-1",
      label: "football",
      confidence: 0.93,
      sourceNode: "detector",
    });
    expect(artifactCropMarks([crop], detections)[0]).toMatchObject({
      parentId: "ball-1",
      parentArtifact: "set-1",
      label: "football",
      sourceNode: "crop",
    });
  });
});
