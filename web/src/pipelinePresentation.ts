import type { AgentSession, WorkflowDraft } from "./types";

// A node is recovery-only if a required input can only come from recovery.
// An OR-merge fed by both the main and recovery paths remains on the main path.
export function recoveryNodeIds(draft: Pick<WorkflowDraft, "nodes" | "edges">): Set<string> {
  const edges = draft.edges ?? [];
  const recovery = new Set(edges.filter(edge => ["relocalize", "search_tiles"].includes(edge.route ?? "")).map(edge => edge.to_node));
  for (;;) {
    const before = recovery.size;
    for (const node of draft.nodes) {
      if ((node.inputs ?? []).filter(port => port.required).some(port => {
        const producers = edges.filter(edge => edge.to_node === node.id && edge.to_port === port.id);
        return producers.length > 0 && producers.every(edge => recovery.has(edge.from_node));
      })) recovery.add(node.id);
    }
    if (before === recovery.size) return recovery;
  }
}

export function builderStopLabel(session: Pick<AgentSession, "builder_stop_reason" | "stop_reason">): string {
  const reason = session.builder_stop_reason ?? session.stop_reason;
  if (["discovery_limit_triggered_salvage", "DiscoveryLimitTriggeredSalvage"].includes(reason ?? "")) return "Exploration stopped; saved available plan";
  if (["runnable_candidate_triggered_salvage", "RunnableCandidateTriggeredSalvage"].includes(reason ?? "")) return "Compatible plan found";
  if (["draft_ready", "DraftReady"].includes(reason ?? "")) return "Draft saved for review";
  return reason?.replaceAll("_", " ") ?? "Completed";
}

export function builderPlanSource(session: Pick<AgentSession, "plan_candidates" | "selected_candidate_id">): string {
  const source = session.plan_candidates?.find(candidate => candidate.id === session.selected_candidate_id)?.source;
  if (source === "registry_synthesis") return "System-composed from registered capabilities";
  if (source === "template_seed") return "Template-based plan";
  return source ? "Preserved candidate plan" : "Source not recorded";
}

import type { Annotation, PipelineArtifact, PipelineStep, PipelineArtifactType, ProjectSummary, DetectionEvidenceDto, EvidenceGateReportDto } from "./types";
import { visualProfilesForSkills } from "./skills/visualProfiles";
import { annotationColor, annotationVisual, type LabelVisualMapping } from "./annotationVisuals";

/** Pure presentation and geometry helpers shared by the new management UI. No page or API dependency. */
export function projectOriginalRectToSubmitted(
  rect: [number, number, number, number],
  trace: ModelInputTraceView,
): [number, number, number, number] | undefined {
  const source = trace.transform_to_original?.source_region;
  const content = trace.transform_to_original?.content_region;
  if (!source || source.length !== 4 || !content || content.length !== 4
    || source[2] <= 0 || source[3] <= 0) return undefined;
  const left = Math.max(rect[0], source[0]);
  const top = Math.max(rect[1], source[1]);
  const right = Math.min(rect[0] + rect[2], source[0] + source[2]);
  const bottom = Math.min(rect[1] + rect[3], source[1] + source[3]);
  if (right <= left || bottom <= top) return undefined;
  const x = content[0] + ((left - source[0]) / source[2]) * content[2];
  const y = content[1] + ((top - source[1]) / source[3]) * content[3];
  const width = ((right - left) / source[2]) * content[2];
  const height = ((bottom - top) / source[3]) * content[3];
  return [clampUnit(x), clampUnit(y), clampUnit(width), clampUnit(height)];
}

