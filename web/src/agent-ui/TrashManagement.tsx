import { useEffect, useRef, useState } from "react";
import type { api } from "../api";
import type { ManagementReceipt, ManagementRequest, TrashEntry } from "../types";
import { LifecycleOperation, type LifecycleService } from "./LifecycleOperation";
import { Disclosure } from "./Disclosure";
import { HistoryBoundary } from "./HistoryBoundary";
import type { HistoryScope, historyScopeApi } from "./historyScope";
export type TrashService = LifecycleService & Pick<typeof api,"trash"|"batch"> & {historyScope:typeof historyScopeApi};
const keyOf = (entry:TrashEntry) => `${entry.object.kind}:${entry.object.id}:${entry.object.version??""}`;
export function trashTargets(entries:TrashEntry[]) {
  const parents=new Set(entries.filter(e=>e.object.kind==="pipeline").map(e=>e.object.id));
  return entries.filter(e=>!(["workflow_draft","workflow_version"].includes(e.object.kind)&&parents.has(e.object.id))).map(e=>e.object);
}
export async function ownedTrashTargets(entries:TrashEntry[],project:string,service:Pick<typeof api,"batch">){
  if(entries.some(e=>e.project_id!==project))throw new Error("回收站选择包含其他项目对象");
  const objects=trashTargets(entries);
  if(!objects.some(o=>o.kind==="run"))return objects;
  const children=new Set<string>();
  await Promise.all(objects.filter(o=>o.kind==="batch").map(async object=>{
    const {batch}=await service.batch(object.id);
    if(batch.project_id!==project||batch.id!==object.id||batch.lifecycle_revision!==object.expected_revision||!batch.in_trash)throw new Error("所选批次归属或版本已变化，请刷新回收站；未执行操作。");
    for(const id of batch.child_run_ids)children.add(id);
  }));
  return objects.filter(o=>o.kind!=="run"||!children.has(o.id));
}
export function TrashManagement({projectId,workspaceId,service}:{projectId:string;workspaceId:string;service:TrashService}) {
  return <HistoryBoundary workspaceId={workspaceId} service={service.historyScope}>{scope=><ScopedTrash key={`${projectId}:${scope.id}`} projectId={projectId} workspaceId={workspaceId} service={service} scope={scope}/>}</HistoryBoundary>;
}
function ScopedTrash({projectId,workspaceId,service,scope}:{projectId:string;workspaceId:string;service:TrashService;scope:HistoryScope}) {
  const [preparing,setPreparing]=useState(false);const prepareLock=useRef(false);const alive=useRef(true);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  const key=`annotagent.lifecycle.${workspaceId}.${projectId}.${scope.id}`;
  const [offset,setOffset]=useState(0);
  const [nextOffset,setNextOffset]=useState<number|null>(null);
  const [items,setItems]=useState<TrashEntry[]>();
  const [selected,setSelected]=useState<Set<string>>(new Set());
  const [error,setError]=useState("");
  const [operation,setOperation]=useState<ManagementRequest>();
  const [pending,setPending]=useState<ManagementRequest>();
  const pendingRef=useRef<ManagementRequest | undefined>(undefined);
  const [receipt,setReceipt]=useState<ManagementReceipt>();
  const [generation,setGeneration]=useState(0);
  useEffect(()=>{
    let current=true;setItems(undefined);setSelected(new Set());setError("");
    void service.trash(projectId,undefined,scope.id,offset).then(r=>{if(!current)return;if(!r.page||r.page.offset!==offset||r.items.some(item=>item.project_id!==projectId))throw new Error("回收站范围、分页或对象归属不匹配");setItems(r.items);setNextOffset(r.page.next_offset);}).catch(e=>{if(current)setError(e.message);});
    return()=>{current=false;};
  },[service,projectId,generation,scope.id,offset]);
  useEffect(()=>{
    pendingRef.current=undefined;setPending(undefined);setReceipt(undefined);setOperation(undefined);
    try {const raw=localStorage.getItem(key);if(!raw)return;const saved=JSON.parse(raw) as {request?:ManagementRequest;receipt?:ManagementReceipt};if(saved.request?.project_id===projectId&&saved.request.history_scope===scope.id&&saved.request.confirmation_token&&Array.isArray(saved.request.objects)){pendingRef.current=saved.request;setPending(saved.request);}if(saved.receipt?.project_id===projectId)setReceipt(saved.receipt);}
    catch {setError("本地操作恢复记录无法读取；未执行任何操作。");}
  },[key,projectId]);
  const targets=(items||[]).filter(item=>selected.has(keyOf(item)));
  const open=async(action:"restore"|"purge",entries=targets)=>{
    if(!entries.length||pending||prepareLock.current)return;
    prepareLock.current=true;setPreparing(true);setError("");
    try{const objects=await ownedTrashTargets(entries,projectId,service);if(alive.current)setOperation({project_id:projectId,history_scope:scope.id,objects,action,idempotency_key:crypto.randomUUID()});}
    catch(e){if(alive.current)setError((e as Error).message);}finally{prepareLock.current=false;if(alive.current)setPreparing(false);}
  };
  return <section className="native-project-manager native-history"><h1>项目回收站</h1><p>恢复删除的记录，或显式清理符合条件的记录。原图、已确认标注、模型和凭证不在这里删除。</p><a href={`/projects/${encodeURIComponent(projectId)}/work`}>返回 Agent</a>
    {error&&<p role="alert" className="error">{error}</p>}{preparing&&<p role="status">核对所选批次包含的子运行…</p>}<p>同时选中批次和它的子运行时，按服务器归属仅提交父批次；确认单仍显示实际子运行影响。</p>
    {pending&&<div className="notice"><p>有已提交操作需要核实。刷新不会重复执行。</p><button onClick={()=>setOperation(pending)}>查看原操作</button></div>}
    {receipt&&<section aria-label="管理操作回执"><p role="status">操作状态：{receipt.status} · {receipt.action}</p>{receipt.error&&<p role="alert">{receipt.error}</p>}<Disclosure title="操作记录与清理报告"><p>{receipt.operation_id}</p>{receipt.purge&&<><p>移除 {receipt.purge.database_rows_removed} 条数据库记录、{receipt.purge.files_removed} 个文件；实际回收 {receipt.purge.bytes_reclaimed} bytes</p><p>保留标注 {receipt.purge.retained_annotation_records} 条；调用账本 {receipt.purge.retained_usage_records} 条</p>{receipt.purge.failed_items.map((item,i)=><p key={i}>{item}</p>)}{receipt.purge.retained_reasons.map((reason,i)=><p key={i}>{reason}</p>)}</>}</Disclosure>{!["completed","failed"].includes(receipt.status)&&<button onClick={()=>void service.managementOperation(projectId,receipt.operation_id).then(setReceipt).catch(e=>setError(e.message))}>刷新回执</button>}</section>}
    <div className="actions"><button disabled={preparing} onClick={()=>setGeneration(n=>n+1)}>刷新列表</button><label><input type="checkbox" disabled={preparing} checked={!!items?.length&&targets.length===items.length} onChange={e=>setSelected(new Set(e.target.checked?items?.map(keyOf):[]))}/>全选当前列表</label><span>{targets.length} 项已选</span><button disabled={preparing||!targets.length||!!pending||targets.some(t=>!t.recoverable)} onClick={()=>open("restore")}>恢复选中项…</button><button disabled={preparing||!targets.length||!!pending} onClick={()=>open("purge")}>永久清理选中项…</button></div>
    {!items&&!error&&<p role="status">读取回收站…</p>}{items?.length===0&&<p>新的历史范围内，回收站为空。</p>}
    <div className="actions"><button disabled={preparing||!items||offset===0} onClick={()=>setOffset(Math.max(0,offset-50))}>上一页</button><button disabled={preparing||!items||nextOffset===null} onClick={()=>setOffset(nextOffset!)}>下一页</button></div>
    {items?.map(item=><article className="settings-row" key={keyOf(item)}><label><input type="checkbox" disabled={preparing} aria-label={`选择 ${item.display_name}`} checked={selected.has(keyOf(item))} onChange={e=>setSelected(old=>{const next=new Set(old);if(e.target.checked)next.add(keyOf(item));else next.delete(keyOf(item));return next;})}/><span>{item.display_name}</span></label><div><p>{item.object.kind} · 删除于 {item.deleted_at}</p><p>{item.recoverable?"可申请恢复":"不可恢复"}</p></div><div className="actions"><button disabled={preparing||!!pending||!item.recoverable} onClick={()=>open("restore",[item])}>恢复…</button><button disabled={preparing||!!pending} onClick={()=>open("purge",[item])}>永久清理…</button></div></article>)}
    {operation&&<LifecycleOperation key={operation.idempotency_key} initial={operation} service={service} onClose={()=>setOperation(undefined)} onSubmitted={exact=>{localStorage.setItem(key,JSON.stringify({request:exact,receipt}));pendingRef.current=exact;setPending(exact);}} onReceipt={result=>{setReceipt(result);localStorage.setItem(key,JSON.stringify({request:result.status==="completed"?undefined:pendingRef.current,receipt:result}));if(result.status==="completed"){pendingRef.current=undefined;setPending(undefined);setOperation(undefined);setSelected(new Set());setOffset(0);setGeneration(n=>n+1);}}}/>}
  </section>;
}
