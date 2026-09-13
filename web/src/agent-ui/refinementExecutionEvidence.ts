import type { WorkflowDryRunReport } from "../types";

type Sample = WorkflowDryRunReport["samples"][number];
type DebugStage = NonNullable<Sample["projection"]>["debug_stages"][number];

export type RefinementEvidenceItem = {
  kind: "node_receipt" | "artifact" | "gate";
  label: string;
  node_id: string;
  status: string | null;
  artifact_id: string | null;
  artifact_ref: string | null;
  detail: string | null;
};

export type CandidateRefinementEvidence = {
  candidate_id: string;
  lineage_id: string | null;
  source: "vlm_only" | "prompted_segmentation_refined";
  executed: boolean;
  reason: string;
  items: RefinementEvidenceItem[];
};

export type ImageRefinementEvidence = {
  configured_refiner: boolean;
  candidates: CandidateRefinementEvidence[];
};

const normalized = (value: unknown) => String(value ?? "").toLowerCase().replace(/[^a-z0-9]+/g, "");
const succeeded = (status: string | undefined) => ["succeeded", "success", "completed", "passed"].includes(normalized(status));
const maskToBbox = (stage: DebugStage) => {
  const marker = `${normalized(stage.node_id)}${normalized(stage.source)}${normalized(stage.detail)}`;
  return marker.includes("masktobbox");
};
const promptedMaskLineage = (terminalLineage: string) => {
  const prefix = "detection:";
  return terminalLineage.startsWith(prefix)
    ? `${prefix}box-prompt:${terminalLineage.slice(prefix.length)}`
    : null;
};
const terminalCandidates = (sample: Sample) => [
  ...(sample.projection?.final_candidates ?? []),
  ...(sample.projection?.review_candidates ?? []).map((item) => item.candidate),
];
const ref = (stage: DebugStage) => ({
  node_id: stage.node_id,
  artifact_id: stage.artifact_id || null,
  artifact_ref: stage.artifact_ref || null,
  detail: stage.detail || null,
});

function stoppedReason(stages: DebugStage[], configured: boolean): string {
  const coverage = [...stages].reverse().find((stage) => stage.stage === "prompt_coverage");
  if (coverage?.detail) return `方案包含精修，但本图实际 lineage 停在 Coverage Gate：${coverage.detail}。`;
  if (!configured) return "此方案未配置提示分割精修；当前终端候选仅来自视觉定位。";
  return "方案包含精修，但本图没有形成提示分割调用、Mask Artifact 与 mask_to_bbox Artifact 的完整证据链。";
}

/**
 * A configured node is not execution evidence. We claim refinement only when a
 * terminal candidate's exact detection/prompt lineage contains both persisted
 * artifacts and successful receipts for segmentation and mask-to-bbox conversion.
 */
export function imageRefinementEvidence(
  sample: Sample,
  candidateIds: string[],
  configuredRefiner: boolean,
): ImageRefinementEvidence {
  const nodes = new Map((sample.nodes ?? []).map((node) => [node.node_id, node]));
  const terminal = terminalCandidates(sample);
  const candidates = candidateIds.map((candidateId): CandidateRefinementEvidence => {
    const matches = terminal.filter((candidate) => candidate.outcome.id === candidateId);
    const lineageId = matches.length === 1 ? matches[0].lineage_id : null;
    const terminalStages = lineageId
      ? (sample.projection?.debug_stages ?? []).filter((stage) => stage.lineage_id === lineageId)
      : [];
    // Mask projection preserves the prompt item id (`box-prompt:<detection id>`),
    // while DetectionSet projection uses the underlying detection id. Associate
    // only that exact prompt-derived lineage; a mask from another candidate is
    // still not execution evidence for this terminal result.
    const maskLineage = lineageId ? promptedMaskLineage(lineageId) : null;
    const mask = lineageId
      ? (sample.projection?.debug_stages ?? []).find((stage) =>
          stage.stage === "mask"
          && (stage.lineage_id === lineageId || stage.lineage_id === maskLineage),
        )
      : undefined;
    const refined = terminalStages.find((stage) => stage.stage === "refined" && maskToBbox(stage));
    const segmentationNode = mask ? nodes.get(mask.node_id) : undefined;
    const conversionNode = refined ? nodes.get(refined.node_id) : undefined;
    const executed = Boolean(
      lineageId && mask?.artifact_id && mask.artifact_ref && refined?.artifact_id && refined.artifact_ref
      && segmentationNode && succeeded(segmentationNode.status)
      && conversionNode && succeeded(conversionNode.status),
    );
    const items: RefinementEvidenceItem[] = [];
    const coverage = [...terminalStages].reverse().find((stage) => stage.stage === "prompt_coverage");
    if (coverage) items.push({kind:"gate",label:"Coverage Gate",status:null,...ref(coverage)});
    if (segmentationNode) items.push({
      kind:"node_receipt", label:"提示分割调用回执", node_id:segmentationNode.node_id,
      status:segmentationNode.status, artifact_id:null, artifact_ref:null,
      detail:segmentationNode.issues?.[0]?.message || null,
    });
    if (mask) items.push({kind:"artifact",label:"Mask Artifact",status:null,...ref(mask)});
    if (conversionNode) items.push({
      kind:"node_receipt", label:"mask_to_bbox 回执", node_id:conversionNode.node_id,
      status:conversionNode.status, artifact_id:null, artifact_ref:null,
      detail:conversionNode.issues?.[0]?.message || null,
    });
    if (refined) items.push({kind:"artifact",label:"mask_to_bbox Artifact",status:null,...ref(refined)});

    let reason = "提示分割调用、Mask Artifact 与 mask_to_bbox Artifact 已在该终端候选的 detection/prompt lineage 中核实。";
    if (!executed) {
      if (!lineageId) reason = "终端候选缺少唯一、可验证的 lineage；按 VLM-only 显示。";
      else if (segmentationNode && !succeeded(segmentationNode.status)) reason = `提示分割节点回执为 ${segmentationNode.status}，未证明执行成功。`;
      else if (segmentationNode && !mask) reason = "提示分割节点存在回执，但该终端 lineage 没有 Mask Artifact。";
      else if (mask && !refined) reason = "已生成 Mask Artifact，但该终端 lineage 没有 mask_to_bbox Artifact。";
      else if (refined && conversionNode && !succeeded(conversionNode.status)) reason = `mask_to_bbox 节点回执为 ${conversionNode.status}，未证明转换成功。`;
      else reason = stoppedReason(terminalStages, configuredRefiner);
    }
    return {
      candidate_id:candidateId, lineage_id:lineageId,
      source:executed ? "prompted_segmentation_refined" : "vlm_only",
      executed, reason, items,
    };
  });
  return {configured_refiner:configuredRefiner,candidates};
}