export function workflowNodeTitle(nodeType: string): string {
  const known: Record<string, string> = {
    "core.image_input": "Read each image",
    "core.existing_annotations": "Read existing annotations",
    "core.resize": "Resize image",
    "core.tile": "Tile image",
    "capability.detect": "Find objects",
    "capability.classify": "Classify crops or images",
    "capability.segment": "Segment regions",
    "vlm_detection.detect": "Find objects",
    "yolo_detection.detect": "Find objects",
    "classification.classify": "Classify crops or images",
    "core.filter": "Select detections",
    "core.project_detection_candidates": "Select detections",
    "core.crop": "Crop candidates",
    "core.detections_to_box_prompts": "Convert detections to box prompts",
    "core.mask_to_bbox": "Convert masks to bounding boxes",
    "core.mask_to_polygon": "Convert masks to polygons",
    "core.map_label": "Select detections",
    "core.select_and_map": "Select and map results",
    "core.project_coordinates": "Project coordinates",
    "core.attach_result": "Combine model evidence",
    "core.candidate_merge": "Combine model evidence",
    "core.match_detection_sets": "Combine model evidence",
    "core.combine_evidence": "Combine model evidence",
    "core.attach_attribute": "Attach attributes",
    "core.confidence_gate": "Decision",
    "core.evidence_gate": "Decision",
    "core.validate": "Validate results",
    "core.decision": "Decision",
    "core.human_review": "Send uncertain results to Review",
    "core.commit": "Save annotations",
    "core.artifact_cache": "Keep replayable artifacts",
  };
  return known[nodeType] ?? nodeType.split(".").at(-1)?.replaceAll("_", " ") ?? nodeType;
}

export function guidedWorkflowNodes<T extends { node_type: string }>(nodes: T[]): T[] {
  return nodes.filter((node, index) =>
    index === 0 ||
    guidedWorkflowConcept(node.node_type) !== guidedWorkflowConcept(nodes[index - 1].node_type),
  );
}

export function guidedPipelineStepGroups(steps: PipelineStep[]): Array<{
  firstIndex: number;
  steps: PipelineStep[];
}> {
  return steps.reduce<Array<{ firstIndex: number; steps: PipelineStep[] }>>(
    (groups, step, index) => {
      const previous = groups.at(-1);
      if (
        previous &&
        guidedWorkflowConcept(previous.steps[0].node_type) === guidedWorkflowConcept(step.node_type)
      ) {
        previous.steps.push(step);
      } else {
        groups.push({ firstIndex: index, steps: [step] });
      }
      return groups;
    },
    [],
  );
}

export function pipelineNodeOutput(nodeType: string): {
  port: string;
  type: PipelineArtifactType;
} {
  if (nodeType === "core.crop") return { port: "crops", type: "crop_set" };
  if (nodeType === "core.detections_to_box_prompts")
    return { port: "prompts", type: "box_prompt_set" };
  if (nodeType === "capability.segment") return { port: "masks", type: "mask_set" };
  if (nodeType === "core.mask_to_polygon") return { port: "polygons", type: "polygon_set" };
  if (nodeType === "core.resize" || nodeType === "core.tile")
    return { port: "images", type: "image" };
  if (nodeType === "classification.classify" || nodeType === "capability.classify")
    return { port: "classifications", type: "classification_set" };
  if (nodeType === "core.attach_result" || nodeType === "core.attach_attribute")
    return { port: "candidates", type: "annotation_candidate_set" };
  if (["core.confidence_gate", "core.decision", "core.validate"].includes(nodeType))
    return { port: "candidates", type: "annotation_candidate_set" };
  if (["core.match_detection_sets", "core.combine_evidence", "core.evidence_gate"].includes(nodeType))
    return { port: "candidates", type: "candidate_cluster_set" };
  return { port: "detections", type: "detection_set" };
}

export function pipelineNodeKind(nodeType: string): NonNullable<PipelineStep["kind"]> {
  if (["core.attach_result", "core.match_detection_sets", "core.combine_evidence"].includes(nodeType))
    return "candidate_merge";
  if (["core.confidence_gate", "core.evidence_gate", "core.decision"].includes(nodeType))
    return "gate";
  if (nodeType === "core.validate") return "validator";
  if (nodeType.includes("classify") || nodeType.includes("detect") || nodeType.includes("segment"))
    return "vision_model";
  return "transform";
}

