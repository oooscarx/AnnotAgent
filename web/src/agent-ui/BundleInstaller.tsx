import { useEffect, useRef, useState } from "react";
import type { api } from "../api";
import type { ModelCatalogEntry, ModelInstallOperation } from "../types";
import { Disclosure } from "./Disclosure";
import { Dialog } from "./Dialog";
import { bundleInstallKey, preserveBundleInstall } from "./bundleInstallRecovery";
export type BundleInstallerService = Pick<typeof api,"compatibleModelBundles"|"modelInstallOperations"|"acceptModelBundleLicense"|"startModelInstallOperation">;
export function BundleInstaller({service,pluginId,version,workspaceId}:{service:BundleInstallerService;pluginId:string;version:string;workspaceId?:string}) {
  const recoveryKey=workspaceId?bundleInstallKey(workspaceId,pluginId,version):undefined;
  const [catalog,setCatalog]=useState<Awaited<ReturnType<BundleInstallerService["compatibleModelBundles"]>>>();
  const [operations,setOperations]=useState<ModelInstallOperation[]>([]);const [error,setError]=useState("");const [selection,setSelection]=useState<ModelCatalogEntry>();const [accepted,setAccepted]=useState(false);const [busy,setBusy]=useState(false);const [reload,setReload]=useState(0);const [uncertain,setUncertain]=useState(false);
  const pending=useRef(false);const alive=useRef(true);
  useEffect(()=>{const restore=()=>{try{setUncertain(!recoveryKey||localStorage.getItem(recoveryKey)!==null);}catch{setUncertain(true);setError("无法读取安装恢复记录，不能安全启动安装。");}};restore();window.addEventListener("storage",restore);return()=>window.removeEventListener("storage",restore);},[recoveryKey]);
  useEffect(()=>{alive.current=true;let current=true;let timer:ReturnType<typeof setTimeout>|undefined;
    const read=async()=>{try{const [compatible,records]=await Promise.all([service.compatibleModelBundles(pluginId,version),service.modelInstallOperations()]);if(!current)return;setCatalog(compatible);const owned=records.operations.filter(o=>o.plugin_id===pluginId&&o.plugin_version===version);setOperations(owned);if(owned.some(o=>o.status==="running"))timer=setTimeout(()=>void read(),1500);}catch(e){if(current)setError((e as Error).message);}};
    void read();return()=>{current=false;alive.current=false;clearTimeout(timer);};
  },[service,pluginId,version,reload]);
  const install=async()=>{
    if(!selection?.catalog_id||!accepted||pending.current||uncertain||!recoveryKey)return;
    pending.current=true;setBusy(true);setError("");const selected=selection;
    try{
      const fresh=await service.compatibleModelBundles(pluginId,version);
      const same=fresh.available.find(e=>e.catalog_id===selected.catalog_id&&e.bundle_id===selected.bundle_id&&e.bundle_version===selected.bundle_version);
      if(JSON.stringify(same)!==JSON.stringify(selected))throw new Error("目录、来源或许可证已变化，请重新选择。");
      if(fresh.setup_blockers.some(b=>b.bundle_id===selected.bundle_id&&b.bundle_version===selected.bundle_version))throw new Error("服务器报告安装阻塞，请重新读取目录。");
      const active=(await service.modelInstallOperations()).operations.find(o=>o.plugin_id===pluginId&&o.plugin_version===version&&o.status==="running");
      if(active)throw new Error("此插件已有进行中的安装，请查看执行记录。");
      if(selected.license_summary.requires_acceptance)await service.acceptModelBundleLicense(selected.bundle_id,selected.bundle_version,selected.license_summary.license_digest);
      // A lost POST response is not a safe signal to initiate another download.
      const request={catalog_id:selected.catalog_id!,bundle_id:selected.bundle_id,bundle_version:selected.bundle_version,plugin_id:pluginId,plugin_version:version};
      preserveBundleInstall(localStorage,recoveryKey,request);setUncertain(true);
      const operation=await service.startModelInstallOperation(request);
      if(operation.plugin_id!==pluginId||operation.plugin_version!==version||operation.bundle_id!==selected.bundle_id||operation.bundle_version!==selected.bundle_version)throw new Error("安装回执归属不匹配");
      localStorage.removeItem(recoveryKey);
      if(alive.current){setUncertain(false);setSelection(undefined);setReload(n=>n+1);}
    }catch(e){if(alive.current){setError(`${(e as Error).message}。没有自动重新安装；请查看服务器安装记录。`);setSelection(undefined);setReload(n=>n+1);}}
    finally{pending.current=false;if(alive.current)setBusy(false);}
  };
  return <section aria-label="兼容模型安装">
    <p>从服务器目录安装此插件的兼容模型包。下载、磁盘占用、启动插件及样例推理均需本次明确确认。</p>
    {!workspaceId&&<p role="alert">缺少服务器 workspace 身份，安装不可用。</p>}
    {error&&<p role="alert">{error}</p>}{uncertain&&<p role="alert">安装结果尚未核实。本页禁止再次启动安装，请先查看以下记录；刷新不会自动执行安装。</p>}
    <button disabled={busy} onClick={()=>setReload(n=>n+1)}>读取目录与安装记录</button>
    {!catalog?<p role="status">读取兼容目录…</p>:<><p>插件运行状态：{catalog.plugin_runtime_status}</p>{!catalog.available.length&&<p>服务器未提供兼容目录项。没有伪造可安装模型。</p>}{catalog.available.map(entry=>{
      const blockers=catalog.setup_blockers.filter(b=>b.bundle_id===entry.bundle_id&&b.bundle_version===entry.bundle_version);
      return <article className="settings-row" key={`${entry.catalog_id}:${entry.bundle_id}@${entry.bundle_version}`}><div><strong>{entry.display_name}</strong><p>{entry.bundle_version} · {entry.fixture?"Fixture 测试包":"模型目录包"} · {entry.bundle_size_bytes} bytes</p><p>{entry.description}</p>{blockers.map(b=><p key={b.code}>{b.message}</p>)}</div><button disabled={busy||uncertain||!entry.catalog_id||blockers.length>0||operations.some(o=>o.status==="running")} onClick={()=>{setAccepted(false);setSelection(entry);}}>检查并安装…</button></article>;
    })}</>}
    <Disclosure title="服务器安装记录" open={operations.some(o=>o.status==="running")||uncertain}>
      {!operations.length&&<p>暂无服务器安装记录。没有记录不证明丢失响应的请求从未执行。</p>}
      {operations.map(operation=><article key={operation.id}><strong>{operation.bundle_id}@{operation.bundle_version} · {operation.status}</strong><p role={operation.status==="running"?"status":undefined}>{operation.stage} · {operation.detail}</p><p>已接收 {operation.bytes_completed} / {operation.bytes_total??"未知"} bytes</p>{operation.error&&<p role="alert">{operation.error}</p>}{operation.suggested_action&&<p>{operation.suggested_action}</p>}<p>模型实例：{operation.model_instance_ids.join(" · ")||"尚无回执"}</p><small>{operation.id} · {operation.updated_at}</small></article>)}
    </Disclosure>
    {selection&&<Dialog title="确认安装兼容模型" onClose={()=>{if(!busy)setSelection(undefined);}}>
      <p>{selection.display_name} · {selection.bundle_version}</p><p>来源：{selection.bundle_url}</p><p>发布者：{selection.publisher.display_name} · {selection.publisher.verified?"已验证":"未验证"}</p><p>包 SHA256：{selection.bundle_sha256}</p><p>下载 {selection.bundle_size_bytes} bytes · 安装空间 {selection.installed_size_bytes??"未知"} bytes</p>
      <p>许可证：{selection.license_summary.name} · 商用：{selection.license_summary.commercial_use} · 再分发：{selection.license_summary.redistribution}</p><p>许可证摘要：{selection.license_summary.license_digest}</p>
      <Disclosure title="平台与执行要求"><pre>{JSON.stringify(selection.platform_requirements,null,2)}</pre></Disclosure>
      <p>将下载模型并启动已安装插件、执行服务端测试样例。不使用当前项目图片。外部网络与计算费用未知，不代表免费。</p>
      <label><input type="checkbox" disabled={busy} checked={accepted} onChange={e=>setAccepted(e.target.checked)}/>接受上述许可证、来源和本次安装执行范围</label>
      <div className="actions"><button disabled={busy} onClick={()=>setSelection(undefined)}>取消</button><button disabled={busy||!accepted||uncertain} onClick={()=>void install()}>{busy?"等待安装回执…":"确认下载并安装"}</button></div>
    </Dialog>}
  </section>;
}
