import { describe, expect, it } from "vitest";
import type { WorkflowDryRunReport } from "../types";
import { sampleRisks } from "./sampleRisks";

const projection = {final_candidates:[],review_candidates:[],committed_annotations:[],no_target:false,intermediate_artifact_ids:[],debug_stages:[]};
const node = (metadata: Record<string, unknown>): WorkflowDryRunReport["samples"][number]["nodes"][number] => ({node_id:"coverage",status:"succeeded",metadata,output_types:[],latency_ms:0,estimated_cost:"0",issues:[]});
describe("sample repair risk evidence", () => {
  it("does not call an empty local search a no-target success", () => {
    const risks = sampleRisks({projection,nodes:[node({selected_route:"review",failure_code:"prompt_coverage_check_missing",prompt_count:0})]}).join(" ");
    expect(risks).toContain("不代表图片没有目标");
    expect(risks).toContain("没有分割 Mask");
  });
  it("exposes bounded recovery exhaustion without claiming quality", () => {
    expect(sampleRisks({projection,nodes:[node({selected_route:"review",failure_code:"recovery_budget_exhausted"})]}).join(" ")).toContain("当前框仍需人工核查");
  });
  it("does not claim no segmentation when another route produced a mask", () => {
    const mask = {artifact_id:"mask",artifact_ref:"mask:1",node_id:"segment",lineage_id:"1",stage:"mask" as const,source:"segmenter",terminal:false};
    expect(sampleRisks({projection:{...projection,debug_stages:[mask]},nodes:[node({selected_route:"review",failure_code:"recovery_budget_exhausted"})]}).join(" ")).not.toContain("没有分割 Mask");
  });
  it("preserves ordinary review explanations without inventing repair", () => {
    expect(sampleRisks({projection,nodes:[]})).toEqual([]);
    expect(sampleRisks({nodes:[]})).toEqual(["旧样例没有终端投影，未显示中间框"]);
  });
});