export function pipelineNodeParameters(nodeType: string, label: string) {
  if (nodeType === "core.crop") return { padding: 0.05 };
  if (nodeType === "core.resize") return { max_edge: 1600, allow_upscale: false };
  if (nodeType === "core.tile")
    return { tile_size: 1024, overlap: 0.15, maximum_tiles: 64, merge_policy: "nms" };
  if (nodeType === "core.filter")
    return { labels: [label], minimum_confidence: 0.5 };
  if (nodeType === "core.map_label") return { class_mapping: {} };
  if (nodeType === "core.select_and_map")
    return { labels: [label], minimum_confidence: 0.5, class_mapping: {}, drop_unknown_labels: false };
  if (nodeType === "core.confidence_gate") return { threshold: 0.9 };
  if (nodeType === "core.decision") return { mode: "confidence", threshold: 0.9 };
  if (nodeType === "core.match_detection_sets" || nodeType === "core.combine_evidence")
    return { method: "iou", minimum_iou: 0.5, preserve_unmatched: true };
  if (nodeType === "core.evidence_gate")
    return {
      accept_when: [{ minimum_sources: 2, minimum_iou: 0.6 }],
      fallback_when: [],
      review_when: [{ geometry_conflict: true, label_conflict: true, score_missing: true }],
      reject_when: [],
    };
  if (nodeType === "classification.classify" || nodeType === "capability.classify")
    return { labels: [label] };
  if (nodeType === "vlm_detection.detect" || nodeType === "capability.detect")
    return {
      labels: [label],
      object_description: `Locate every visible ${label} and return a tight normalized bounding box.`,
      max_detections: 20,
      grounding_assist: {
        mode: "grid",
        enabled: false,
        rows: 10,
        columns: 10,
      },
    };
  return {};
}

export function workflowNodeModelCapability(nodeType: string): string | undefined {
  return nodeType === "vlm_detection.detect"
    ? "vision_language"
    : nodeType === "capability.segment"
    ? "prompted_segmentation"
    : nodeType.includes("classify")
    ? "classification"
    : nodeType.includes("detect")
      ? "object_detection"
      : undefined;
}

export function decodeCocoRleMask(
  width: number,
  height: number,
  counts: number[],
): Uint8Array | undefined {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width <= 0 || height <= 0)
    return undefined;
  const pixelCount = width * height;
  if (!Number.isSafeInteger(pixelCount) || pixelCount > 16_000_000) return undefined;
  if (counts.some((count) => !Number.isSafeInteger(count) || count < 0)) return undefined;
  if (counts.reduce((total, count) => total + count, 0) !== pixelCount) return undefined;
  const pixels = new Uint8Array(pixelCount);
  let sourceIndex = 0;
  counts.forEach((count, runIndex) => {
    if (runIndex % 2 === 1) {
      for (let offset = 0; offset < count; offset += 1) {
        const columnMajorIndex = sourceIndex + offset;
        const x = Math.floor(columnMajorIndex / height);
        const y = columnMajorIndex % height;
        pixels[y * width + x] = 1;
      }
    }
    sourceIndex += count;
  });
  return pixels;
}

export function artifactMasks(artifacts: PipelineArtifact[]): ArtifactMask[] {
  return artifacts.flatMap((artifact, artifactIndex) => {
    if (artifact.kind !== "mask_set" || !Array.isArray(artifact.artifact.masks)) return [];
    return artifact.artifact.masks.flatMap((item, maskIndex) => {
      if (!item || typeof item !== "object") return [];
      const record = item as Record<string, unknown>;
      if (!record.mask || typeof record.mask !== "object") return [];
      const mask = record.mask as Record<string, unknown>;
      if (mask.encoding !== "coco_rle"
        || typeof mask.width !== "number"
        || typeof mask.height !== "number"
        || typeof mask.counts !== "string") return [];
      const counts = mask.counts.trim().split(/\s+/).filter(Boolean).map(Number);
      if (!decodeCocoRleMask(mask.width, mask.height, counts)) return [];
      return [{
        id: typeof record.mask_id === "string"
          ? record.mask_id
          : `mask-${artifactIndex}-${maskIndex}`,
        width: mask.width,
        height: mask.height,
        counts,
      }];
    });
  });
}

export function artifactRects(artifacts: PipelineArtifact[]): ArtifactRect[] {
  return artifacts.flatMap((artifact) => {
    const detections = artifact.kind === "detection_set"
      ? artifact.artifact.detections
      : artifact.kind === "candidate_cluster_set"
        ? artifact.artifact.candidates
        : artifact.kind === "box_prompt_set"
          ? artifact.artifact.prompts
        : undefined;
    if (Array.isArray(detections)) {
      return detections.flatMap((detection) => {
        if (!detection || typeof detection !== "object") return [];
        const record = detection as Record<string, unknown>;
        const rect = record.representative_bbox ?? record.bbox ?? record.rect;
        return parseArtifactRect(rect) ? [parseArtifactRect(rect)!] : [];
      });
    }
    const polygonItems = artifact.kind === "mask_set"
      ? artifact.artifact.masks
      : artifact.kind === "polygon_set"
        ? artifact.artifact.polygons
        : undefined;
    if (!Array.isArray(polygonItems)) return [];
    return polygonItems.flatMap((item) => {
      if (!item || typeof item !== "object") return [];
      const record = item as Record<string, unknown>;
      const bounds = artifactPolygonBounds(
        artifact.kind === "mask_set" ? record.mask : { rings: record.rings },
      );
      return bounds ? [bounds] : [];
    });
  });
}

