import { describe, expect, it } from "vitest";
import { builderPlanSource, builderStopLabel, recoveryNodeIds } from "./pipelinePresentation";
import { translate } from "./i18n";
import type { WorkflowDraftNode, WorkflowEdge } from "./types";

function node(id: string, ports: string[] = []): WorkflowDraftNode {
  return { id, node_type: "test", depends_on: [], validators: [], refiners: [], max_retries: 0, review_gate: false, parameters: {}, inputs: ports.map(id => ({id, required: true, multiple: true, artifact_type: "detection_set"})) };
}
function edge(from: string, to: string, port: string, route?: string): WorkflowEdge {
  return { from_node: from, from_port: "output", to_node: to, to_port: port, route };
}
describe("Pipeline presentation", () => {
  it("folds recovery descendants but retains the shared SAM and review merge", () => {
    const nodes = [node("image"), node("gate"), node("expand", ["image", "boxes"]), node("crop", ["image", "boxes"]), node("retry", ["image"]), node("sam", ["image", "prompts"]), node("review", ["boxes"])];
    const edges = [edge("image", "expand", "image"), edge("gate", "expand", "boxes", "relocalize"), edge("image", "crop", "image"), edge("expand", "crop", "boxes"), edge("crop", "retry", "image"), edge("image", "sam", "image"), edge("retry", "sam", "prompts"), edge("gate", "sam", "prompts", "refine"), edge("gate", "review", "boxes", "review"), edge("retry", "review", "boxes")];
    expect([...recoveryNodeIds({ nodes, edges })].sort()).toEqual(["crop", "expand", "retry"]);
  });
  it("does not infer recovery from similar names or repeated model bindings", () => {
    expect(recoveryNodeIds({ nodes: [node("recovery_model"), node("coarse_model")], edges: [] }).size).toBe(0);
  });
  it("translates legacy stop reasons without claiming sample accuracy", () => {
    const label = builderStopLabel({ stop_reason: "DiscoveryLimitTriggeredSalvage" });
    expect(translate(label, "zh-CN")).toBe("探索已收尾，已保存可用方案");
    expect(builderStopLabel({ builder_stop_reason: "runnable_candidate_triggered_salvage" })).toBe("Compatible plan found");
    expect(translate("Draft ready for human review", "zh-CN")).toBe("草稿已保存，等待人工检查");
    expect(builderPlanSource({})).toBe("Source not recorded");
  });
});
