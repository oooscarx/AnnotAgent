import {useEffect,useRef,useState} from "react";
import type {api} from "../api";
import type {ConversationCallReceipt,ConversationSchemaDraft} from "../types";
import {Disclosure} from "./Disclosure";
type Service=Pick<typeof api,"conversationSchemaDraftForCall"|"saveConversationSchemaDraft">;
export function ownedProposalDraft(draft:ConversationSchemaDraft,task:string,call:string){
  if(draft.task_id!==task||draft.source_call_id!==call)throw new Error("建议草稿回执不属于当前任务与原调用");
  return draft;
}
export async function saveOwnedProposal(service:Service,project:string,conversation:string,task:string,call:string){
  const saved=await service.conversationSchemaDraftForCall(project,conversation,task,call);
  return ownedProposalDraft(saved||await service.saveConversationSchemaDraft(project,conversation,task,call),task,call);
}
export function SchemaProposal({service,project,conversation,task,call,onSaved}:{service:Service;project:string;conversation:string;task:string;call:ConversationCallReceipt;onSaved:(draft:ConversationSchemaDraft)=>void}){
  const [busy,setBusy]=useState(false);const [error,setError]=useState("");const lock=useRef(false);const alive=useRef(true);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  const decision=call.evidence?.decision?.Ok;
  if(call.task_id!==task||call.status!=="completed"||decision?.decision!=="draft")return null;
  const save=async()=>{if(lock.current)return;lock.current=true;setBusy(true);setError("");try{const saved=await saveOwnedProposal(service,project,conversation,task,call.id);if(alive.current)onSaved(saved);}catch(e){if(alive.current)setError((e as Error).message);}finally{lock.current=false;if(alive.current)setBusy(false);}};
  return <Disclosure title={`尚未保存为草稿的建议 · ${decision.labels?.join("、")||"标签语义"}`}><section aria-label="已保存的模型建议">
    <p>{decision.rationale}</p><p>{decision.kind==="bounding_box"?"框出目标":decision.kind==="classification"?"整图分类":decision.kind}</p>
    <ul>{decision.labels?.map((label,i)=><li key={i}>{label}</li>)}</ul>{!!decision.boundary_rules?.length&&<ul aria-label="建议边界规则">{decision.boundary_rules.map((rule,i)=><li key={i}>{rule}</li>)}</ul>}
    <p>使用原调用已保存的建议创建可编辑草稿，不重新调用模型、不发布、不试跑。</p>
    {error&&<p role="alert">{error}。结果待核实；刷新只读取服务器状态，再次操作会先查找原调用的草稿。</p>}
    <button disabled={busy} onClick={()=>void save()}>{busy?"保存并核实…":error?"核实并保存原建议":"保存建议为草稿"}</button>
  </section></Disclosure>;
}
