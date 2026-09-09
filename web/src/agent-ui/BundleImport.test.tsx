import {it,expect} from "vitest";
import {renderToStaticMarkup} from "react-dom/server";
import {BundleImport,type BundleImportService} from "./BundleImport";
import source from "./BundleImport.tsx?raw";
it("keeps local package selection separate from upload and import",()=>{
  const service=new Proxy({}, {get(){throw new Error("render mutation");}}) as BundleImportService;
  const html=renderToStaticMarkup(<BundleImport service={service} onImported={async()=>{throw new Error("render mutation");}}/>);
  expect(html).toContain("选择模型包");expect(html).not.toContain("已导入");
});
it("binds import to inspected identity and explicitly guards unsaved files",()=>{
  expect(source).toContain("result.bundle.bundle_sha256!==inspection!.bundle_sha256");
  expect(source).toContain("!inspection||!accepted||unknown");expect(source).toContain("ui-preview:before-navigate");expect(source).toContain("beforeunload");expect(source).not.toContain("../App");
});
