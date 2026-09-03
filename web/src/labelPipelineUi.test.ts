import { describe, expect, it } from "vitest";
import {
  artifactCrops,
  artifactCropMarks,
  artifactDetectionMarks,
  artifactMasks,
  annotationDetectionMarks,
  artifactRects,
  decodeCocoRleMask,
  evidenceGateReport,
  pipelineNodeKind,
  pipelineNodeOutput,
  pipelineNodeParameters,
  guidedPipelineStepGroups,
  guidedWorkflowNodes,
  geometrySemanticsLabel,
  scoreSemanticsLabel,
  workflowNodeTitle,
} from "./App";
import type { Annotation, PipelineArtifact, PipelineStep } from "./types";

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
    expect(pipelineNodeOutput("core.match_detection_sets")).toEqual({
      port: "candidates",
      type: "candidate_cluster_set",
    });
    expect(pipelineNodeOutput("core.evidence_gate")).toEqual({
      port: "candidates",
      type: "candidate_cluster_set",
    });
    expect(pipelineNodeOutput("capability.detect")).toEqual({
      port: "detections",
      type: "detection_set",
    });
    expect(pipelineNodeOutput("capability.classify")).toEqual({
      port: "classifications",
      type: "classification_set",
    });
    expect(pipelineNodeOutput("core.combine_evidence")).toEqual({
      port: "candidates",
      type: "candidate_cluster_set",
    });
    expect(pipelineNodeKind("core.crop")).toBe("transform");
    expect(pipelineNodeKind("yolo_detection.detect")).toBe("vision_model");
    expect(pipelineNodeKind("vlm_detection.detect")).toBe("vision_model");
    expect(pipelineNodeKind("core.match_detection_sets")).toBe("candidate_merge");
    expect(pipelineNodeKind("core.evidence_gate")).toBe("gate");
    expect(pipelineNodeParameters("core.crop", "person")).toEqual({
      padding: 0.05,
    });
    expect(pipelineNodeParameters("vlm_detection.detect", "ball")).toMatchObject({
      grounding_assist: {
        mode: "grid",
        enabled: false,
        rows: 10,
        columns: 10,
      },
    });
  });

  it("projects technical nodes into a smaller guided vocabulary", () => {
    expect(workflowNodeTitle("core.filter")).toBe("Select detections");
    expect(workflowNodeTitle("core.map_label")).toBe("Select detections");
    expect(workflowNodeTitle("core.match_detection_sets")).toBe(
      "Combine model evidence",
    );
    expect(workflowNodeTitle("core.evidence_gate")).toBe("Decision");
    expect(workflowNodeTitle("core.select_and_map")).toBe("Select and map results");
    expect(workflowNodeTitle("core.combine_evidence")).toBe("Combine model evidence");
    expect(workflowNodeTitle("core.decision")).toBe("Decision");
    expect(
      guidedWorkflowNodes([
        { node_type: "core.filter" },
        { node_type: "core.map_label" },
        { node_type: "core.confidence_gate" },
        { node_type: "core.evidence_gate" },
      ]),
    ).toEqual([
      { node_type: "core.filter" },
      { node_type: "core.confidence_gate" },
    ]);

    const step = (id: string, node_type: string): PipelineStep => ({
      id,
      node_type,
      kind: "transform",
      inputs: {},
      outputs: {},
      parameters: {},
      validators: [],
      refiners: [],
      retry_policy: { max_attempts: 1 },
      review_gate: { required: false, allow_manual_override: false },
      resources: {},
    });
    expect(
      guidedPipelineStepGroups([
        step("filter", "core.filter"),
        step("map", "core.map_label"),
        step("gate", "core.confidence_gate"),
      ]).map((group) => group.steps.length),
    ).toEqual([2, 1]);
  });

  it("extracts real Detection and Crop geometry for previews", () => {
    const detection: PipelineArtifact = {
      kind: "detection_set",
      artifact: {
        detections: [
          { bbox: [0.1, 0.2, 0.3, 0.4] },
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

  it("decodes uncompressed COCO RLE masks from column-major storage", () => {
    const mask: PipelineArtifact = {
      kind: "mask_set",
      artifact: {
        masks: [{
          mask_id: "mask-a",
          mask: { encoding: "coco_rle", width: 3, height: 2, counts: "1 2 1 1 1" },
        }],
      },
    };
    expect(artifactMasks([mask])).toEqual([{
      id: "mask-a",
      width: 3,
      height: 2,
      counts: [1, 2, 1, 1, 1],
    }]);
    expect(Array.from(decodeCocoRleMask(3, 2, [1, 2, 1, 1, 1]) ?? [])).toEqual([
      0, 1, 1,
      1, 0, 0,
    ]);
    expect(decodeCocoRleMask(3, 2, [1, 1])).toBeUndefined();
  });

  it("previews Candidate Clusters and parses explainable Evidence Gate reports", () => {
    const clusters: PipelineArtifact = {
      kind: "candidate_cluster_set",
      artifact: {
        candidates: [
          {
            id: "cluster-0000",
            target_label: "ball",
            representative_bbox: [0.2, 0.3, 0.15, 0.2],
          },
        ],
      },
    };
    expect(artifactRects([clusters])).toEqual([
      { x: 0.2, y: 0.3, width: 0.15, height: 0.2 },
    ]);
    expect(evidenceGateReport({
      evidence_gate: {
        decision: "review",
        candidate_count: 1,
        validation_issue_count: 0,
        reasons: [{
          code: "score_not_comparable",
          message: "Confidence was not provided or is not comparable",
          candidate_id: "cluster-0000",
          source_model_ids: ["open-model"],
          metrics: {},
        }],
      },
    })).toMatchObject({
      decision: "review",
      candidate_count: 1,
      reasons: [{ code: "score_not_comparable", source_model_ids: ["open-model"] }],
    });
    expect(evidenceGateReport({ evidence_gate: { decision: "maybe", reasons: [] } }))
      .toBeUndefined();
  });

  it("renders the evidence-aware Detection DTO without inventing confidence", () => {
    const detection: PipelineArtifact = {
      kind: "detection_set",
      artifact: {
        schema_version: 2,
        reference: { artifact_id: "open-set", source_node: "open-detector" },
        detections: [
          {
            detection_id: "phrase-1",
            query_id: "query-football",
            model_label: null,
            project_label: "ball",
            bbox: [0.2, 0.3, 0.1, 0.1],
            score: { value: null, semantics: "not_provided" },
            source_model_id: "open-model",
            source_capability: "open_vocabulary_detection",
            evidence: [],
          },
        ],
      },
    };
    expect(artifactDetectionMarks([detection])[0]).toMatchObject({
      id: "phrase-1",
      label: "ball",
      confidence: undefined,
      sourceNode: "open-detector",
      evidence: [{
        source_model_id: "open-model",
        score: { value: undefined, semantics: "not_provided" },
      }],
    });
  });

  it("keeps semantic score and geometry verification separate", () => {
    const detection: PipelineArtifact = {
      kind: "detection_set",
      artifact: {
        detections: [{
          detection_id: "vlm-ball",
          project_label: "ball",
          bbox: [0.4, 0.4, 0.2, 0.2],
          score: { value: 0.99, semantics: "semantic_confidence" },
          source_model_id: "qwen-vlm",
          source_capability: "vision_language",
          geometry_semantics: "coarse_hypothesis",
        }],
      },
    };
    const mark = artifactDetectionMarks([detection])[0];
    expect(mark).toMatchObject({
      confidence: 0.99,
      scoreSemantics: "semantic_confidence",
      geometrySemantics: "coarse_hypothesis",
      calibrationStatus: "uncalibrated",
    });
    expect(scoreSemanticsLabel(mark.scoreSemantics)).toBe("Semantic confidence");
    expect(geometrySemanticsLabel(mark.geometrySemantics)).toBe(
      "Uncalibrated coarse proposal",
    );
  });

  it("keeps every detector box and agreement metric in Candidate Cluster previews", () => {
    const clusters: PipelineArtifact = {
      kind: "candidate_cluster_set",
      artifact: {
        reference: { artifact_id: "clusters", source_node: "candidate-match" },
        candidates: [{
          id: "cluster-1",
          target_label: "football",
          representative_bbox: [0.2, 0.3, 0.1, 0.12],
          agreement: { multi_source_agreement: { minimum_iou: 0.74, mean_iou: 0.81 } },
          members: [{
            source_model_id: "rfdetr-specialist-local",
            source_artifact_id: "specialist-set",
            bbox: [0.2, 0.3, 0.1, 0.12],
            score: { value: 0.87, semantics: "relative_confidence" },
            source_capability: "object_detection",
          }, {
            source_model_id: "locate-anything-local",
            source_artifact_id: "open-set",
            bbox: [0.205, 0.305, 0.095, 0.115],
            score: { value: null, semantics: "not_provided" },
            source_capability: "open_vocabulary_detection",
          }],
        }],
      },
    };

    expect(artifactDetectionMarks([clusters])[0]).toMatchObject({
      id: "cluster-1",
      label: "football",
      agreement: { multi_source_agreement: { minimum_iou: 0.74 } },
      evidence: [
        { source_model_id: "rfdetr-specialist-local", score: { value: 0.87 } },
        { source_model_id: "locate-anything-local", score: { value: undefined } },
      ],
    });
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
