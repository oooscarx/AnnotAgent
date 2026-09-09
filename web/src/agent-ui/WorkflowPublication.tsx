import {useEffect,useRef,useState} from "react";
import {ApiRequestError,type api} from "../api";
import type {WorkflowDraft,ExactPublicationRequest,FrozenWorkflowVersion} from "../types";
import {Dialog} from "./Dialog";
import {verifyFrozenVersion,versionLink} from "./WorkflowVersionDetail";
export type PublicationService=Pick<typeof api,"publishExactWorkflow">;
export function publicationRequest(draft:WorkflowDraft,commandId:string):ExactPublicationRequest {
  if(!draft.project_id||!draft.id||!draft.content_hash||!Number.isSafeInteger(draft.revision)||draft.revision<1)throw new Error("草稿缺少稳定身份或版本");
  return {command_id:commandId,project_id:draft.project_id,expected_revision:draft.revision,expected_content_hash:draft.content_hash};
}
export function WorkflowPublication({draft,workspaceId,disabled,service,onPublished}:{draft:WorkflowDraft;workspaceId:string;disabled:boolean;service:PublicationService;onPublished:(draft:WorkflowDraft)=>void}) {
  const key=`annotagent.publication.${workspaceId}.${draft.project_id}.${draft.id}`;
  const [confirm,setConfirm]=useState<ExactPublicationRequest>();const [pending,setPending]=useState<ExactPublicationRequest>();const [result,setResult]=useState<FrozenWorkflowVersion>();const [error,setError]=useState("");const [busy,setBusy]=useState(false);const lock=useRef(false);const live=useRef(false);
  useEffect(()=>{live.current=true;setPending(undefined);setResult(undefined);setError("");try{const raw=localStorage.getItem(key);if(raw){const r=JSON.parse(raw) as ExactPublicationRequest;if(r.project_id!==draft.project_id||!r.command_id||!r.expected_content_hash||!Number.isSafeInteger(r.expected_revision))throw new Error("发布恢复记录不匹配");setPending(r);}}catch(e){setError((e as Error).message);}return()=>{live.current=false;};},[key,draft.project_id]);
  const send=async(exact:ExactPublicationRequest)=>{
    if(lock.current)return;lock.current=true;setBusy(true);setError("");
    try{
      localStorage.setItem(key,JSON.stringify(exact));setPending(exact);setConfirm(undefined);
      const r=await service.publishExactWorkflow(draft.id,exact);verifyFrozenVersion(r,draft.project_id,draft.id,r.version);
      if(r.draft.revision!==exact.expected_revision||r.draft.content_hash!==exact.expected_content_hash)throw new Error("发布回执不属于确认的草稿版本，请保留原请求核实");
      localStorage.removeItem(key);if(live.current){setResult(r);setPending(undefined);onPublished(r.draft);}
    }catch(e){if(live.current){
      // These explicit conflicts reject this scope; uncertain network results retain the original command.
      if(e instanceof ApiRequestError&&["workflow_draft_revision_conflict","workflow_draft_content_conflict"].includes(e.code||"")){localStorage.removeItem(key);setPending(undefined);}
      setError(`${(e as Error).message} 没有自动重试、重新试跑或启动 Run。`);
    }}finally{lock.current=false;if(live.current)setBusy(false);}
  };
  return <section aria-label="发布 Workflow"><h2>发布不可变版本</h2><p>仅发布当前已保存草稿，并按现有发布规则设为项目默认方案。服务器检查样本证据、模型及安全约束；不会重新试跑、启动数据集或自动接受标注。</p>
    {error&&<p role="alert">{error}</p>}{result?<p role="status">已发布 v{result.version}。<a href={versionLink(draft.project_id,`${result.workflow_id}@${result.version}`)}>查看冻结版本</a></p>:pending?<><p role="status">发布结果待核实。刷新不会重复发布；核实只重放原 command、revision 和 hash。</p><button disabled={busy} onClick={()=>void send(pending)}>{busy?"核实中…":"核实原发布请求"}</button></>:<button disabled={disabled||busy||["published","archived"].includes(draft.status)} onClick={()=>{try{setConfirm(publicationRequest(draft,crypto.randomUUID()));}catch(e){setError((e as Error).message);}}}>预览发布确认…</button>}
    {confirm&&<Dialog title="确认发布当前草稿" onClose={()=>setConfirm(undefined)}><p>{draft.name} · revision {confirm.expected_revision}</p><p>草稿 Hash：{confirm.expected_content_hash}</p><p>操作范围：发布不可变版本并更新本项目默认方案。不创建 Run，不调用模型，不修改任何已有 Published Version。</p><div className="actions"><button onClick={()=>setConfirm(undefined)}>取消</button><button disabled={disabled||busy} onClick={()=>void send(confirm)}>确认发布</button></div></Dialog>}
  </section>;
}
