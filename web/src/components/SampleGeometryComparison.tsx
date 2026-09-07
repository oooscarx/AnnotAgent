import { useState } from "react";
import type { WorkflowDryRunReport } from "../types";
import { t } from "../i18n";

type Sample = WorkflowDryRunReport["samples"][number];
const groups = [
  { id: "coarse", title: "Whole-image VLM", color: "#2563eb", dash: "8 4" },
  { id: "relocalized", title: "Local VLM", color: "#7c3aed", dash: "3 3" },
  { id: "refined", title: "SAM / refiner", color: "#c2410c", dash: "" },
  { id: "retained", title: "Retained for review", color: "#0f766e", dash: "12 3 2 3" },
];

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
  if (sample.projection) for (const outcome of sample.outcomes) {
    if (outcome.value?.kind === "bounding_box") results.push({ group: "retained", label: outcome.label, rect: outcome.value.rect, detail: outcome.status });
  }
  return results;
}

export function SampleGeometryComparison({ sample, imageUrl }: { sample: Sample; imageUrl?: string }) {
  const [visible, setVisible] = useState(groups.map((group) => group.id));
  const boxes = comparisonBoxes(sample);
  return <section aria-label={t("Compare all geometry")}>
    <p role="status">{t("Diagnostic comparison only. Refiner boxes may have been rejected; showing them does not accept annotations.")}</p>
    <div className="button-row" style={{ marginBlock: "1rem" }}>
      {groups.map((group) => <label key={group.id} style={{ display: "inline-flex", alignItems: "center", gap: ".5rem", color: group.color }}>
        <input type="checkbox" checked={visible.includes(group.id)} onChange={(event) => setVisible((current) => event.target.checked ? [...current, group.id] : current.filter((id) => id !== group.id))} />
        {t(group.title)} ({boxes.filter((box) => box.group === group.id).length})
      </label>)}
    </div>
    {imageUrl ? <svg viewBox={`0 0 ${sample.width} ${sample.height}`} style={{ width: "100%", display: "block" }} role="img" aria-label={t("All geometry over the original image; numbered details below")}>
      <image href={imageUrl} width={sample.width} height={sample.height} />
      {boxes.map((box, index) => {
        if (!visible.includes(box.group)) return null;
        const group = groups.find((item) => item.id === box.group)!;
        const [x, y, w, h] = box.rect.map((value, i) => value * (i % 2 ? sample.height : sample.width));
        return <g key={index}><title>{`${index + 1}. ${t(group.title)} · ${box.label} · ${box.detail ?? ""}`}</title>
          <rect x={x} y={y} width={w} height={h} fill="none" stroke={group.color} strokeWidth="1.5" vectorEffect="non-scaling-stroke" strokeDasharray={group.dash} />
          <text x={x} y={Math.max(12, y - 4)} fill={group.color} fontSize="12" stroke="white" strokeWidth="2" paintOrder="stroke">{index + 1}</text>
        </g>;
      })}
    </svg> : <p>{t("Preview unavailable")}</p>}
    <ol style={{ display: "grid", gap: ".5rem", marginBlock: "1rem" }}>{boxes.map((box, index) => visible.includes(box.group) && <li key={index} value={index + 1}>
      <strong>{t(groups.find((group) => group.id === box.group)!.title)}</strong>{" · "}{box.label}{" · "}
      {box.rect.map((value, i) => Math.round(value * (i % 2 ? sample.height : sample.width))).join(", ")} px (x, y, w, h){box.detail ? ` · ${box.detail}` : ""}
    </li>)}</ol>
    <p>{t("Mask artifacts remain in execution evidence; this comparison displays bounding boxes, not mask pixels.")}</p>
  </section>;
}
