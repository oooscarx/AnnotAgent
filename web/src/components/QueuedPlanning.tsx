import {useEffect,useRef,useState} from "react";
import {api,ApiRequestError} from "../api";
import {parseQueueConsent,queuePlanningApi,type QueueConsent,type QueuePreview} from "../conversation-queue-api";
import type {ConversationCallReceipt,ConversationSchemaDraft} from "../types";

/** One explicit text-only step, not acceptance of the entire instruction. */
export function QueuedPlanning({project,conversation,task,message,cancelled,onClose}:{project:string;conversation:string;task:string;message:string;cancelled:boolean;onClose:()=>void}){
  const key=`annotagent.queued-plan:${project}:${conversation}:${task}:${message}`;
  const [preview,setPreview]=useState<QueuePreview>(),[consent,setConsent]=useState<QueueConsent>(),[receipt,setReceipt]=useState<ConversationCallReceipt>(),[draft,setDraft]=useState<ConversationSchemaDraft>();
  const [ready,setReady]=useState(false),[confirmed,setConfirmed]=useState(false),[sending,setSending]=useState(false),[error,setError]=useState(""),[stopping,setStopping]=useState(false);
  const frozen=useRef<QueueConsent|undefined>(undefined),alive=useRef(false),pending=useRef(false),stopPending=useRef(false),generation=useRef(0);
  const keep=(value:QueueConsent)=>{frozen.current=value;setConsent(value);};
  async function restore(signal?:AbortSignal){
    const ticket=++generation.current;
    const saved=await queuePlanningApi.authorization(project,conversation,task,message,signal);
    if(!alive.current||signal?.aborted||ticket!==generation.current)return;
    if(saved){
      keep(saved);
      const calls=await api.conversationSchemaCancellations(project,conversation,task,signal);
      if(!alive.current||signal?.aborted||ticket!==generation.current)return;
      setStopping(stopPending.current||calls.some(call=>call.call_id===saved.call_id));
      const found=await queuePlanningApi.receipt(project,conversation,task,saved.call_id,signal).catch(reason=>{if(reason instanceof ApiRequestError&&reason.status===404)return undefined;throw reason;});
      if(!alive.current||signal?.aborted||ticket!==generation.current)return;
      if(found){setReceipt(found);if(found.status==="completed"){
        const result=await api.conversationSchemaDraftForCall(project,conversation,task,saved.call_id,signal);
        if(alive.current&&!signal?.aborted&&ticket===generation.current)setDraft(result??undefined);
      }}
    }
    if(alive.current&&!signal?.aborted)setReady(true);
  }
  useEffect(()=>{
    alive.current=true;const controller=new AbortController();let timer:ReturnType<typeof setTimeout>;
    try{const local=parseQueueConsent(sessionStorage.getItem(key));if(local)keep(local);}catch{/* Server remains authoritative. */}
    async function poll(){try{await restore(controller.signal);}catch(reason){if(alive.current&&!controller.signal.aborted)setError((reason as Error).message);}if(!controller.signal.aborted)timer=setTimeout(()=>void poll(),2000);}
    void poll();return()=>{alive.current=false;generation.current++;controller.abort();clearTimeout(timer);};
  },[key]);
  async function review(){setError("");try{const value=await queuePlanningApi.preview(project,conversation,task,message);if(alive.current){setPreview(value);setConfirmed(false);}}catch(reason){if(alive.current)setError((reason as Error).message);}}
  async function submit(){
    if(cancelled||pending.current||!ready||(!frozen.current&&(!preview||!confirmed)))return;
    const value=frozen.current??{call_id:crypto.randomUUID(),model_id:preview!.model_id,scope_hash:preview!.scope_hash,request_hash:preview!.request_hash,previous_grant_id:preview!.previous_grant_id,maximum_calls:preview!.maximum_calls,expires_at:preview!.expires_at,allow_unknown_cost:true};
    try{sessionStorage.setItem(key,JSON.stringify(value));}catch{setError("Cannot preserve the request identity in this browser. No request was sent.");return;}
    keep(value);pending.current=true;setSending(true);setError("");
    try{await queuePlanningApi.propose(project,conversation,task,message,value);if(alive.current)await restore();}
    catch(reason){if(alive.current){setError((reason as Error).message);try{await restore();}catch{/* Preserve same consent for explicit recovery. */}}}
    finally{pending.current=false;if(alive.current)setSending(false);}
  }
  async function stop(){if(!consent||stopPending.current)return;stopPending.current=true;setStopping(true);setError("");try{await api.cancelConversationSchema(project,conversation,task,consent.call_id);if(alive.current)await restore();}catch(reason){if(alive.current){setError((reason as Error).message);setStopping(false);}}finally{stopPending.current=false;}}
  const active=receipt?.status==="reserved"||sending;
  return <section className="agent-queued-planning" aria-label="Queued text planning">
    <header><h3>Plan this supplement</h3><button type="button" onClick={onClose}>Close planning details</button></header>
    <p>This step proposes annotation semantics only. It does not revise the Workflow or run images.</p>
    {!ready&&<p role="status">Reading saved authorization…</p>}
    {error&&(receipt?<details><summary>Last request error · saved server state shown below</summary><p>{error}</p></details>:<p role="alert">{error}</p>)}
    {cancelled&&<p>Instruction cancelled. Saved results remain readable; no new request can start.</p>}
    {!cancelled&&!consent&&ready&&!preview&&<button type="button" onClick={()=>void review()}>Review text planning scope</button>}
    {preview&&!consent&&<><dl><dt>Agent model</dt><dd>{preview.model_name}</dd><dt>Data destination</dt><dd>{preview.destination}</dd><dt>Data sent</dt><dd>{preview.data_scope}</dd><dt>Limits</dt><dd>{preview.new_request_limit} new text request · {preview.used_calls} calls already used · cumulative ceiling {preview.maximum_calls}</dd><dt>Cost</dt><dd>Unknown · not zero</dd></dl><p>{preview.operation}</p><label><input type="checkbox" checked={confirmed} onChange={event=>setConfirmed(event.target.checked)}/>Allow this text-only call with unknown Provider cost</label></>}
    {consent&&<><p>Saved authorization · cumulative ceiling {consent.maximum_calls} calls. Image processing is not authorized.</p><details><summary>Authorization details</summary><dl><dt>Model Profile</dt><dd>{consent.model_id}</dd><dt>Call</dt><dd>{consent.call_id}</dd><dt>Expiry</dt><dd>{consent.expires_at}</dd></dl></details></>}
    {active&&<><p role="status">{stopping?"Stopping · waiting for the server to settle":"Text planning is running. Leaving this view does not cancel it."}</p><button type="button" disabled={stopping||!consent} onClick={()=>void stop()}>Stop this planning call</button></>}
    {!cancelled&&!active&&!receipt&&ready&&(preview||consent)&&<button type="button" className="primary" disabled={!consent&&!confirmed} onClick={()=>void submit()}>{consent?"Retry same authorized request":"Authorize text planning"}</button>}
    {receipt&&receipt.status!=="reserved"&&<p role="status">{receipt.status==="completed"?"Planning response saved · the full instruction is not yet completed":receipt.status==="in_doubt"?"Remote completion and cost are unknown. No automatic retry.":"Planning call ended without a successful result."}</p>}
    {receipt?.evidence?.error&&<p>{receipt.evidence.error}</p>}
    {receipt?.evidence?.decision?.Err&&<p role="alert">{receipt.evidence.decision.Err}</p>}
    {receipt?.evidence?.decision?.Ok?.question&&<p>{receipt.evidence.decision.Ok.question}</p>}
    {draft&&<div className="agent-queued-draft"><h4>Saved semantic Draft · revision {draft.revision}</h4><p>{draft.definition.task.kind==="classification"?"Image categories":"Object boxes"} · {draft.definition.task.labels.join(", ")}</p><ul>{draft.definition.boundary_rules.map((rule,index)=><li key={index}>{rule}</li>)}</ul><small>No Workflow was published or activated.</small><details><summary>Draft reference</summary><p>{draft.id}</p></details></div>}
  </section>;
}
