import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CapabilitySetupCard } from "./CapabilitySetupCard";
import type { MainlineCapabilitySetupRequest } from "./modelPreparation";

const request: MainlineCapabilitySetupRequest = {
  id: "setup-id",
  project_id: "project",
  task_id: "task",
  task_revision: "task-revision",
  registry_revision: "registry-revision",
  role: "task_planning",
  required_capabilities: ["text_generation"],
  compatible_model_ids: ["model-profile:one"],
  status: "required",
  return_path: "/projects/project/work?task=task",
};

describe("CapabilitySetupCard", () => {
  it("shows one concrete blocker without technical identities or fake readiness", () => {
    const html = renderToStaticMarkup(
      <CapabilitySetupCard request={request} onOpen={() => undefined} />,
    );
    expect(html).toContain("继续前需要连接模型");
    expect(html).toContain("生成并整理标注方案");
    expect(html).toContain("真实 Registry 状态重新检查");
    expect(html).not.toContain("model-profile:one");
    expect(html).not.toContain("task-revision");
    expect(html.match(/<button/g)).toHaveLength(1);
  });

  it("does not keep a setup blocker mounted after the server marks it ready", () => {
    const html = renderToStaticMarkup(
      <CapabilitySetupCard request={{ ...request, status: "ready" }} onOpen={() => undefined} />,
    );
    expect(html).toBe("");
  });
});
