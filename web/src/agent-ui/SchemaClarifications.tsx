import {useEffect,useRef,useState} from "react";
import type {api} from "../api";
import type {ConversationSchemaDraft} from "../types";
import {CreateTaskSchema} from "./CreateTaskSchema";
type Service=Pick<typeof api,"conversationSchemaCalls"|"conversationSchemaClarification"|"cancelSchemaClarification"|"saveHumanConversationSchema">;
type Item=Awaited<ReturnType<Service["conversationSchemaClarification"]>>;
export function ownedClarification(item:Item,conversation:string,task:string,call:string){
  if(item.id!==call||item.task_id!==task||item.conversation_id!==conversation||!item.expected_schema_revision||!["pending","applied","cancelled"].includes(item.status))throw new Error("澄清记录归属或状态不匹配");return item;
}
export function SchemaClarifications({service,project,conversation,task,workspace,onSaved}:{service:Service;project:string;conversation:string;task:string;workspace:string;onSaved:(draft:ConversationSchemaDraft)=>void}){
  const [items,setItems]=useState<Item[]>();const [error,setError]=useState("");
  useEffect(()=>{const c=new AbortController();void (async()=>{const calls=await service.conversationSchemaCalls(project,conversation,task,c.signal);if(calls.some(v=>v.task_id!==task))throw new Error("澄清调用不属于当前任务");const rows=await Promise.all(calls.filter(v=>v.status==="completed"&&v.evidence?.decision?.Ok?.decision==="clarify").map(async v=>ownedClarification(await service.conversationSchemaClarification(project,conversation,task,v.id,c.signal),conversation,task,v.id)));if(!c.signal.aborted)setItems(rows);})().catch(e=>{if(!c.signal.aborted)setError((e as Error).message);});return()=>c.abort();},[service,project,conversation,task]);
  return <section aria-label="任务澄清问题">{error&&<p role="alert">{error}</p>}{!items&&!error&&<p role="status">读取澄清问题…</p>}{items?.map(item=><Clarification key={item.id} {...{service,project,conversation,task,workspace,item,onSaved}}/>)}</section>;
}
function Clarification({service,project,conversation,task,workspace,item,onSaved}:{service:Service;project:string;conversation:string;task:string;workspace:string;item:Item;onSaved:(draft:ConversationSchemaDraft)=>void}){
  const [current,setCurrent]=useState(item);const [error,setError]=useState("");const [busy,setBusy]=useState(false);const [unknown,setUnknown]=useState(false);const lock=useRef(false);const alive=useRef(true);
  const [editing,setEditing]=useState(false);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  const read=async()=>ownedClarification(await service.conversationSchemaClarification(project,conversation,task,item.id),conversation,task,item.id);
  const cancel=async()=>{if(lock.current)return;if(!window.confirm("取消这个未回答的问题？不会删除已有模型证据，也不会退还已用调用。"))return;lock.current=true;setBusy(true);setError("");try{const latest=await read();if(latest.expected_schema_revision!==item.expected_schema_revision)throw new Error("澄清版本变化，未取消");if(latest.status==="pending")await service.cancelSchemaClarification(project,conversation,task,{call_id:item.id,expected_schema_revision:item.expected_schema_revision});const saved=await read();if(alive.current){setCurrent(saved);setUnknown(false);}}catch(e){if(alive.current){setError((e as Error).message);setUnknown(true);}}finally{lock.current=false;if(alive.current)setBusy(false);}};
  return <article><h3>{current.question}</h3><p role="status">{current.status==="pending"?"等待回答":current.status==="applied"?"回答已保存；不会自动续跑":"问题已取消"}</p>{error&&<p role="alert">{error}。操作结果待核实，没有自动重试。</p>}
    {current.status==="pending"&&<fieldset disabled={busy||unknown}><CreateTaskSchema {...{service,project,conversation,task,workspace}} onActive={setEditing} clarification={{call_id:item.id,expected_schema_revision:item.expected_schema_revision}} onSaved={draft=>{setEditing(false);setCurrent({...current,status:"applied",schema_draft_id:draft.id});onSaved(draft);}}/></fieldset>}
    {current.status==="pending"&&<button disabled={busy||editing} onClick={()=>void cancel()}>取消此澄清问题</button>}
    <button disabled={busy||editing} onClick={async()=>{setBusy(true);try{const saved=await read();if(alive.current){setCurrent(saved);setUnknown(false);setError("");}}catch(e){if(alive.current)setError((e as Error).message);}finally{if(alive.current)setBusy(false);}}}>读取澄清最新状态</button>
  </article>;
}