export function artifactCrops(artifacts: PipelineArtifact[]): ArtifactRect[] {
  return artifacts.flatMap((artifact) => {
    if (artifact.kind !== "crop_set") return [];
    const crops = artifact.artifact.crops;
    if (!Array.isArray(crops)) return [];
    return crops.flatMap((crop) => {
      if (!crop || typeof crop !== "object") return [];
      const rect = (crop as Record<string, unknown>).rect;
      return parseArtifactRect(rect) ? [parseArtifactRect(rect)!] : [];
    });
  });
}

export function evidenceGateReport(
  metadata: Record<string, unknown>,
): EvidenceGateReportDto | undefined {
  const value = metadata.evidence_gate;
  if (!value || typeof value !== "object") return undefined;
  const report = value as Record<string, unknown>;
  if (!(["accept", "fallback", "review", "reject"] as const).includes(
    report.decision as "accept" | "fallback" | "review" | "reject",
  )) return undefined;
  if (!Array.isArray(report.reasons)) return undefined;
  const reasons = report.reasons.flatMap((reason) => {
    if (!reason || typeof reason !== "object") return [];
    const item = reason as Record<string, unknown>;
    if (typeof item.code !== "string" || typeof item.message !== "string") return [];
    return [{
      code: item.code,
      message: item.message,
      candidate_id: typeof item.candidate_id === "string" ? item.candidate_id : undefined,
      source_model_ids: Array.isArray(item.source_model_ids)
        ? item.source_model_ids.filter((source): source is string => typeof source === "string")
        : [],
      metrics: item.metrics && typeof item.metrics === "object"
        ? Object.fromEntries(
          Object.entries(item.metrics as Record<string, unknown>)
            .filter((entry): entry is [string, number] =>
              typeof entry[1] === "number" && Number.isFinite(entry[1]),
            ),
        )
        : {},
    }];
  });
  return {
    decision: report.decision as EvidenceGateReportDto["decision"],
    reasons,
    candidate_count: typeof report.candidate_count === "number" ? report.candidate_count : 0,
    validation_issue_count:
      typeof report.validation_issue_count === "number" ? report.validation_issue_count : 0,
  };
}

export function geometrySemanticsLabel(value?: string): string {
  const labels: Record<string, string> = {
    not_applicable: "Not applicable",
    coarse_hypothesis: "Uncalibrated coarse proposal",
    predicted_geometry: "Predicted box",
    refined_geometry: "Refined by prompted segmentation",
    mask_refined_geometry: "Refined by prompted segmentation",
    calibrated_geometry: "Project-calibrated geometry",
    human_verified: "Human-verified geometry",
  };
  return value ? labels[value] ?? value.replaceAll("_", " ") : "Geometry source not recorded";
}

export function scoreSemanticsLabel(value?: string): string {
  const labels: Record<string, string> = {
    semantic_confidence: "Semantic confidence",
    detection_confidence: "Detection confidence",
    calibrated_probability: "Calibrated probability",
    relative_confidence: "Relative model score",
    ranking_score: "Ranking score",
    not_provided: "Model score",
    unknown: "Model score",
  };
  return value ? labels[value] ?? value.replaceAll("_", " ") : "Model score";
}

