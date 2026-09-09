import type { WorkflowDryRunReport } from "../types";

type Sample = WorkflowDryRunReport["samples"][number];

/** Report actual route evidence, never infer quality from a successful transport. */
export function sampleRisks(sample: Pick<Sample, "nodes" | "projection">): string[] {
  const risks = sample.projection?.review_candidates.map(r => r.explanation.summary) ||
    (sample.projection ? [] : ["旧样例没有终端投影，未显示中间框"]);
  const coverage = (sample.nodes || []).filter(n => n.metadata?.selected_route === "review");
  const specific: string[] = [];
  if (coverage.some(n => n.metadata.failure_code === "prompt_coverage_check_missing" && n.metadata.prompt_count === 0)) {
    specific.push("局部搜索没有返回可用于精修的候选；这不代表图片没有目标，请检查是否漏标。");
  }
  if (coverage.some(n => n.metadata.failure_code === "recovery_budget_exhausted")) {
    specific.push("局部定位与原定位的覆盖证据仍不一致，本轮修复次数已用完；当前框仍需人工核查。");
  }
  if (specific.length && !sample.projection?.debug_stages?.some(s => s.stage === "mask")) {
    specific.push("本图没有分割 Mask 输出，不能视为已通过精修。");
  }
  return [...specific, ...risks];
}
