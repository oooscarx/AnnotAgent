import { useEffect, useRef, useState } from "react";
import type { api } from "../api";
import type { DatasetBatchSummary } from "../types";
import { agentPath } from "./navigationContract";
import { Disclosure } from "./Disclosure";
import { Dialog } from "./Dialog";
export type BatchDetailService = Pick<typeof api,"batch"|"controlBatch">;
export function ownedBatch(batch:DatasetBatchSummary,project:string,id:string) {
  if(batch.id!==id||batch.project_id!==project)throw new Error("批处理不属于当前项目，未打开其他对象。");
  return batch;
}
export function batchControls(batch:DatasetBatchSummary):("pause"|"resume"|"cancel")[] {
  if(batch.in_trash)return [];
  if(batch.status==="running")return ["pause","cancel"];
  if(batch.status==="paused"||batch.status==="pending")return ["resume","cancel"];
  if(batch.status==="awaiting_review")return ["cancel"];
  return [];
}
export function BatchDetail({service,projectId,batchId}:{service:BatchDetailService;projectId:string;batchId:string}) {
  const [batch,setBatch]=useState<DatasetBatchSummary>();const [error,setError]=useState("");const [message,setMessage]=useState("");const [busy,setBusy]=useState(false);const [reload,setReload]=useState(0);
  const [confirmation,setConfirmation]=useState<"pause"|"resume"|"cancel">();
  const [filter,setFilter]=useState(()=>new URL(location.href).searchParams.get("status")||"all");
  const pending=useRef(false);const lifetime=useRef<AbortController|undefined>(undefined);
  useEffect(()=>{const controller=new AbortController();lifetime.current=controller;let timer:ReturnType<typeof setTimeout>|undefined;
    const read=async()=>{try{const value=ownedBatch((await service.batch(batchId,controller.signal)).batch,projectId,batchId);if(controller.signal.aborted)return;setBatch(value);setError("");if(["running","pending","paused","awaiting_review"].includes(value.status))timer=setTimeout(()=>void read(),2000);}catch(e){if(!controller.signal.aborted)setError((e as Error).message);}};
    void read();return()=>{controller.abort();clearTimeout(timer);};
  },[service,projectId,batchId,reload]);
  useEffect(()=>{const restore=()=>setFilter(new URL(location.href).searchParams.get("status")||"all");window.addEventListener("popstate",restore);return()=>window.removeEventListener("popstate",restore);},[]);
  const control=async(action:"pause"|"resume"|"cancel")=>{
    if(pending.current||!batch||!batchControls(batch).includes(action))return;
    pending.current=true;setBusy(true);setError("");setMessage("");const signal=lifetime.current?.signal;
    try{const current=ownedBatch((await service.batch(batchId,signal)).batch,projectId,batchId);if(signal?.aborted)return;if(!batchControls(current).includes(action))throw new Error("状态已变化，未发送控制请求");await service.controlBatch(batchId,action);if(!signal?.aborted){setMessage("服务器已响应，正在核实批次状态。");setReload(v=>v+1);}}
    catch(e){if(!signal?.aborted)setError(`${(e as Error).message}。未自动重试，请重新读取状态。`);}finally{pending.current=false;if(!signal?.aborted)setBusy(false);}
  };
  return <section className="native-project-manager" aria-label="批处理详情"><h1>批量处理</h1>
    <nav className="native-management-tabs"><a href={agentPath({kind:"work",projectId})}>返回 Agent</a><a href={agentPath({kind:"management",projectId,page:"review"})}>审核队列</a><a href={agentPath({kind:"management",projectId,page:"export"})}>导出标注</a><a href={agentPath({kind:"management",projectId,page:"trash"})}>回收站</a></nav>
    {error&&<p role="alert">{error}</p>}{message&&<p role="status">{message}</p>}{!batch&&!error&&<p role="status">读取批处理…</p>}
    {batch&&<><p role="status">{batch.status}{batch.in_trash?" · 已在回收站":""}</p><p>离开页面不会取消后台处理。当前状态只来自服务器。</p>
      <div className="actions"><button disabled={busy} onClick={()=>setReload(v=>v+1)}>重新读取批次</button>{batchControls(batch).map(action=><button disabled={busy} key={action} onClick={()=>setConfirmation(action)}>{action==="pause"?"暂停批次":action==="resume"?"继续批次":"取消批次"}</button>)}</div>
      <p>共 {batch.progress.total_images} 张 · 完成 {batch.progress.completed_images} · 运行中 {batch.progress.running_images} · 待审核 {batch.progress.review_images} · 失败 {batch.progress.failed_images} · 等待 {batch.progress.pending_images} · 取消 {batch.progress.cancelled_images}</p>
      <p>已记录调用 {batch.budget_ledger.consumed.request_count} 次 · {batch.budget_ledger.consumed.total_tokens} tokens · {Number(batch.budget_ledger.consumed.cost)>0?`估算 $${batch.budget_ledger.consumed.cost}`:"费用未核实，零记录不代表免费"}</p>
      <label>筛选图片状态<select value={filter} onChange={e=>{setFilter(e.target.value);const url=new URL(location.href);url.searchParams.set("status",e.target.value);history.replaceState(history.state,"",url);}}><option value="all">全部图片</option>{[...new Set([...batch.images.map(i=>i.status),...(filter!=="all"?[filter]:[])])].map(status=><option key={status} value={status}>{status}</option>)}</select></label>
      {batch.images.filter(i=>filter==="all"||i.status===filter).map(image=><article className="settings-row" key={image.image_id}><div><strong>{image.name}</strong><p>{image.status} · 标注 {image.annotation_count} · 待审 {image.review_count}</p>{image.failure&&<p>{image.failure}</p>}</div>{image.child_run_id?<a href={agentPath({kind:"detail",projectId,page:"runs",objectId:image.child_run_id})}>查看图片结果</a>:<span>尚无可打开的运行结果</span>}</article>)}
      {!batch.images.some(i=>filter==="all"||i.status===filter)&&<p>当前筛选下没有图片。</p>}
      <Disclosure title="批次来源与冻结方案"><p>{batch.id} · {batch.workflow_version}</p><p>{batch.created_at} — {batch.updated_at}</p><p>已删除子运行 {batch.deleted_child_runs}；未用新结果替代。</p><p>方案：{batch.workflow_snapshot.workflow?.draft?.name || batch.workflow_snapshot.draft?.name || "未记录名称"}。每张图片的实际冻结绑定在对应 Run 来源中查看。</p></Disclosure>
    </>}
    {confirmation&&<Dialog title={confirmation==="resume"?"确认继续批次":confirmation==="cancel"?"确认取消批次":"确认暂停批次"} onClose={()=>{if(!busy)setConfirmation(undefined);}}>
      <p>{confirmation==="resume"?"继续此批次的现有授权处理，可能继续产生模型费用；不会新建批次。":confirmation==="cancel"?"取消此批次尚未完成的处理。已经保存的结果不会删除，之后仍可从历史中查看。":"请求服务器暂停后续处理；已经发出的远端请求不一定能立即中断。"}</p>
      <div className="actions"><button disabled={busy} onClick={()=>setConfirmation(undefined)}>返回</button><button disabled={busy} onClick={async()=>{const action=confirmation;await control(action);setConfirmation(undefined);}}>{busy?"正在核实…":confirmation==="resume"?"继续批次":confirmation==="cancel"?"确认取消":"确认暂停"}</button></div>
    </Dialog>}
  </section>;
}