export function artifactDetectionMarks(
  artifacts: PipelineArtifact[],
  project?: ProjectSummary,
): ArtifactMark[] {
  return artifacts.flatMap((artifact) => {
    if (artifact.kind !== "detection_set" && artifact.kind !== "candidate_cluster_set") return [];
    const clusterSet = artifact.kind === "candidate_cluster_set";
    const detections = clusterSet ? artifact.artifact.candidates : artifact.artifact.detections;
    const reference = artifact.artifact.reference as Record<string, unknown> | undefined;
    if (!Array.isArray(detections)) return [];
    return detections.flatMap((value, index) => {
      if (!value || typeof value !== "object") return [];
      const detection = value as Record<string, unknown>;
      const rect = parseArtifactRect(detection.representative_bbox ?? detection.bbox ?? detection.rect);
      if (!rect) return [];
      const label = typeof detection.target_label === "string"
        ? detection.target_label
        : typeof detection.project_label === "string"
        ? detection.project_label
        : typeof detection.model_label === "string"
          ? detection.model_label
          : typeof detection.label === "string"
            ? detection.label
            : typeof detection.class_id === "string"
              ? detection.class_id
              : "detection";
      const evidence = parseDetectionEvidence(detection.members ?? detection.evidence);
      if (!evidence.length && typeof detection.source_model_id === "string") {
        evidence.push({
          source_model_id: detection.source_model_id,
          source_artifact_id: typeof reference?.artifact_id === "string" ? reference.artifact_id : "unknown",
          bbox: [rect.x, rect.y, rect.width, rect.height],
          score: {
            value: detectionScoreValue(detection),
            semantics: detection.score && typeof detection.score === "object" &&
              typeof (detection.score as Record<string, unknown>).semantics === "string"
              ? (detection.score as Record<string, unknown>).semantics as DetectionEvidenceDto["score"]["semantics"]
              : "unknown",
          },
          query_id: typeof detection.query_id === "string" ? detection.query_id : undefined,
          model_label: typeof detection.model_label === "string" ? detection.model_label : undefined,
          project_label: label,
          source_capability: typeof detection.source_capability === "string" ? detection.source_capability : "object_detection",
        });
      }
      const geometryState = geometryStateFromDetection(detection, evidence);
      return [{
        ...rect,
        id: typeof detection.detection_id === "string"
          ? detection.detection_id
          : typeof detection.id === "string"
            ? detection.id
            : `detection-${index}`,
        label,
        confidence: detectionScoreValue(detection),
        color: markColor(label, project),
        parentArtifact: typeof reference?.artifact_id === "string" ? reference.artifact_id : undefined,
        sourceNode: typeof reference?.source_node === "string" ? reference.source_node : undefined,
        evidence,
        ...geometryState,
        agreement: clusterSet ? detection.agreement as ArtifactMark["agreement"] : undefined,
      }];
    });
  });
}

export function artifactCropMarks(
  artifacts: PipelineArtifact[],
  detections: ArtifactMark[],
): ArtifactMark[] {
  return artifacts.flatMap((artifact) => {
    if (artifact.kind !== "crop_set") return [];
    const crops = artifact.artifact.crops;
    const reference = artifact.artifact.reference as Record<string, unknown> | undefined;
    if (!Array.isArray(crops)) return [];
    return crops.flatMap((value, index) => {
      if (!value || typeof value !== "object") return [];
      const crop = value as Record<string, unknown>;
      const rect = parseArtifactRect(crop.rect);
      if (!rect) return [];
      const parent = crop.parent as Record<string, unknown> | undefined;
      const parentId = typeof parent?.item_id === "string" ? parent.item_id : undefined;
      const detection = detections.find((item) => item.id === parentId);
      return [{
        ...rect,
        id: typeof crop.id === "string" ? crop.id : `crop-${index}`,
        parentId,
        label: detection?.label ?? "crop",
        confidence: detection?.confidence,
        color: detection?.color ?? markColor("crop"),
        parentArtifact: typeof parent?.artifact_id === "string" ? parent.artifact_id : undefined,
        sourceNode: typeof reference?.source_node === "string" ? reference.source_node : undefined,
        evidence: detection?.evidence ?? [],
        agreement: detection?.agreement,
      }];
    });
  });
}

