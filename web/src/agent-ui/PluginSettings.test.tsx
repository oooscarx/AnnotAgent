import { it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { PluginSettings } from "./PluginSettings";
import type { PluginManagement } from "./pluginManagement";
import source from "./PluginSettings.tsx?raw";
import { HttpAdapter } from "./http";

it("rendering does not perform installation, testing, or other requests", () => {
  const service = new Proxy({}, {get() { throw new Error("render performed an operation"); }}) as PluginManagement;
  const html = renderToStaticMarkup(<PluginSettings service={service}/>);
  expect(html).toContain("读取插件、模型包与实例");
  expect(html).toContain("选择插件包");
  expect(html).not.toContain("已完成");
});
it("does not leak real mutation services into custom-transport adapter tests", () => {
  const adapter = new HttpAdapter(async () => { throw new Error("test transport"); });
  expect(adapter.pluginManagement).toBeUndefined();
});
it("retains explicit installation checks, confirmation and repeat protection", () => {
  expect(source).toContain("!inspection.web_installable");
  expect(source).toContain("!accepted");
  expect(source).toContain("pending.current");
  expect(source).toContain("ui-preview:before-navigate");
  expect(source).toContain("beforeunload");
  expect(source).not.toContain("../App");
  expect(source).not.toContain("fixture");
});
