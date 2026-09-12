import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { DemoCard } from "./DemoOnboarding";
import {
  beginDemoStart,
  readPendingDemo,
  validateDemoReceipt,
  visibleDemoEntries,
  type DemoCatalogEntry,
} from "./demoOnboardingService";

const entry: DemoCatalogEntry = {
  demo_id: "object-detection-review",
  version: "1.0.0",
  manifest_sha256: "manifest",
  title: "标注桌面物品",
  summary: "检查六张原创合成图片中的杯子和瓶子。",
  learning_objectives: ["审核候选"],
  image_count: 6,
  labels: ["cup", "bottle"],
  delivery_format: "yolo_detection",
  thumbnail_asset_id: "thumbnail",
  thumbnail_url: "/api/demo-catalog/object-detection-review/versions/1.0.0/assets/thumbnail",
  license: { spdx_id: "CC0-1.0", source_url: "https://example.invalid/license", attribution_asset_id: "attribution" },
  modes: [
    { source_mode: "preset_candidates", status: "ready", reason: null, required_capabilities: [] },
    { source_mode: "live_model", status: "setup_required", reason: "需要视觉模型", required_capabilities: ["vision_language"] },
  ],
};

function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() { return values.size; },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => { values.delete(key); },
    setItem: (key, value) => { values.set(key, value); },
  };
}

it("renders a truthful preset primary action and a separate model setup path", () => {
  const html = renderToStaticMarkup(<DemoCard item={entry} catalogRevision="catalog" liveModels={[]} selectedModel="" busy={false} onSelectModel={() => {}} onStart={() => {}}/>);
  expect(html).toContain("体验预置候选");
  expect(html).toContain("不调用模型 · 结果仍需逐图人工审核");
  expect(html).toContain("不产生模型 Token，不代表实时模型准确率");
  expect(html).toContain("连接视觉模型");
  expect(html).not.toContain("正在识别");
});

it("shows no more than two server catalog entries", () => {
  const catalog = { contract_version: "demo-catalog-v1" as const, catalog_revision:"catalog", next_cursor:null, items: [entry, {...entry, demo_id:"two"}, {...entry, demo_id:"three"}] };
  expect(visibleDemoEntries(catalog).map((item) => item.demo_id)).toEqual(["object-detection-review", "two"]);
});

it("reuses the exact pending command and changes it only for a changed explicit scope", () => {
  const storage = memoryStorage();
  const input = { demo_id: entry.demo_id, demo_version: entry.version, source_mode: "preset_candidates" as const, model_profile_id:null, catalog_revision:"catalog", manifest_sha256:entry.manifest_sha256 };
  const first = beginDemoStart(storage, "workspace-one", input, "command-one");
  expect(beginDemoStart(storage, "workspace-one", input, "command-two").command_id).toBe("command-one");
  expect(beginDemoStart(storage, "workspace-one", {...input, source_mode:"live_model",model_profile_id:"model"}, "command-two").command_id).toBe("command-two");
  expect(readPendingDemo(storage, "workspace-one")?.source_mode).toBe("live_model");
  expect(readPendingDemo(storage, "workspace-two")).toBeNull();
  expect(first.state).toBe("pending");
});

it("rejects a receipt that could navigate to another Demo task", () => {
  const input = { command_id:"command", demo_id:entry.demo_id, demo_version:entry.version, source_mode:"preset_candidates" as const, model_profile_id:null,catalog_revision:"catalog",manifest_sha256:entry.manifest_sha256,state:"pending" as const };
  const receipt = { contract_version:"demo-start-v1" as const, command_id:"other", demo_id:entry.demo_id, demo_version:entry.version, source_mode:"preset_candidates" as const,catalog_revision:"catalog",manifest_sha256:entry.manifest_sha256,status:"ready" as const,project_id:"project",project_owner_id:"owner",conversation_id:"conversation",task_id:"task",work_route:"/projects/project/work?conversation=conversation&task=task",source_provenance:{kind:"preset_candidates" as const,live_inference_occurred:false,review_status:"needs_review",source_asset_id:"preset",source_asset_sha256:"source"},replayed:false,retry_safe:false,detail:null };
  expect(() => validateDemoReceipt(input, receipt)).toThrow("不匹配");
});