export function annotationDetectionMarks(
  annotations: Annotation[],
  project?: ProjectSummary,
): ArtifactMark[] {
  return annotations.flatMap((annotation) => {
    if (annotation.value.kind !== "bounding_box") return [];
    const [x, y, width, height] = annotation.value.rect;
    const label = annotation.label ?? annotation.task_id;
    return [{
      x,
      y,
      width,
      height,
      id: annotation.id,
      label,
      confidence: annotation.confidence,
      color: markColor(label, project),
      sourceNode: "committed annotation",
      evidence: [],
      scoreSemantics: typeof annotation.provenance.score_semantics === "string"
        ? annotation.provenance.score_semantics
        : annotation.confidence === undefined ? "not_provided" : "unknown",
      geometrySemantics: annotation.review_status === "human_accepted" || annotation.source === "human"
        ? "human_verified"
        : typeof annotation.provenance.geometry_semantics === "string"
          ? annotation.provenance.geometry_semantics
          : undefined,
      calibrationStatus: typeof annotation.provenance.geometry_calibration_status === "string"
        ? annotation.provenance.geometry_calibration_status
        : annotation.review_status === "human_accepted" ? "passed" : "uncalibrated",
    }];
  });
}

export function clampUnit(value: number) {
  return Math.max(0, Math.min(1, value));
}

export function guidedWorkflowConcept(nodeType: string): string {
  if (["core.filter", "core.map_label", "core.project_detection_candidates", "core.select_and_map"].includes(nodeType))
    return "select_detections";
  if (["core.attach_result", "core.candidate_merge", "core.match_detection_sets", "core.combine_evidence"].includes(nodeType))
    return "combine_model_evidence";
  if (["core.confidence_gate", "core.evidence_gate", "core.decision"].includes(nodeType))
    return "decision";
  if (nodeType.includes("detect") || nodeType.includes("ground")) return "find_objects";
  return nodeType;
}

export function parseArtifactRect(value: unknown): ArtifactRect | undefined {
  if (Array.isArray(value) && value.length === 4 && value.every((item) => typeof item === "number"))
    return { x: value[0], y: value[1], width: value[2], height: value[3] };
  if (!value || typeof value !== "object") return undefined;
  const rect = value as Record<string, unknown>;
  const x = rect.x;
  const y = rect.y;
  const width = rect.width;
  const height = rect.height;
  return [x, y, width, height].every((item) => typeof item === "number")
    ? { x: x as number, y: y as number, width: width as number, height: height as number }
    : undefined;
}

export function artifactPolygonBounds(value: unknown): ArtifactRect | undefined {
  if (!value || typeof value !== "object") return undefined;
  const rings = (value as Record<string, unknown>).rings;
  if (!Array.isArray(rings)) return undefined;
  const points = rings.flatMap((ring) => Array.isArray(ring) ? ring : []).flatMap((point) => {
    if (!point || typeof point !== "object") return [];
    const record = point as Record<string, unknown>;
    return typeof record.x === "number" && typeof record.y === "number"
      ? [{ x: record.x, y: record.y }]
      : [];
  });
  if (points.length === 0) return undefined;
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  const left = Math.min(...xs);
  const top = Math.min(...ys);
  return {
    x: left,
    y: top,
    width: Math.max(...xs) - left,
    height: Math.max(...ys) - top,
  };
}

export function geometryStateFromDetection(
  detection: Record<string, unknown>,
  evidence: DetectionEvidenceDto[],
): Pick<ArtifactMark, "scoreSemantics" | "geometrySemantics" | "calibrationStatus" | "geometryReportId" | "geometryIssues"> {
  const quality = detection.quality && typeof detection.quality === "object"
    ? detection.quality as Record<string, unknown>
    : undefined;
  const geometry = quality?.geometry && typeof quality.geometry === "object"
    ? quality.geometry as Record<string, unknown>
    : undefined;
  const sourceCapability = evidence[0]?.source_capability ?? detection.source_capability;
  const conservativeGeometry = sourceCapability === "vision_language"
    ? "coarse_hypothesis"
    : typeof sourceCapability === "string" && (sourceCapability.includes("detect") || sourceCapability.includes("ground"))
      ? "predicted_geometry"
      : undefined;
  return {
    scoreSemantics: typeof (detection.score as Record<string, unknown> | undefined)?.semantics === "string"
      ? String((detection.score as Record<string, unknown>).semantics)
      : evidence[0]?.score.semantics,
    geometrySemantics: typeof geometry?.semantics === "string"
      ? geometry.semantics
      : typeof detection.geometry_semantics === "string"
        ? detection.geometry_semantics
        : conservativeGeometry,
    calibrationStatus: typeof geometry?.calibration_status === "string"
      ? geometry.calibration_status
      : typeof detection.calibration_status === "string"
        ? detection.calibration_status
        : "uncalibrated",
    geometryReportId: typeof geometry?.report_id === "string" ? geometry.report_id : undefined,
    geometryIssues: Array.isArray(detection.geometry_issue_codes)
      ? detection.geometry_issue_codes.filter((value): value is string => typeof value === "string")
      : [],
  };
}

