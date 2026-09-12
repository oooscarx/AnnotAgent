import { useEffect, useRef, useState } from "react";
import type { ProviderProfile, RegistryModelProfile } from "../types";
import type { api } from "../api";
import { Dialog } from "./Dialog";
import {GlobalModelDefaults,type GlobalDefaultsService} from "./GlobalModelDefaults";
import { ModelProfileEditor, type EditableModel, type ModelRuntimeOptions } from "./ModelProfileEditor";
import { ModelProfileActions, type ModelActionService } from "./ModelProfileActions";
import { ModelRequestEvidence, type ModelRuntimeEvidenceService } from "./ModelRequestEvidence";
export type ModelProfileService = Pick<typeof api, "modelProfiles" | "providers" | "createModelProfile" | "updateModelProfile"> & ModelActionService & GlobalDefaultsService & ModelRuntimeEvidenceService;
export type ModelProfileScope = "agent" | "vision";
export function modelMatchesScope(model:RegistryModelProfile,scope:ModelProfileScope){
  return scope==="agent"
    ? model.task_capabilities.includes("text_generation")
    : model.input_modalities.includes("image")&&model.task_capabilities.some(capability=>capability!=="text_generation");
}
export function ModelProfiles({service,scope}:{service:ModelProfileService;scope:ModelProfileScope}) {
  const [models,setModels]=useState<RegistryModelProfile[]>();
  const [providers,setProviders]=useState<ProviderProfile[]>([]);
  const [edit,setEdit]=useState<RegistryModelProfile | "new">();
  const [runtimeOptions,setRuntimeOptions]=useState<ModelRuntimeOptions>();
  const [query,setQuery]=useState("");
  const [error,setError]=useState(""); const [message,setMessage]=useState("");
  const [busy,setBusy]=useState(false);
  const pending=useRef(false); const revision=useRef(0); const mounted=useRef(false);
  const reload=async()=>{
    const generation=++revision.current;
    const [m,p]=await Promise.all([service.modelProfiles(),service.providers()]);
    if(mounted.current&&generation===revision.current){setModels(m.models);setProviders(p.providers);}
  };
  useEffect(()=>{mounted.current=true;void reload().catch(e=>{if(mounted.current)setError(e.message);});return()=>{mounted.current=false;revision.current++;};},[service]);
  useEffect(()=>{
    const guard=(e:Event)=>{if(edit&&!window.confirm("放弃尚未保存的模型修改？"))e.preventDefault();};
    const unload=(e:BeforeUnloadEvent)=>{if(edit){e.preventDefault();e.returnValue="";}};
    window.addEventListener("ui-preview:before-navigate",guard);window.addEventListener("beforeunload",unload);
    return()=>{window.removeEventListener("ui-preview:before-navigate",guard);window.removeEventListener("beforeunload",unload);};
  },[edit]);
  const save=async(value:EditableModel)=>{
    if(pending.current||!edit)return;
    pending.current=true;setBusy(true);setError("");
    try{
      if(edit==="new")await service.createModelProfile(value);
      else{
        const current=(await service.modelProfiles()).models.find(m=>m.id===edit.id);
        if(!current||current.revision!==edit.revision||current.locked)throw new Error("模型已变化或锁定；本地输入保留，请取消并重新读取后核对。");
        await service.updateModelProfile(edit.id,{...value,expected_revision:edit.revision});
      }
      if(!mounted.current)return;
      setEdit(undefined);setMessage("模型配置已保存。未执行探测或推理。");await reload();
    }catch(reason){if(mounted.current)setError((reason as Error).message);}
    finally{pending.current=false;if(mounted.current)setBusy(false);}
  };
  const scopedModels=models?.filter(model=>modelMatchesScope(model,scope));
  return <section aria-label={scope==="agent"?"模型配置":"视觉模型配置"}><h2>{scope==="agent"?"Agent 规划模型配置":"视觉 Model Profiles"}</h2><p>{scope==="agent"?"只显示具备 text_generation 的规划与对话候选；不会改变 Workflow 的视觉绑定。":"只显示具备图像输入和视觉任务能力的远程模型；Plugin 与本地 Model Instance 在下方单独管理。"} Registry 读取不会自动探测或更改已有发布版本。</p>
    <GlobalModelDefaults service={service} scope={scope}/>
    {error&&!edit&&<p role="alert" className="error">{error}</p>}{message&&<p role="status">{message}</p>}
    {!models&&!error&&<p role="status">读取模型配置…</p>}
    <div className="actions"><button disabled={busy} onClick={()=>void reload().catch(e=>setError(e.message))}>重新读取</button><button disabled={!providers.length||busy} onClick={()=>{setError("");setRuntimeOptions(undefined);setEdit("new");}}>添加模型配置</button></div>
    {!providers.length&&models&&<p>先在 Providers 中保存连接；添加模型不需要收费探测。</p>}
    <label>搜索模型<input value={query} onChange={e=>setQuery(e.target.value)} placeholder="名称、模型 ID 或能力"/></label>
    {scopedModels?.filter(m=>[m.display_name,m.remote_model_id,...m.task_capabilities].join(" ").toLowerCase().includes(query.toLowerCase())).map(m=><div className="settings-row" key={m.id}><div><strong>{m.display_name}</strong><p>{providers.find(p=>p.id===m.provider_id)?.display_name||"Provider 不存在"} · {m.status}</p><p>{m.remote_model_id}</p><p>{m.input_modalities.join(" · ")} · {m.task_capabilities.join(" · ")}</p><ModelProfileActions key={`${m.id}:${m.revision}:${m.locked}`} model={m} provider={providers.find(p=>p.id===m.provider_id)} service={service} reload={reload}/></div><button disabled={m.locked||busy} onClick={()=>{setError("");setRuntimeOptions(undefined);setEdit(structuredClone(m));}}>{m.locked?"已锁定":"编辑配置"}</button></div>)}
    {scopedModels?.length===0&&<p>{scope==="agent"?"没有具备文字生成能力的 Agent 模型配置。":"没有具备图像输入和视觉任务能力的远程模型配置。"}</p>}
    {edit&&<Dialog title={edit==="new"?"添加模型配置":"编辑模型配置"} onClose={()=>{if(!busy&&window.confirm("放弃编辑？"))setEdit(undefined);}}>
      {error&&<p role="alert">{error}</p>}
      {edit!=="new"&&<ModelRequestEvidence model={edit} service={service} onRuntimeOptions={setRuntimeOptions}/>}
      <ModelProfileEditor model={edit==="new"?undefined:edit} providers={providers} busy={busy} runtimeOptions={runtimeOptions} cancel={()=>setEdit(undefined)} save={value=>void save(value)}/>
    </Dialog>}
  </section>;
}
