import { useEffect, useRef, useState } from "react";
import type { api } from "../api";
import type { HistoryRun, ImageItem, RunAnnotationInspection, RunResultSummary } from "../types";
import { AnnotationCanvas } from "../components/AnnotationCanvas";
import { Disclosure } from "./Disclosure";
import { agentPath } from "./navigationContract";
import { RunInspector, type RunInspectorService } from "./RunInspector";
import {versionLink} from "./WorkflowVersionDetail";

export type RunDetailService = Pick<typeof api,"run"|"runAnnotations"|"runResultSummary"|"images"|"control"> & RunInspectorService;
export function assertRunOwner(run:HistoryRun,project:string,id:string):HistoryRun {
  if(run.id!==id||run.project_id!==project||run.ownership_status!=="resolved")throw new Error("运行记录不属于当前项目，未打开其他对象。");
  return run;
}
export function runControls(run:HistoryRun):("pause"|"resume"|"cancel")[] {
  if(!run.controllable||run.in_trash)return [];
  if(run.status==="running")return ["pause","cancel"];
  if(run.status==="paused")return ["resume","cancel"];
  return ["pending","awaiting_review"].includes(run.status)?["cancel"]:[];
}
export function RunDetail({service,projectId,runId}:{service:RunDetailService;projectId:string;runId:string}) {
  const [run,setRun]=useState<HistoryRun>();
  const [annotations,setAnnotations]=useState<RunAnnotationInspection>();
  const [summary,setSummary]=useState<RunResultSummary>();
  const [image,setImage]=useState<ImageItem>();
  const [selected,setSelected]=useState<string|undefined>(()=>new URL(location.href).searchParams.get("annotation")||undefined);
  const [original,setOriginal]=useState(()=>new URL(location.href).searchParams.get("view")==="original");
  const [debug,setDebug]=useState(()=>new URL(location.href).searchParams.get("view")==="debug");
  const [error,setError]=useState("");
  const [message,setMessage]=useState("");
  const [busy,setBusy]=useState(false);
  const [reload,setReload]=useState(0);
  const pending=useRef(false);
  const lifetime=useRef<AbortController|undefined>(undefined);
  useEffect(()=>{
    const restore=()=>{const url=new URL(location.href);setDebug(url.searchParams.get("view")==="debug");setOriginal(url.searchParams.get("view")==="original");setSelected(url.searchParams.get("annotation")||undefined);};
    window.addEventListener("popstate",restore);return()=>window.removeEventListener("popstate",restore);
  },[]);
  const selectView=(raw:boolean,id?:string)=>{
    setOriginal(raw);setSelected(id);
    const url=new URL(location.href);url.searchParams.set("view",raw?"original":"results");
    if(id)url.searchParams.set("annotation",id);else url.searchParams.delete("annotation");
    history.replaceState(history.state,"",url);
  };
  useEffect(()=>{
    const controller=new AbortController();lifetime.current=controller;
    let timer:ReturnType<typeof setTimeout>|undefined;
    const read=async()=>{
      try{
        const first=await service.run(runId,controller.signal);
        const owned=assertRunOwner(first.run,projectId,runId);
        if(controller.signal.aborted)return;
        setRun(owned);
        const [results,totals,images]=await Promise.all([service.runAnnotations(runId,controller.signal),service.runResultSummary(runId,controller.signal),service.images(projectId,controller.signal)]);
        if(controller.signal.aborted)return;
        if(results.project_id!==projectId||results.run_id!==runId||totals.project_id!==projectId||totals.run_id!==runId)throw new Error("结果与运行归属不匹配");
        setRun(owned);setAnnotations(results);setSummary(totals);
        setImage(images.images.find(item=>item.project_id===projectId&&item.image_id===(results.image_id||owned.image_id)));
        setError("");
        if(["pending","running","paused","awaiting_review"].includes(owned.status))timer=setTimeout(()=>void read(),2000);
      }catch(reason){if(!controller.signal.aborted)setError((reason as Error).message);}
    };
    void read();return()=>{controller.abort();clearTimeout(timer);};
  },[service,projectId,runId,reload]);
  const control=async(action:"pause"|"resume"|"cancel")=>{
    if(pending.current||!run||!runControls(run).includes(action))return;
    if(action==="resume"&&!window.confirm("继续此 Run 的现有授权任务，可能继续产生模型费用。不会创建新 Run。继续？"))return;
    pending.current=true;setBusy(true);setError("");setMessage("");
    const signal=lifetime.current?.signal;
    try{
      const current=assertRunOwner((await service.run(runId,signal)).run,projectId,runId);
      if(!runControls(current).includes(action))throw new Error("服务器运行状态已变化；未重复发送控制请求。");
      await service.control(runId,action);
      if(!signal?.aborted){setMessage("服务器已响应控制请求，正在重新读取实际状态。");setReload(v=>v+1);}
    }catch(reason){if(!signal?.aborted)setError(`${(reason as Error).message}。请重新读取服务器状态；不会自动重试控制请求。`);}
    finally{pending.current=false;if(!signal?.aborted)setBusy(false);}
  };
  return <section className="native-project-manager native-review" aria-label="运行详情">
    <h1>运行结果</h1><nav className="native-management-tabs"><a href={agentPath({kind:"work",projectId})}>返回 Agent</a><a href={agentPath({kind:"management",projectId,page:"review"})}>审核队列</a><a href={agentPath({kind:"management",projectId,page:"export"})}>导出标注</a></nav>
    {error&&<p role="alert">{error}</p>}{message&&<p role="status">{message}</p>}
    {!run&&!error&&<p role="status">读取运行与最终结果…</p>}
    {run&&<><p role="status">{run.status} · {run.current_node||"未记录当前节点"}</p>
      {versionLink(projectId,run.workflow_version_id)&&<a href={versionLink(projectId,run.workflow_version_id)}>查看运行的 Workflow 版本</a>}
      <button aria-expanded={debug} onClick={()=>{const next=!debug;setDebug(next);const url=new URL(location.href);url.searchParams.set("view",next?"debug":original?"original":"results");history.replaceState(history.state,"",url);}}>{debug?"返回结果画布":"查看执行详情"}</button>
      {debug&&<RunInspector key={runId} service={service} projectId={projectId} runId={runId}/>}
      {run.terminal_reason&&<p>{run.terminal_reason}</p>}
      <div className="actions"><button disabled={busy} onClick={()=>setReload(v=>v+1)}>重新读取状态</button>{runControls(run).map(action=><button key={action} disabled={busy} onClick={()=>void control(action)}>{action==="pause"?"暂停":action==="resume"?"继续运行":"取消运行"}</button>)}</div>
      {run.controllable&&<p>离开页面不会取消运行。停止请使用取消运行。</p>}
      {summary&&<p>结果 {summary.result_count} · 可交付 {summary.ready_count} · 待审核 {summary.needs_review_count} · 未找到 {summary.no_target_count} · 失败 {summary.failed_count}</p>}
      {!debug&&<div className="actions"><button aria-pressed={original} onClick={()=>selectView(!original,selected)}>{original?"显示标注":"查看原图"}</button><span>{image?.name||"未找到对应原图"}</span></div>}
      {!debug&&(image?<AnnotationCanvas imageUrl={image.url} annotations={original?[]:annotations?.annotations||[]} selectedId={selected} onSelect={id=>selectView(original,id)} onChange={()=>{}} readOnly compactList/>:<p>没有用其他图片替代缺失的原图。</p>)}
      {annotations?.annotations.length===0&&<p>没有正式标注。待审候选、中间产物及失败原因不计为正式结果。</p>}
      <Disclosure title="正式标注列表">{annotations?.annotations.map(annotation=><article className="settings-row" key={annotation.id}><div><strong>{annotation.label||annotation.task_id}</strong><p>{annotation.review_status}</p></div><button onClick={()=>{selectView(false,annotation.id);}}>定位</button>{annotation.review_status==="needs_review"&&<a href={agentPath({kind:"detail",projectId,page:"review",objectId:annotation.id})}>审核这个对象</a>}</article>)}</Disclosure>
      <Disclosure title="运行来源与冻结绑定"><p>{run.workflow_name} · {run.workflow_version_id||run.workflow_version}</p><p>{run.created_at} — {run.updated_at}</p><p>tokens {run.input_tokens}/{run.output_tokens} · {Number(run.cost)>0?`服务器估算 $${run.cost}`:"费用未核实，零记录不代表免费"}</p><pre>{JSON.stringify(run.frozen_model_bindings,null,2)}</pre>{run.validation_issue_codes.map(code=><p key={code}>{code}</p>)}</Disclosure>
    </>}
  </section>;
}
