import { useEffect, useState } from "react";
import { api, request } from "../api";
import type { HistoryRun, DatasetBatchSummary, PipelineLifecycleSummary, ManagementObjectRef, ManagementRequest, ManagementReceipt } from "../types";
import { HistoryBoundary } from "./HistoryBoundary";
import { historyScopeApi, historyQuery, type HistoryScope } from "./historyScope";
import { LifecycleOperation } from "./LifecycleOperation";
import { Disclosure } from "./Disclosure";

type Page = {total:number;offset:number;next_offset:number|null};
export const historyManagementApi = {
  historyScope:historyScopeApi,
  previewManagement:api.previewManagement, executeManagement:api.executeManagement, managementOperation:api.managementOperation,
  pipelines:(project:string,scope:HistoryScope,offset:number,archived:boolean,signal?:AbortSignal)=>{
    const params=historyQuery(scope,offset);params.set("include_archived",String(archived));
    return request<{pipelines:PipelineLifecycleSummary[];page:Page}>(`/api/projects/${encodeURIComponent(project)}/pipelines?${params}`,{signal});
  },
  runs:(project:string,scope:HistoryScope,offset:number,signal?:AbortSignal)=>{
    const params=historyQuery(scope,offset);params.set("project_id",project);
    return request<{runs:HistoryRun[];page:Page}>(`/api/runs?${params}`,{signal});
  },
  batches:(project:string,scope:HistoryScope,offset:number,signal?:AbortSignal)=>{
    const params=historyQuery(scope,offset);params.set("project_id",project);
    return request<{batches:DatasetBatchSummary[];page:Page}>(`/api/batches?${params}`,{signal});
  },
};
export type HistoryService=typeof historyManagementApi;
export const historyObjectKey=(o:ManagementObjectRef)=>`${o.kind}:${o.id}:${o.version??""}`;
export function removeCoveredChildren(objects:ManagementObjectRef[],pipelines:PipelineLifecycleSummary[]) {
  const children=new Set(pipelines.filter(p=>objects.some(o=>o.kind==="pipeline"&&o.id===p.workflow_id)).flatMap(p=>[...p.drafts,...p.versions].map(c=>historyObjectKey(c.object))));
  return objects.filter(o=>!children.has(historyObjectKey(o)));
}
export function HistoryManagement({projectId,workspaceId,kind,service}:{projectId:string;workspaceId:string;kind:"pipelines"|"runs";service:HistoryService}) {
  return <HistoryBoundary workspaceId={workspaceId} service={service.historyScope}>{scope=><ScopedHistory key={`${projectId}:${scope.id}:${kind}`} projectId={projectId} workspaceId={workspaceId} scope={scope} kind={kind} service={service}/>}</HistoryBoundary>;
}
function ScopedHistory({projectId,workspaceId,scope,kind,service}:{projectId:string;workspaceId:string;scope:HistoryScope;kind:"pipelines"|"runs";service:HistoryService}) {
  const [pipelines,setPipelines]=useState<PipelineLifecycleSummary[]>([]);
  const [runs,setRuns]=useState<HistoryRun[]>([]);
  const [batches,setBatches]=useState<DatasetBatchSummary[]>([]);
  const [mode,setMode]=useState<"runs"|"batches">("runs");
  const [archived,setArchived]=useState(false);
  const [page,setPage]=useState<Page>();
  const [offset,setOffset]=useState(0);
  const [reload,setReload]=useState(0);
  const [error,setError]=useState("");
  const [selected,setSelected]=useState<Map<string,ManagementObjectRef>>(new Map());
  const [operation,setOperation]=useState<ManagementRequest>();
  const [pending,setPending]=useState<ManagementRequest>();
  const [receipt,setReceipt]=useState<ManagementReceipt>();
  const [rename,setRename]=useState<{object:ManagementObjectRef;name:string}>();
  const key=`annotagent.history-operation.${workspaceId}.${projectId}.${scope.id}.${kind}`;
  useEffect(()=>{
    try {const raw=localStorage.getItem(key);if(!raw)return;const value=JSON.parse(raw) as ManagementRequest;if(value.project_id!==projectId||value.history_scope!==scope.id||!value.confirmation_token)throw new Error("操作恢复范围不匹配");setPending(value);}
    catch(e){setError((e as Error).message);}
  },[key,projectId,scope.id]);
  useEffect(()=>{
    const controller=new AbortController();setPage(undefined);setError("");setPipelines([]);setRuns([]);setBatches([]);setSelected(new Map());
    const load=async()=>{
      let result:Page;
      if(kind==="pipelines") {const r=await service.pipelines(projectId,scope,offset,archived,controller.signal);if(controller.signal.aborted)return;if(r.pipelines.some(p=>p.project_id!==projectId))throw new Error("方案归属不匹配");setPipelines(r.pipelines);result=r.page;}
      else if(mode==="batches") {const r=await service.batches(projectId,scope,offset,controller.signal);if(controller.signal.aborted)return;if(r.batches.some(b=>b.project_id!==projectId))throw new Error("批次归属不匹配");setBatches(r.batches);result=r.page;}
      else {const r=await service.runs(projectId,scope,offset,controller.signal);if(controller.signal.aborted)return;if(r.runs.some(r=>r.project_id!==projectId||r.ownership_status!=="resolved"))throw new Error("Run 归属不匹配");setRuns(r.runs);result=r.page;}
      if(!result||result.offset!==offset)throw new Error("服务器未返回匹配的隔离分页");setPage(result);
    };
    void load().catch(e=>{if(!controller.signal.aborted){setError(e.message);setPipelines([]);setRuns([]);setBatches([]);}});
    return()=>controller.abort();
  },[service,projectId,scope,kind,mode,offset,archived,reload]);
  const all:ManagementObjectRef[]=kind==="pipelines"?pipelines.flatMap(p=>[{kind:"pipeline" as const,id:p.workflow_id,expected_revision:p.lifecycle_revision},...p.drafts.map(d=>d.object),...p.versions.map(v=>v.object)]):mode==="runs"?runs.map(r=>({kind:"run",id:r.id,expected_revision:r.lifecycle_revision})):batches.map(b=>({kind:"batch",id:b.id,expected_revision:b.lifecycle_revision}));
  const choose=(o:ManagementObjectRef,checked:boolean)=>setSelected(old=>{const next=new Map(old);if(checked)next.set(historyObjectKey(o),o);else next.delete(historyObjectKey(o));return next;});
  const select=(o:ManagementObjectRef,title:string)=><input type="checkbox" aria-label={`选择 ${title}`} checked={selected.has(historyObjectKey(o))} onChange={e=>choose(o,e.target.checked)}/>;
  const open=(action:ManagementRequest["action"],objects:ManagementObjectRef[],displayName?:string)=>{
    if(pending)return;
    setOperation({project_id:projectId,history_scope:scope.id,objects:removeCoveredChildren(objects,pipelines),action,idempotency_key:crypto.randomUUID(),...(displayName?{display_name:displayName}:{})});
  };
  const controls=(o:ManagementObjectRef,isArchived:boolean)=><><button disabled={!!pending} onClick={()=>open(isArchived?"unarchive":"archive",[o])}>{isArchived?"取消归档":"归档"}…</button><button disabled={!!pending} onClick={()=>open("move_to_trash",[o])}>移入回收站…</button></>;
  const base=`/projects/${encodeURIComponent(projectId)}`;
  return <section className="native-project-manager native-history"><h1>{kind==="pipelines"?"自动化方案与版本":"处理记录"}</h1><p>只列出新历史范围内的记录。旧任务的来源链接仍可读取，原图和标注不因列表隔离而删除。</p>
    <div className="actions"><a href={`${base}/work`}>返回 Agent</a><a href={`${base}/manage/trash`}>回收站</a><button onClick={()=>{setOffset(0);setReload(n=>n+1);}}>刷新</button></div>
    {error&&<p role="alert">{error}</p>}
    {pending&&<p role="status">有已提交操作待核实。<button onClick={()=>setOperation(pending)}>查看原操作</button></p>}
    {receipt&&<p role="status">{receipt.action}：{receipt.status} {receipt.error||""}</p>}
    {kind==="pipelines"?<label><input type="checkbox" checked={archived} onChange={e=>{setArchived(e.target.checked);setOffset(0);}}/>显示已归档方案</label>:<label>记录类型<select value={mode} onChange={e=>{setMode(e.target.value as "runs"|"batches");setOffset(0);}}><option value="runs">单图 Run</option><option value="batches">数据集 Batch</option></select></label>}
    <div className="actions"><label><input type="checkbox" disabled={!page||!all.length} checked={!!all.length&&selected.size===all.length} ref={node=>{if(node)node.indeterminate=selected.size>0&&selected.size<all.length;}} onChange={e=>setSelected(new Map(e.target.checked?all.map(o=>[historyObjectKey(o),o]):[]))}/>全选当前页</label><span>{selected.size} 项已选</span><button disabled={!page||!selected.size||!!pending} onClick={()=>open("move_to_trash",[...selected.values()])}>选中项移入回收站…</button></div>
    {!page&&!error&&<p role="status">读取历史…</p>}{page?.total===0&&<p>当前范围内没有记录。可返回 Agent 创建新的任务。</p>}
    {page&&pipelines.map(p=>{const o:ManagementObjectRef={kind:"pipeline",id:p.workflow_id,expected_revision:p.lifecycle_revision};return <article key={p.workflow_id}><div className="settings-row">{select(o,p.display_name)}<strong>{p.display_name}</strong><span>{p.archived_at?"已归档":""}{p.default_version?` 默认 v${p.default_version}`:""}</span><div className="actions"><button onClick={()=>setRename({object:o,name:p.display_name})}>重命名</button>{controls(o,!!p.archived_at)}</div></div>
      {rename?.object.id===p.workflow_id&&<div className="actions"><label>显示名称<input maxLength={160} value={rename.name} onChange={e=>setRename({...rename,name:e.target.value})}/></label><button onClick={()=>setRename(undefined)}>取消</button><button disabled={!rename.name.trim()||!!pending} onClick={()=>{open("rename",[rename.object],rename.name.trim());setRename(undefined);}}>预览重命名…</button></div>}
      <Disclosure title={`草稿 ${p.drafts.length} · 已发布版本 ${p.versions.length}`}>
        {[...p.drafts,...p.versions].map(item=><div className="settings-row" key={historyObjectKey(item.object)}>{select(item.object,item.display_name)}<span>{item.display_name}{item.object.version?` · v${item.object.version}`:""}{item.is_default?" · 默认":""}</span><div className="actions"><a href={`${base}/manage/pipelines/${encodeURIComponent(item.object.id)}${item.object.kind==="workflow_version"?`?version=${item.object.version}`:""}`}>打开</a>{controls(item.object,!!item.archived_at)}{item.object.kind==="workflow_version"&&<button disabled={!!pending} onClick={()=>open(item.is_default?"clear_default":"set_default",[item.object])}>{item.is_default?"清除默认":"设为默认"}…</button>}</div></div>)}
      </Disclosure></article>;})}
    {page&&runs.map(r=>{const o:ManagementObjectRef={kind:"run",id:r.id,expected_revision:r.lifecycle_revision};return <div className="settings-row" key={r.id}>{select(o,r.id)}<span>{r.workflow_name||r.id} · {r.status}</span><div className="actions"><a href={`${base}/manage/runs/${encodeURIComponent(r.id)}`}>查看结果</a>{controls(o,!!r.archived_at)}{r.controllable&&<button disabled={!!pending} onClick={()=>open("cancel_and_delete",[o])}>取消并移入回收站…</button>}</div></div>;})}
    {page&&batches.map(b=>{const o:ManagementObjectRef={kind:"batch",id:b.id,expected_revision:b.lifecycle_revision};return <div className="settings-row" key={b.id}>{select(o,b.id)}<span>{b.id} · {b.status} · {b.progress.completed_images}/{b.progress.total_images}</span><div className="actions"><a href={`${base}/manage/batches/${encodeURIComponent(b.id)}`}>查看批次</a>{controls(o,!!b.archived_at)}<button disabled={!!pending} onClick={()=>open("cancel_and_delete",[o])}>取消并移入回收站…</button></div></div>;})}
    <div className="actions"><button disabled={!page||offset===0} onClick={()=>setOffset(Math.max(0,offset-50))}>上一页</button><span>{page?`共 ${page.total} 项`:""}</span><button disabled={!page||page.next_offset===null} onClick={()=>setOffset(page!.next_offset!)}>下一页</button></div>
    {operation&&<LifecycleOperation key={operation.idempotency_key} initial={operation} service={service} replacements={pipelines.flatMap(p=>p.versions.filter(v=>!v.archived_at&&!v.deleted_at).map(v=>({label:`${p.display_name} v${v.object.version}`,value:{workflow_id:v.object.id,version:v.object.version!}})))} onClose={()=>setOperation(undefined)} onSubmitted={exact=>{localStorage.setItem(key,JSON.stringify(exact));setPending(exact);}} onReceipt={r=>{setReceipt(r);if(r.status==="completed"){localStorage.removeItem(key);setPending(undefined);setOperation(undefined);setOffset(0);setReload(n=>n+1);}}}/>}
  </section>;
}
