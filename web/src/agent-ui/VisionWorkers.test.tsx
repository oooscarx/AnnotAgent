import { expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { VisionWorkers, type VisionWorkerService } from "./VisionWorkers";
import { HttpAdapter } from "./http";
import source from "./VisionWorkers.tsx?raw";

it("does not call a worker during render or claim discovery is inference", () => {
  const service = new Proxy({}, { get() { throw new Error("render must not call APIs"); } }) as VisionWorkerService;
  const html = renderToStaticMarkup(<VisionWorkers service={service} />);
  expect(html).toContain("不执行样例推理");
  expect(html).toContain("读取模型绑定");
  expect(html).not.toContain("已完成");
});
it("isolates real worker operations from custom transports", () => {
  expect(new HttpAdapter(async () => { throw new Error("test"); }).visionWorkerManagement).toBeUndefined();
});
it("guards explicit discovery and never imports legacy UI", () => {
  expect(source).toContain("!accepted || pending.current");
  expect(source).toContain("JSON.stringify(latest) !== JSON.stringify(selected)");
  expect(source).toContain("result.model_id !== selected.id");
  expect(source).not.toContain("../App");
  expect(source).not.toContain("sampleTestModel");
});
