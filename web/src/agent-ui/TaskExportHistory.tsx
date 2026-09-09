import { useEffect, useState } from "react";
import type { api } from "../api";
import { Disclosure } from "./Disclosure";

export type TaskExportService=Pick<typeof api,"conversationExports"|"conversationExportStatus">;
type Delivery=Awaited<ReturnType<TaskExportService["conversationExports"]>>[number];
export function exportDeliveryState(value:Pick<Delivery,"error"|"result">,active?:boolean){
  return value.error?"导出失败":value.result?"导出报告已保存":active?"服务器正在导出":"完成状态未确认；读取状态不会重新导出";
}
export function TaskExportHistory({project,conversation,task,service}:{project:string;conversation:string;task:string;service:TaskExportService}){
  const [open,setOpen]=useState(false);
  return <Disclosure title="导出历史与兼容性报告" onToggle={e=>setOpen(e.currentTarget.open)}>{open&&<ExportPage key={`${project}:${conversation}:${task}`} project={project} conversation={conversation} task={task} service={service}/>}</Disclosure>;
}
function ExportPage({project,conversation,task,service}:{project:string;conversation:string;task:string;service:TaskExportService}){
  const [cursors,setCursors]=useState<(string|undefined)[]>([undefined]);const before=cursors.at(-1);
  const [rows,setRows]=useState<Delivery[]>();const [error,setError]=useState("");const [refresh,setRefresh]=useState(0);
  useEffect(()=>{
    const controller=new AbortController();setRows(undefined);setError("");
    void service.conversationExports(project,conversation,task,controller.signal,before,20).then(items=>{if(!controller.signal.aborted)setRows(items);}).catch(e=>{if(!controller.signal.aborted)setError((e as Error).message);});
    return()=>controller.abort();
  },[project,conversation,task,service,before,refresh]);
  return <section aria-label="任务导出历史"><p>由当前任务请求的项目级导出，可能包含同项目其他任务的已确认标注。这里不会启动或重试导出。</p>
    <div className="actions"><button disabled={cursors.length===1} onClick={()=>setCursors(v=>v.slice(0,-1))}>较新记录</button><button disabled={rows?.length!==20} onClick={()=>setCursors(v=>[...v,rows!.at(-1)!.id])}>较早记录</button><button onClick={()=>setRefresh(v=>v+1)}>刷新导出记录</button></div>
    {error&&<p role="alert">{error}</p>}{!rows&&!error&&<p role="status">读取导出记录…</p>}{rows?.length===0&&<p>当前页没有导出记录。</p>}
    {rows?.map(row=><ExportRow key={row.id} project={project} conversation={conversation} task={task} service={service} initial={row}/>)}
  </section>;
}
function ExportRow({project,conversation,task,service,initial}:{project:string;conversation:string;task:string;service:TaskExportService;initial:Delivery}){
  const [job,setJob]=useState(initial);const [active,setActive]=useState<boolean>();const [error,setError]=useState("");const [refresh,setRefresh]=useState(0);
  useEffect(()=>{
    if(initial.result||initial.error)return;
    const controller=new AbortController();let timer:ReturnType<typeof setTimeout>|undefined;
    const read=async()=>{
      try{const value=await service.conversationExportStatus(project,conversation,task,initial.id,controller.signal);if(controller.signal.aborted)return;
        if(value.job.id!==initial.id)throw new Error("导出回执 ID 不匹配");
        setJob({...initial,...value.job});setActive(value.active);setError("");
        if(value.active&&!value.job.result&&!value.job.error)timer=setTimeout(()=>void read(),2000);
      }catch(e){if(!controller.signal.aborted)setError((e as Error).message);}
    };void read();return()=>{controller.abort();if(timer!==undefined)clearTimeout(timer);};
  },[project,conversation,task,service,initial,refresh]);
  return <article className="settings-row"><div><strong>{initial.format}</strong><p>{initial.created_at}</p><p role="status">{exportDeliveryState(job,active)}</p>
    {job.error&&<p role="alert">{job.error}</p>}{error&&<p role="alert">{error}</p>}
    {!job.result&&!job.error&&<button onClick={()=>setRefresh(v=>v+1)}>核实此导出状态</button>}
    {job.result&&<><p>已导出 {job.result.report.exported_count} 条，跳过 {job.result.report.skipped_count} 条。</p>
      {job.result.delivery&&<a download href={`/api/projects/${encodeURIComponent(project)}/exports/${encodeURIComponent(job.result.delivery.id)}/download`}>下载标注文件</a>}
      <Disclosure title="兼容性报告">{job.result.report.warnings.length?<ul>{job.result.report.warnings.map((w,i)=><li key={i}>{w}</li>)}</ul>:<p>报告未列出兼容性警告；这不是标注准确率保证。</p>}</Disclosure>
    </>}
  </div></article>;
}
