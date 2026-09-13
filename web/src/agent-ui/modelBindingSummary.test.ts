import { describe, expect, it } from "vitest";
import type { Task } from "./adapter";
import { taskModelBindingSummary } from "./modelBindingSummary";
import { renderToStaticMarkup } from "react-dom/server";
import { TaskModelBindings } from "./TaskModelBindings";

const task = (overrides: Partial<Task>): Task => ({
  id: "task", project: "project", title: "Ball", phase: "idle", revision: "schema",
  items: [], queue: [], draft: "", model: "model-profile:glm-planner", boxes: [], image: "image",
  ...overrides,
});
describe("task model binding summary", () => {
  it("separates the planner from visual and prompted-segmentation Draft bindings", () => {
    const result = taskModelBindingSummary(task({
      plan: {
        revision: "draft-2",
        steps: ["open_vocabulary_detection · Qwen VLM", "prompted_segmentation · EfficientSAM", "mask_to_bbox"],
        models: ["Qwen VLM", "EfficientSAM"],
        images: 3,
        destination: "saved Draft",
        budget: null,
      },
      approval: { id: "approval", title: "Sample", revision: "scope", budget: null, scope: ["model-profile:qwen · digest-qwen", "model-instance:efficientsam · digest-sam"] },
    }));
    expect(result).toMatchObject({
      planningModel: "model-profile:glm-planner",
      workflowModels: ["Qwen VLM", "EfficientSAM"],
      refiner: { state: "in_draft" },
    });
    expect(result.authorizationBindings).toHaveLength(2);
    const html=renderToStaticMarkup(TaskModelBindings({summary:result}));
    expect(html).toContain("方案包含精修");
    expect(html).not.toContain("已进入 Draft");
    expect(html).not.toContain("提示分割已执行");
  });

  it("does not claim an authorized EfficientSAM is part of the Draft", () => {
    const result = taskModelBindingSummary(task({
      plan: { revision: "draft-1", steps: ["detection · Qwen VLM"], models: ["Qwen VLM"], images: 3, destination: "saved Draft", budget: null },
      approval: { id: "approval", title: "Sample", revision: "scope", budget: null, scope: ["EfficientSAM → local runtime"] },
    }));
    expect(result.refiner.state).toBe("authorized_only");
    expect(result.refiner.detail).toContain("尚无证据");
  });

  it("reports no refiner when neither the Draft nor authorization contains one", () => {
    expect(taskModelBindingSummary(task({})).refiner.state).toBe("absent");
  });
});
