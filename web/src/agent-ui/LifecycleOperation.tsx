import { useEffect, useRef, useState } from "react";
import type { api } from "../api";
import type { ManagementPreview, ManagementReceipt, ManagementRequest, WorkflowVersionRef } from "../types";
import { Dialog } from "./Dialog";
import { Disclosure } from "./Disclosure";

export type LifecycleService = Pick<typeof api, "previewManagement" | "executeManagement" | "managementOperation">;
export function validateImpact(request: ManagementRequest, preview: ManagementPreview) {
  const keys = (objects: ManagementRequest["objects"]) => objects.map(o => `${o.kind}:${o.id}:${o.version ?? ""}:${o.expected_revision}`).sort();
  if (preview.project_id !== request.project_id || preview.action !== request.action || JSON.stringify(keys(preview.objects)) !== JSON.stringify(keys(request.objects))) throw new Error("影响预览与所选项目、操作或对象版本不匹配");
  return preview;
}
const actionNames: Record<ManagementRequest["action"], string> = {move_to_trash:"移入回收站", restore:"恢复", archive:"归档", unarchive:"取消归档", rename:"重命名", set_default:"设为默认方案", clear_default:"清除默认方案", purge:"永久清理", cancel_and_delete:"取消运行并移入回收站"};

/** A new presentation of the existing preview/token/revision/idempotency protocol. */
export function LifecycleOperation({ initial, service, replacements = [], onClose, onSubmitted, onReceipt }: {
  initial: ManagementRequest;
  service: LifecycleService;
  replacements?: {label:string; value:WorkflowVersionRef}[];
  onClose:()=>void;
  onSubmitted:(request:ManagementRequest)=>void;
  onReceipt:(receipt:ManagementReceipt)=>void;
}) {
  const [request,setRequest]=useState(()=>structuredClone(initial));
  const [preview,setPreview]=useState<ManagementPreview>();
  const [error,setError]=useState("");
  const [busy,setBusy]=useState(false);
  const [submitted,setSubmitted]=useState(!!initial.confirmation_token);
  const [operationId,setOperationId]=useState<string>();
  const frozen=useRef<ManagementRequest | undefined>(initial.confirmation_token?structuredClone(initial):undefined);
  const [purge,setPurge]=useState("");
  const pending=useRef(false);
  const generation=useRef(0);
  useEffect(()=>{
    const current=++generation.current;
    setPreview(undefined);setError("");
    void service.previewManagement(request.project_id,request).then(p=>{if(current===generation.current)setPreview(validateImpact(request,p));}).catch(e=>{if(current===generation.current)setError(e.message);});
    return()=>{generation.current++;};
  },[request,service]);
  const verifyReceipt=(receipt:ManagementReceipt)=>{
    if(receipt.project_id!==request.project_id||!receipt.operation_id||receipt.action!==request.action)throw new Error("操作回执身份不匹配");
    setOperationId(receipt.operation_id);
    onReceipt(receipt);
  };
  const execute=async()=>{
    if(pending.current||submitted||!preview?.can_execute||(request.action==="purge"&&purge!=="DELETE"))return;
    const exact={...request,confirmation_token:preview.confirmation_token};
    pending.current=true;setBusy(true);setError("");
    try {
      // Parent persists recovery identity before the first mutation, never on GET.
      onSubmitted(exact);frozen.current=exact;setSubmitted(true);
      verifyReceipt(await service.executeManagement(exact.project_id,exact));
    } catch(e){setError(`${(e as Error).message}。没有自动重复执行；请核实原操作回执。`);}
    finally{pending.current=false;setBusy(false);}
  };
  return <Dialog title={actionNames[request.action]} onClose={()=>{if(!busy)onClose();}}>
    <p>当前项目：{request.project_id} · {request.objects.length} 个选中对象</p>
    {!preview&&!error&&<p role="status">读取真实影响预览…</p>}
    {error&&<p role="alert" className="error">{error}</p>}
    {preview&&<><p>{preview.summary}</p><dl className="lifecycle-impact">
      <div><dt>选中对象</dt><dd>{preview.impact.top_level_objects}</dd></div>
      <div><dt>包含的子运行</dt><dd>{preview.impact.child_runs}</dd></div>
      <div><dt>隐藏的待审核项</dt><dd>{preview.impact.unresolved_reviews_hidden}</dd></div>
      <div><dt>保留已确认标注</dt><dd>{preview.impact.confirmed_annotations_retained}</dd></div>
      <div><dt>历史引用</dt><dd>{preview.impact.historical_run_references}</dd></div>
      <div><dt>保留校准引用</dt><dd>{preview.impact.calibration_references}</dd></div>
    </dl><p>{preview.impact.estimate_note}</p><p>{preview.recoverable?"本操作可恢复。":"请检查不可恢复操作的范围。"}</p>
    {preview.blockers.length>0&&<section role="alert">{preview.blockers.map((b,i)=><p key={i}>{b.message}</p>)}</section>}
    {!submitted&&preview.blockers.some(b=>b.code==="default_replacement_required")&&<fieldset><legend>默认方案的处理方式</legend><label>替代版本<select value={request.replacement_default_version?JSON.stringify(request.replacement_default_version):""} onChange={e=>{const option=replacements.find(r=>JSON.stringify(r.value)===e.target.value);setRequest({...request,replacement_default_version:option?.value,clear_default:false,confirmation_token:undefined});}}><option value="">选择已发布版本</option>{replacements.map(r=><option key={JSON.stringify(r.value)} value={JSON.stringify(r.value)}>{r.label}</option>)}</select></label><button onClick={()=>setRequest({...request,replacement_default_version:undefined,clear_default:true,confirmation_token:undefined})}>明确清除默认方案并重新预览</button></fieldset>}
    <Disclosure title="查看对象与版本">{request.objects.map(o=><p key={`${o.kind}:${o.id}:${o.version}`}>{o.kind} · {o.id} · {o.version??""} · revision {o.expected_revision}</p>)}</Disclosure></>}
    {request.action==="purge"&&!submitted&&<label>输入 DELETE 确认永久清理<input value={purge} onChange={e=>setPurge(e.target.value)} autoComplete="off"/></label>}
    {submitted&&!operationId&&<p>未取得操作 ID。可以明确重试同一请求；原确认令牌、对象版本与幂等键保持不变，不新建操作。</p>}
    <div className="actions"><button disabled={busy} onClick={onClose}>{submitted?"关闭并保留操作记录":"取消"}</button>{submitted?<button disabled={busy||(!operationId&&!frozen.current)} onClick={()=>{if(pending.current)return;pending.current=true;setBusy(true);const result=operationId?service.managementOperation(request.project_id,operationId):service.executeManagement(request.project_id,frozen.current!);void result.then(verifyReceipt).catch(e=>setError(e.message)).finally(()=>{pending.current=false;setBusy(false);});}}>{operationId?"核实原操作结果":"重试原操作（相同幂等键）"}</button>:<button className={request.action==="purge"?"danger":"primary"} disabled={busy||!preview?.can_execute||(request.action==="purge"&&purge!=="DELETE")} onClick={()=>void execute()}>{busy?"等待服务端回执…":actionNames[request.action]}</button>}</div>
  </Dialog>;
}
