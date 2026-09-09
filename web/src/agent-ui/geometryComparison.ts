import type {WorkflowDryRunReport} from "../types";
import {terminalSampleAnnotations} from "../sampleAnnotations";
export type GeometrySample=WorkflowDryRunReport["samples"][number];
type Sample=GeometrySample;
export function comparisonBoxes(sample: Sample) {
  const seen = new Set<string>();
  const results: { group: string; label: string; rect: number[]; detail?: string | null }[] = [];
  for (const stage of sample.projection?.debug_stages ?? []) {
    if (!["coarse", "relocalized", "refined"].includes(stage.stage) || stage.value?.kind !== "bounding_box") continue;
    // Some older debug projections call crop-local coordinates "coarse" too.
    // Never overlay these on the root image; use the explicit projected stage.
    if (stage.stage === "coarse") {
      const trace = sample.nodes?.find((node) => node.node_id === stage.node_id)?.metadata?.model_input_trace as { source_region_pixels?: number[] } | undefined;
      if (JSON.stringify(trace?.source_region_pixels) !== JSON.stringify([0, 0, sample.width, sample.height])) continue;
    }
    const key = JSON.stringify([stage.stage, stage.label, stage.value.rect]);
    if (seen.has(key)) continue;
    seen.add(key);
    results.push({ group: stage.stage, label: stage.label ?? "", rect: stage.value.rect, detail: stage.detail });
  }
  for (const outcome of terminalSampleAnnotations(sample,"diagnostic-image","diagnostic-test")) {
    if (outcome.value?.kind === "bounding_box") results.push({ group: "retained", label: outcome.label ?? "", rect: outcome.value.rect, detail: outcome.review_status });
  }
  return results;
}
