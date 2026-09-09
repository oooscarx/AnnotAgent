import { expect,it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { BundleInstaller,type BundleInstallerService } from "./BundleInstaller";
import source from "./BundleInstaller.tsx?raw";
it("does not download, accept licenses or invent progress during render",()=>{
  const service=new Proxy({}, {get(){throw new Error("unexpected render operation");}}) as BundleInstallerService;
  const html=renderToStaticMarkup(<BundleInstaller service={service} pluginId="p" version="1"/>);
  expect(html).toContain("读取兼容目录");expect(html).not.toContain("已完成");
});
it("keeps explicit scope and failed-response protections independent of legacy pages",()=>{
  expect(source).toContain("!accepted||pending.current||uncertain");
  expect(source).toContain("selected.license_summary.license_digest");
  expect(source).toContain("JSON.stringify(same)!==JSON.stringify(selected)");
  expect(source).toContain("verifyInstallCommand(request,operation)");
  expect(source).not.toContain("../App");
});
