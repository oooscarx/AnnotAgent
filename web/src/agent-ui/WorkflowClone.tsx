import {useEffect,useRef,useState} from "react";
import type {api} from "../api";
import type {ExactCloneRequest,FrozenWorkflowVersion,WorkflowDraft} from "../types";
import {Dialog} from "./Dialog";
import {agentPath} from "./navigationContract";
export type WorkflowCloneService=Pick<typeof api,"cloneExactWorkflowVersion"|"workflowDraft">;
export function cloneRequest(source:FrozenWorkflowVersion,commandId:string):ExactCloneRequest {
  if(!source.content_hash||!source.project_id||!source.workflow_id||!Number.isSafeInteger(source.version)||source.version<1)throw new Error("冻结来源缺少稳定身份");
  return {command_id:commandId,project_id:source.project_id,source_snapshot_hash:source.content_hash};
}
export function WorkflowClone({source,workspaceId,service}:{source:FrozenWorkflowVersion;workspaceId:string;service:WorkflowCloneService}) {
  const key=`annotagent.workflow-clone.${workspaceId}.${source.project_id}.${source.workflow_id}.${source.version}`;
  const [restoreBlocked,setRestoreBlocked]=useState(false);
  const [pending,setPending]=useState<ExactCloneRequest>();const [confirm,setConfirm]=useState<ExactCloneRequest>();const [result,setResult]=useState<WorkflowDraft>();const [error,setError]=useState("");const [busy,setBusy]=useState(false);const lock=useRef(false);const live=useRef(false);
  useEffect(()=>{live.current=true;try{const raw=localStorage.getItem(key);if(raw){const p=JSON.parse(raw) as ExactCloneRequest;if(p.project_id!==source.project_id||p.source_snapshot_hash!==source.content_hash||typeof p.command_id!=="string"||!p.command_id)throw new Error("复制恢复记录与当前冻结版本不匹配");setPending(p);}}catch(e){setRestoreBlocked(true);setError((e as Error).message);}return()=>{live.current=false;};},[key,source.project_id,source.content_hash]);
  const send=async(command:ExactCloneRequest)=>{
    if(lock.current)return;lock.current=true;setBusy(true);setError("");
    try{localStorage.setItem(key,JSON.stringify(command));setPending(command);setConfirm(undefined);
      const receipt=await service.cloneExactWorkflowVersion(source.workflow_id,source.version,command);
      if(receipt.project_id!==source.project_id||receipt.id===source.source_draft_id||!receipt.id)throw new Error("复制回执身份不匹配");
      // Receipt is the original creation result. Never write it over later human edits.
      const current=await service.workflowDraft(source.project_id,receipt.id);
      if(current.id!==receipt.id||current.project_id!==source.project_id)throw new Error("复制后读取的草稿归属不匹配");
      try{localStorage.removeItem(key);}catch{/* Server receipt remains authoritative. */}
      if(live.current){setResult(current);setPending(undefined);}
    }catch(e){if(live.current)setError(`${(e as Error).message} 不会自动重试、创建另一个副本或恢复已删除草稿；已发出的请求保留原命令用于核实。`);}finally{lock.current=false;if(live.current)setBusy(false);}
  };
  return <section aria-label="复制 Workflow 版本"><h2>复制为可编辑草稿</h2><p>只复制当前冻结版本；不改默认方案，不发布，不试跑或调用模型。</p>{error&&<p role="alert">{error}</p>}
    {result?<p role="status">副本已确认：{result.name} · revision {result.revision}。<a href={agentPath({kind:"detail",projectId:source.project_id,page:"pipelines",objectId:result.id})}>打开当前副本</a></p>:pending?<><p role="status">复制结果待核实。沿用原 command 和来源快照，不会重复创建；若副本已删除，需到回收站处理。</p><button disabled={busy} onClick={()=>void send(pending)}>核实原复制请求</button></>:<button disabled={busy||restoreBlocked} onClick={()=>setConfirm(cloneRequest(source,crypto.randomUUID()))}>复制此版本…</button>}
    {confirm&&<Dialog title="确认复制冻结版本" onClose={()=>setConfirm(undefined)}><p>{source.draft.name} · v{source.version}</p><p>执行快照 Hash：{confirm.source_snapshot_hash}</p><p>将在当前项目创建一个独立 Draft。已有 Published Version、默认方案和标注保持不变。</p><div className="actions"><button onClick={()=>setConfirm(undefined)}>取消</button><button disabled={busy} onClick={()=>void send(confirm)}>确认复制</button></div></Dialog>}
  </section>;
}
