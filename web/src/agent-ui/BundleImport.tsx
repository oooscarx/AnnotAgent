import { useEffect, useRef, useState } from "react";
import type { api } from "../api";
import type { VerifiedModelBundlePackage } from "../types";
import { Dialog } from "./Dialog";
import { Disclosure } from "./Disclosure";
export type BundleImportService=Pick<typeof api,"inspectModelBundlePackage"|"importModelBundlePackage">;
export function BundleImport({service,onImported}:{service:BundleImportService;onImported:()=>Promise<void>}) {
  const [file,setFile]=useState<File>();const [inspection,setInspection]=useState<VerifiedModelBundlePackage>();const [accepted,setAccepted]=useState(false);const [action,setAction]=useState<"inspect"|"import">();
  const [busy,setBusy]=useState(false);const [error,setError]=useState("");const [message,setMessage]=useState("");const [unknown,setUnknown]=useState(false);const pending=useRef(false);const alive=useRef(true);const picker=useRef<HTMLInputElement>(null);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  useEffect(()=>{const dirty=!!file||busy;const guard=(e:Event)=>{if(dirty&&!window.confirm("本地文件选择或导入结果尚未处理完，仍要离开？刷新后未上传的文件需要重新选择。"))e.preventDefault();};const unload=(e:BeforeUnloadEvent)=>{if(dirty){e.preventDefault();e.returnValue="";}};window.addEventListener("ui-preview:before-navigate",guard);window.addEventListener("beforeunload",unload);return()=>{window.removeEventListener("ui-preview:before-navigate",guard);window.removeEventListener("beforeunload",unload);};},[file,busy]);
  const execute=async()=>{
    if(!file||!action||pending.current||(action==="import"&&(!inspection||!accepted||unknown)))return;
    const chosen=file;const kind=action;pending.current=true;setBusy(true);setError("");setMessage("");
    try{
      if(kind==="inspect"){
        const value=await service.inspectModelBundlePackage(chosen);if(!value.verified||value.installed!==false)throw new Error("服务器未返回有效的未安装包检查报告");
        if(alive.current){setInspection(value);setAccepted(false);setMessage("包检查完成，尚未导入或安装。");}
      }else{
        setUnknown(true);
        const result=await service.importModelBundlePackage(chosen,true);
        if(result.bundle.bundle_sha256!==inspection!.bundle_sha256||result.bundle.manifest.id!==inspection!.manifest.id||result.bundle.manifest.version!==inspection!.manifest.version)throw new Error("导入回执与已检查的包不匹配");
        if(alive.current){setUnknown(false);setFile(undefined);setInspection(undefined);setAccepted(false);if(picker.current)picker.current.value="";setMessage(`服务器已导入模型包；返回 ${result.model_instances.length} 个模型实例。实际 Ready 状态请查看模型实例列表。`);await onImported();}
      }
      if(alive.current)setAction(undefined);
    }catch(e){if(alive.current){setAction(undefined);setError(`${(e as Error).message}。未自动重试。${kind==="import"?"请重新读取已安装模型包和实例，核实服务器状态。":"文件仍保留，可更换后检查。"}`);}}
    finally{pending.current=false;if(alive.current)setBusy(false);}
  };
  return <section aria-label="导入本地模型包"><h2>导入本地 Model Bundle</h2>
    <p>选择打包好的 Model Bundle。检查会上传到当前服务器，不会安装；导入需另行接受许可证，可能启动已安装插件进行兼容性检查。</p>
    {error&&<p role="alert">{error}</p>}{message&&<p role="status">{message}</p>}{unknown&&<p role="alert">导入结果未核实，本次文件不能直接再次导入。</p>}
    <label className="plugin-package-picker">选择模型包<input ref={picker} type="file" disabled={busy||unknown} onChange={e=>{setFile(e.target.files?.[0]);setInspection(undefined);setAccepted(false);setError("");setMessage("");}}/></label>
    {file&&<div className="actions"><span>{file.name} · {file.size} bytes · {inspection?"已上传检查，尚未确认导入":"未上传"}</span><button disabled={busy||unknown} onClick={()=>setAction("inspect")}>检查模型包…</button><button disabled={busy||unknown} onClick={()=>{setFile(undefined);setInspection(undefined);setAccepted(false);if(picker.current)picker.current.value="";}}>取消选择</button></div>}
    {inspection&&<div><h3>{inspection.manifest.display_name} · {inspection.manifest.version}</h3><p>{inspection.manifest.fixture?"Fixture 测试包":"模型包"} · 签名：{inspection.signature}</p><p>包 SHA256：{inspection.bundle_sha256}</p><p>许可证：{inspection.manifest.license.name} · 商用：{inspection.manifest.license.commercial_use} · 再分发：{inspection.manifest.license.redistribution}</p><p>许可证摘要：{inspection.manifest.license.license_digest}</p>{inspection.manifest.license.usage_notes.map((note,i)=><p key={i}>{note}</p>)}<Disclosure title="模型来源、文件和兼容要求"><pre>{JSON.stringify({source:inspection.manifest.source,files:inspection.manifest.files,runtime:inspection.manifest.runtime,compatible_plugins:inspection.manifest.compatible_plugins},null,2)}</pre></Disclosure><label><input type="checkbox" disabled={busy||unknown} checked={accepted} onChange={e=>setAccepted(e.target.checked)}/>接受此模型包的许可证和使用限制</label><button disabled={busy||unknown||!accepted} onClick={()=>setAction("import")}>导入已检查的模型包…</button></div>}
    {action&&<Dialog title={action==="inspect"?"确认上传检查模型包":"确认导入模型包"} onClose={()=>{if(!busy)setAction(undefined);}}><p>{file?.name} · {file?.size} bytes</p><p>{action==="inspect"?"上传到当前 AnnotAgent 服务器，仅检查包内容。":"导入刚才检查的同一文件，保存模型包并发现兼容实例；不会自动标注项目图片。外部或计算费用未知。"}</p><div className="actions"><button disabled={busy} onClick={()=>setAction(undefined)}>取消</button><button disabled={busy} onClick={()=>void execute()}>{busy?"等待服务器结果…":"确认操作"}</button></div></Dialog>}
  </section>;
}
