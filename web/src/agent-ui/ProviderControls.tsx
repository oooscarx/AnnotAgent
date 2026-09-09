import { useEffect, useRef, useState } from "react";
import type { api } from "../api";
import type { ProviderProfile } from "../types";
import { Dialog } from "./Dialog";
import { Disclosure } from "./Disclosure";

export type ProviderControlService=Pick<typeof api,"providers"|"providerPresets"|"saveProviderCredential"|"discoverProviderModels"|"updateProvider"|"deleteProviderCredential">;
export function ProviderControls({providerId,service,onChanged}:{providerId:string;service:ProviderControlService;onChanged:()=>Promise<void>}) {
  const [provider,setProvider]=useState<ProviderProfile>();
  const [error,setError]=useState("");
  const [message,setMessage]=useState("");
  const [busy,setBusy]=useState(false);
  const [action,setAction]=useState<"discover"|"toggle"|"remove">();
  const [models,setModels]=useState<Awaited<ReturnType<ProviderControlService["discoverProviderModels"]>>>();
  const [reload,setReload]=useState(0);
  const locked=useRef(false);
  const generation=useRef(0);
  useEffect(()=>{
    const current=++generation.current;setProvider(undefined);setModels(undefined);setError("");setAction(undefined);
    void service.providers().then(r=>{if(current!==generation.current)return;const value=r.providers.find(p=>p.id===providerId);if(!value)throw new Error("Provider 不存在或已删除");setProvider(value);}).catch(e=>{if(current===generation.current)setError(e.message);});
    return()=>{generation.current++;};
  },[providerId,service,reload]);
  const execute=async()=>{
    if(locked.current||!provider||!action)return;
    locked.current=true;setBusy(true);setError("");setMessage("");const current=generation.current;
    try{
      const latest=(await service.providers()).providers.find(p=>p.id===providerId);
      if(!latest||JSON.stringify(latest)!==JSON.stringify(provider))throw new Error("连接配置已变化，请刷新并重新确认。没有执行本次操作。");
      if(action==="discover"){
        const result=await service.discoverProviderModels(providerId);
        if(result.provider_id!==providerId)throw new Error("模型发现回执 Provider 不匹配");
        if(current===generation.current){setModels(result);setMessage(`发现 ${result.models.length} 个远端模型 ID；未创建模型配置，也未证明其推理可用。`);}
      } else if(action==="toggle") {
        const result=await service.updateProvider(providerId,{enabled:!provider.enabled});
        if(result.id!==providerId||result.enabled===provider.enabled)throw new Error("Provider 状态回执不匹配，请刷新核实。");
        if(current===generation.current){setProvider(result);setMessage(result.enabled?"Provider 已启用。":"Provider 已停用。");}
        await onChanged();
      } else {
        const result=await service.deleteProviderCredential(providerId);
        if(result.provider_id!==providerId||result.credential_configured)throw new Error("凭证移除状态尚未确认，请刷新核实。");
        if(current===generation.current){setProvider({...provider,credential_configured:false});setMessage("服务器已确认移除凭证引用。不会读取或显示原密钥。");}
        await onChanged();
      }
      if(current===generation.current)setAction(undefined);
    }catch(e){if(current===generation.current){setAction(undefined);setError(`${(e as Error).message} 未自动重试；可刷新服务器状态核实。`);}}
    finally{locked.current=false;if(current===generation.current)setBusy(false);}
  };
  return <Disclosure title="连接状态与模型发现"><section aria-label="Provider 高级控制">
    {error&&<p role="alert">{error}</p>}{message&&<p role="status">{message}</p>}
    {!provider&&!error&&<p role="status">读取连接状态…</p>}
    {provider&&<><p>{provider.endpoint_summary} · {provider.enabled?"已启用":"已停用"} · {provider.credential_source||"没有凭证引用"}</p><p>{provider.health.safe_message}</p><div className="actions"><button disabled={busy||!provider.enabled} onClick={()=>setAction("discover")}>发现模型…</button><button disabled={busy} onClick={()=>setAction("toggle")}>{provider.enabled?"停用连接":"启用连接"}…</button><button disabled={busy||!provider.credential_configured} onClick={()=>setAction("remove")}>移除凭证…</button><button disabled={busy} onClick={()=>setReload(n=>n+1)}>刷新连接状态</button></div></>}
    {models&&<><p>{models.warning}</p><ul>{models.models.map(m=><li key={m.remote_model_id}>{m.remote_model_id}</li>)}</ul><a href="/settings/agent-models">管理模型配置</a></>}
    {action&&provider&&<Dialog title={action==="discover"?"发现远端模型":action==="toggle"?"确认连接状态变更":"移除 Provider 凭证"} onClose={()=>{if(!busy)setAction(undefined);}}><p>Provider：{provider.display_name}</p><p>目的地：{provider.endpoint_summary}</p><p>{action==="discover"?"使用此连接的凭证读取远端模型列表。不发送图片、不发起生成；返回 ID 不代表模型可用，不自动注册或探测模型。":action==="toggle"?"更改连接可用状态可能影响后续模型调用，不重写已发布 Workflow。":"移除服务器凭证引用将使需要该凭证的后续请求无法认证。不会删除模型、任务或图片；需要重新输入凭证才能恢复。"}</p><div className="actions"><button disabled={busy} onClick={()=>setAction(undefined)}>取消</button><button disabled={busy} onClick={()=>void execute()}>{busy?"等待服务器…":"确认"}</button></div></Dialog>}
  </section></Disclosure>;
}