export function artifactVisualContext(project?: ProjectSummary) {
  const schemaVisuals: Record<string, LabelVisualMapping> = {};
  project?.annotation_schema.flatMap((task) => task.labels).forEach((label, index) => {
    schemaVisuals[label] = { slot: ((index % 8) + 1) as LabelVisualMapping["slot"] };
  });
  return {
    projectOverrides: project?.annotation_visuals as
      | Record<string, LabelVisualMapping>
      | undefined,
    skillProfiles: visualProfilesForSkills(project?.enabled_skills.map((skill) => skill.id) ?? []),
    schemaVisuals,
  };
}

export function markColor(label: string, project?: ProjectSummary): string {
  const visual = annotationVisual(
    {
      id: "preview",
      image_id: "preview",
      task_id: label,
      label,
      value: { kind: "bounding_box", rect: [0, 0, 1, 1] },
      attributes: {},
      source: "model",
      review_status: "draft",
      provenance: {},
      created_at: "",
    },
    artifactVisualContext(project),
  );
  return annotationColor(visual.slot);
}

export function detectionScoreValue(detection: Record<string, unknown>): number | undefined {
  const score = detection.score;
  if (score && typeof score === "object") {
    const value = (score as Record<string, unknown>).value;
    return typeof value === "number" ? value : undefined;
  }
  return typeof detection.confidence === "number" ? detection.confidence : undefined;
}

export function parseDetectionEvidence(value: unknown): DetectionEvidenceDto[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    if (!item || typeof item !== "object") return [];
    const evidence = item as Record<string, unknown>;
    const rect = parseArtifactRect(evidence.bbox);
    if (!rect || typeof evidence.source_model_id !== "string") return [];
    const score = evidence.score && typeof evidence.score === "object"
      ? evidence.score as Record<string, unknown>
      : {};
    return [{
      source_model_id: evidence.source_model_id,
      source_model_display_name: typeof evidence.source_model_display_name === "string" ? evidence.source_model_display_name : undefined,
      source_artifact_id: typeof evidence.source_artifact_id === "string" ? evidence.source_artifact_id : "unknown",
      bbox: [rect.x, rect.y, rect.width, rect.height],
      score: {
        value: typeof score.value === "number" ? score.value : undefined,
        semantics: typeof score.semantics === "string" ? score.semantics as DetectionEvidenceDto["score"]["semantics"] : "unknown",
      },
      query_id: typeof evidence.query_id === "string" ? evidence.query_id : undefined,
      model_label: typeof evidence.model_label === "string" ? evidence.model_label : undefined,
      project_label: typeof evidence.project_label === "string" ? evidence.project_label : undefined,
      source_capability: typeof evidence.source_capability === "string" ? evidence.source_capability : "object_detection",
      raw_output_ref: evidence.raw_output_ref as DetectionEvidenceDto["raw_output_ref"],
    }];
  });
}

export type ArtifactRect = { x: number; y: number; width: number; height: number };

export type ArtifactMask = { id: string; width: number; height: number; counts: number[] };

export type ArtifactMark = ArtifactRect & {
  id: string;
  label: string;
  confidence?: number;
  color: string;
  parentId?: string;
  parentArtifact?: string;
  sourceNode?: string;
  evidence: DetectionEvidenceDto[];
  scoreSemantics?: string;
  geometrySemantics?: string;
  calibrationStatus?: string;
  geometryReportId?: string;
  geometryIssues?: string[];
  agreement?: "single_source" | "geometry_conflict" | "label_conflict" | { multi_source_agreement: { minimum_iou: number; mean_iou: number } };
};

export type ModelInputTraceView = {
  source_region_pixels: number[];
  crop_dimensions: number[];
  submitted_dimensions: number[];
  submitted_image_sha256: string;
  normalized_pixel_digest: string;
  interpolation: string;
  color_format: string;
  letterbox_padding: number[];
  provider_effective_dimensions?: unknown;
  transform_to_original?: {
    source_region: number[];
    content_region: number[];
  };
};
