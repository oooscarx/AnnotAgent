import {useEffect,useRef,useState} from "react";
import type {api} from "../api";
import type {WorkflowDraft,StaticWorkflowValidation} from "../types";
import {Disclosure} from "./Disclosure";
import {agentPath} from "./navigationContract";
import {WorkflowCatalogEditor,type WorkflowCatalogService} from "./WorkflowCatalogEditor";
export type WorkflowEditorService=Pick<typeof api,"workflowDraft"|"validateWorkflowDraft"|"saveWorkflowDraft"> & WorkflowCatalogService;
export function verifyStaticValidation(base:WorkflowDraft,result:StaticWorkflowValidation) {
  if(result.validation_kind!=="static"||result.project_id!==base.project_id||result.draft_id!==base.id||result.revision!==base.revision||result.content_hash!==base.content_hash)throw new Error("校验回执不属于当前已保存版本，请重新读取草稿");
  return result;
}
const editableFields=["nodes","edges","enabled_skills","resource_versions","runtime_policies","allow_unvalidated_commit","label_pipeline"] as const;
export function editableWorkflow(draft:WorkflowDraft){return Object.fromEntries(editableFields.filter(key=>draft[key]!==undefined).map(key=>[key,draft[key]]));}
export function workflowEdit(base:WorkflowDraft,name:string,text:string):WorkflowDraft {
  const value:unknown=JSON.parse(text);
  if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("执行配置必须是 JSON 对象");
  const fields=value as Record<string,unknown>;
  if(Object.keys(fields).some(key=>!(editableFields as readonly string[]).includes(key)))throw new Error("不能修改草稿身份、revision、状态或时间字段");
  if(!Array.isArray(fields.nodes))throw new Error("nodes 必须是数组");
  if(!name.trim())throw new Error("方案名称不能为空");
  const next={...base,name:name.trim()};
  for(const key of editableFields)delete (next as unknown as Record<string,unknown>)[key];
  return {...next,...fields} as WorkflowDraft;
}
export function WorkflowEditor({service,projectId,draftId}:{service:WorkflowEditorService;projectId:string;draftId:string}) {
  const [validation,setValidation]=useState<StaticWorkflowValidation>();const [validating,setValidating]=useState(false);const checking=useRef(false);
  const [base,setBase]=useState<WorkflowDraft>();const [name,setName]=useState("");const [text,setText]=useState("");const [error,setError]=useState("");const [message,setMessage]=useState("");const [busy,setBusy]=useState(false);const [reload,setReload]=useState(0);const pending=useRef(false);const lifetime=useRef<AbortController|undefined>(undefined);
  const dirty=!!base&&(name!==base.name||text!==JSON.stringify(editableWorkflow(base),null,2));
  const locked=base?.status==="published"||base?.status==="archived";
  let working:WorkflowDraft|undefined;try{if(base)working=workflowEdit(base,name,text);}catch{/* Invalid JSON stays editable; catalog does not discard it. */}
  const accept=(draft:WorkflowDraft)=>{setValidation(undefined);setBase(draft);setName(draft.name);setText(JSON.stringify(editableWorkflow(draft),null,2));};
  useEffect(()=>{const controller=new AbortController();lifetime.current=controller;setBase(undefined);setValidation(undefined);setError("");void service.workflowDraft(projectId,draftId,controller.signal).then(draft=>{if(controller.signal.aborted)return;if(draft.id!==draftId||draft.project_id!==projectId)throw new Error("此项目中未找到该草稿，可能已删除、归档或归属不同。没有自动打开其他方案。");accept(draft);}).catch(e=>{if(!controller.signal.aborted)setError(e.message);});return()=>controller.abort();},[service,projectId,draftId,reload]);
  useEffect(()=>{const guard=(e:Event)=>{if((dirty||busy)&&!window.confirm("草稿有未保存修改或保存结果待核实，仍要离开？"))e.preventDefault();};const unload=(e:BeforeUnloadEvent)=>{if(dirty||busy){e.preventDefault();e.returnValue="";}};window.addEventListener("ui-preview:before-navigate",guard);window.addEventListener("beforeunload",unload);return()=>{window.removeEventListener("ui-preview:before-navigate",guard);window.removeEventListener("beforeunload",unload);};},[dirty,busy]);
  const save=async()=>{if(!base||locked||pending.current||!dirty)return;pending.current=true;setBusy(true);setError("");setMessage("");const signal=lifetime.current?.signal;
    try{const next=workflowEdit(base,name,text);const result=await service.saveWorkflowDraft(next,signal);if(signal?.aborted)return;if(result.id!==draftId||result.project_id!==projectId)throw new Error("保存回执身份不匹配");accept(result);setMessage("草稿已保存。未自动发布、验证或执行模型。");}
    catch(e){if(!signal?.aborted)setError(`${(e as Error).message}。保留当前编辑，未自动覆盖或重试。`);}finally{pending.current=false;if(!signal?.aborted)setBusy(false);}
  };
  const validate=async()=>{if(!base||dirty||busy||checking.current)return;checking.current=true;setValidating(true);setValidation(undefined);setError("");const signal=lifetime.current?.signal;
    try{const result=await service.validateWorkflowDraft(projectId,draftId,base.revision,signal);if(!signal?.aborted)setValidation(verifyStaticValidation(base,result));}
    catch(e){if(!signal?.aborted)setError((e as Error).message);}finally{checking.current=false;if(!signal?.aborted)setValidating(false);}
  };
  return <section className="native-project-manager" aria-label="高级 Workflow 草稿"><h1>编辑自动化草稿</h1><nav className="native-management-tabs"><a href={agentPath({kind:"work",projectId})}>返回 Agent</a><a href={agentPath({kind:"management",projectId,page:"trash"})}>回收站</a></nav>
    {error&&<p role="alert">{error}</p>}{message&&<p role="status">{message}</p>}{!base&&!error&&<p role="status">读取当前项目的草稿…</p>}
    <button disabled={busy||validating} onClick={()=>{if(!dirty||window.confirm("丢弃当前编辑并读取服务器最新草稿？"))setReload(v=>v+1);}}>重新读取草稿</button>
    {base&&<><p>revision {base.revision} · {base.status} · {busy?"保存中":dirty?"有未保存修改":"已读取服务器版本"}</p>{locked&&<p>此草稿已发布或归档，本页不直接修改。不可变版本保持不变。</p>}
      <form onSubmit={e=>{e.preventDefault();void save();}}><label>方案名称<input disabled={busy||locked} value={name} onChange={e=>setName(e.target.value)}/></label>
      {working?<WorkflowCatalogEditor service={service} draft={working} disabled={busy||!!locked} onChange={next=>setText(JSON.stringify(editableWorkflow(next),null,2))}/>:<p>请先修正名称或 JSON 格式，再从目录添加节点。</p>}
      <Disclosure title="高级执行配置" open><p>编辑真实节点、绑定与 Label Pipeline。身份和版本字段不可编辑。保存不是发布，也不代表静态校验通过。</p><label>执行配置 JSON<textarea rows={18} spellCheck={false} disabled={busy||locked} value={text} onChange={e=>setText(e.target.value)}/></label></Disclosure>
      <div className="actions"><button type="button" disabled={busy||!dirty} onClick={()=>{accept(base);setError("");}}>取消修改</button><button disabled={busy||locked||!dirty}>{busy?"保存中…":"保存草稿"}</button></div></form>
      <section aria-label="静态校验"><h2>静态校验</h2><p>仅检查已保存的节点类型、模型绑定与安全约束，不调用模型、不创建样本或发布版本。通过不代表实际推理质量。</p><button disabled={busy||dirty||validating} onClick={()=>void validate()}>{validating?"校验中…":"校验已保存草稿"}</button>{dirty&&<p>请先保存修改，再校验当前版本。</p>}
      {validation&&!dirty&&validation.revision===base.revision&&validation.content_hash===base.content_hash&&<div role="status"><p>{validation.validation.valid?"静态校验通过":"静态校验未通过"} · revision {validation.revision}</p><ul>{validation.validation.issues.map((issue,index)=><li key={index}><strong>{issue.blocking?"阻止执行":"提示"}：{issue.message}</strong><p>{issue.path} · {issue.code}</p></li>)}</ul></div>}</section>
      <Disclosure title="已保存节点与实际绑定">{base.nodes.map(node=><article className="settings-row" key={node.id}><div><strong>{node.node_type} · {node.id}</strong><p>{node.model_binding||"无直接模型绑定"}</p><pre>{JSON.stringify(node.model_profile_binding??null,null,2)}</pre></div></article>)}</Disclosure>
    </>}
  </section>;
}
