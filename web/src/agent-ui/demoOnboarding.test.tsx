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
  id: "tabletop-cup-bottle",
  version: "1.0.0",
  catalog_digest: "digest",
  title: "桌面杯子与瓶子",
  description: "检查六张图片中的杯子和瓶子。",
  image_count: 6,
  labels: ["cup", "bottle"],
  delivery_format: "yolo_detection",
  thumbnail_url: "/api/demo-packs/tabletop-cup-bottle/1.0.0/thumbnail/01",
  thumbnail_alt: "桌面上的杯子和瓶子",
  license_summary: "原创合成示例，可随项目分发",
  modes: [
    { mode: "preset_candidates", status: "ready", reason: null, model_name: null, provider_name: null, destination: "本地工作区", maximum_model_calls: 0, maximum_cost: "0", currency: "USD" },
    { mode: "live_model", status: "setup_required", reason: "需要视觉模型", model_name: null, provider_name: null, destination: "由所选 Provider 决定", maximum_model_calls: 6, maximum_cost: null, currency: null },
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
  const html = renderToStaticMarkup(<DemoCard item={entry} busy={false} onStart={() => {}}/>);
  expect(html).toContain("免配置体验（预置候选）");
  expect(html).toContain("预置候选，无本次模型推理");
  expect(html).toContain("本次不调用模型，不产生模型 Token");
  expect(html).toContain("配置模型并试跑");
  expect(html).not.toContain("正在识别");
});

it("shows no more than two server catalog entries", () => {
  const catalog = { contract_version: "demo-catalog-v1" as const, items: [entry, {...entry, id:"two"}, {...entry, id:"three"}] };
  expect(visibleDemoEntries(catalog).map((item) => item.id)).toEqual(["tabletop-cup-bottle", "two"]);
});

it("reuses the exact pending command and changes it only for a changed explicit scope", () => {
  const storage = memoryStorage();
  const input = { demo_id: entry.id, demo_version: entry.version, catalog_digest: entry.catalog_digest, mode: "preset_candidates" as const, confirmed_scope: true as const };
  const first = beginDemoStart(storage, "workspace-one", input, "command-one");
  expect(beginDemoStart(storage, "workspace-one", input, "command-two").command_id).toBe("command-one");
  expect(beginDemoStart(storage, "workspace-one", {...input, mode:"live_model"}, "command-two").command_id).toBe("command-two");
  expect(readPendingDemo(storage, "workspace-one")?.mode).toBe("live_model");
  expect(readPendingDemo(storage, "workspace-two")).toBeNull();
  expect(first.state).toBe("pending");
});

it("rejects a receipt that could navigate to another Demo task", () => {
  const input = { command_id:"command", demo_id:entry.id, demo_version:entry.version, catalog_digest:entry.catalog_digest, mode:"preset_candidates" as const, confirmed_scope:true as const };
  const receipt = { contract_version:"demo-start-v1" as const, command_id:"other", demo_id:entry.id, demo_version:entry.version, mode:"preset_candidates" as const, status:"ready" as const, project_id:"project",conversation_id:"conversation",task_id:"task",replayed:false,retry_safe:false,detail:null };
  expect(() => validateDemoReceipt(input, receipt)).toThrow("不匹配");
});
