import { describe, expect, it } from "vitest";
import {
  artifactCrops,
  artifactCropMarks,
  artifactDetectionMarks,
  annotationDetectionMarks,
  artifactRects,
  pipelineNodeKind,
  pipelineNodeOutput,
  pipelineNodeParameters,
} from "./App";
import type { Annotation, PipelineArtifact } from "./types";

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

  it("turns a committed bounding-box Annotation into a Run preview mark", () => {
    const annotation: Annotation = {
      id: "annotation-1",
      image_id: "image-1",
      task_id: "objects",
      label: "ball",
      value: { kind: "bounding_box", rect: [0.422, 0.334, 0.055, 0.067] },
      attributes: {},
      confidence: 0.95,
      source: "model",
      review_status: "auto_accepted",
      provenance: {},
      created_at: "2026-08-28T00:00:00Z",
    };

    expect(annotationDetectionMarks([annotation])[0]).toMatchObject({
      id: "annotation-1",
      label: "ball",
      confidence: 0.95,
      x: 0.422,
      y: 0.334,
      width: 0.055,
      height: 0.067,
      sourceNode: "committed annotation",
    });
  });
});
