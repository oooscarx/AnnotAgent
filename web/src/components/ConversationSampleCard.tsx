import { useEffect, useRef, useState } from "react";
import { ConversationBudgetNotice } from "./ConversationBudgetNotice";
import {projectBudgetAvailability} from "../projectBudget";
import { api, ApiRequestError, type ConversationSamplePreview, type SampleOperation } from "../api";

export type OpenConversationSample = (draft: string, test: string, image?: string) => void;
const active = (value?: SampleOperation) => !!value && ["queued", "running", "cancelling"].includes(value.status);
const needsUpdate = (value?:SampleOperation)=>active(value) || (value?.status==="succeeded" && (value.assistance===undefined || value.assistance?.status==="waiting"));

/** A task-scoped view of the existing Sample Operation, never another executor. */
export function ConversationSampleCard({project, conversation, task, draft, disabled, onOpen, onAssistance}: {
  project: string; conversation: string; task: string; draft: string; disabled: boolean; onOpen: OpenConversationSample;
  onAssistance?:()=>void;
}) {
  const [operation,setOperation] = useState<SampleOperation>();
  const [preview,setPreview] = useState<ConversationSamplePreview>();
  const [confirmed,setConfirmed] = useState(false);
  const [busy,setBusy] = useState(false);
  const [ready,setReady] = useState(false);
  const [error,setError] = useState("");
  const [uncertain,setUncertain] = useState(false);
  const pending = useRef(false);
  const notified = useRef<string | undefined>(undefined);
  const alive = useRef(true);
  const frozen = useRef<Parameters<typeof api.startSampleOperation>[1] | undefined>(undefined);
  const pendingKey=`annotagent.conversation-sample:${project}:${conversation}:${task}:${draft}`;
  const clearPending=()=>{frozen.current=undefined;try{sessionStorage.removeItem(pendingKey);}catch{/* Server history remains authoritative. */}};
  useEffect(()=>{
    alive.current=true;const controller=new AbortController();
    try{const raw=sessionStorage.getItem(pendingKey);if(raw){const input=JSON.parse(raw) as Parameters<typeof api.startSampleOperation>[1];if(input.draft_id===draft && input.conversation?.task_id===task && input.conversation.conversation_id===conversation){frozen.current=input;setUncertain(true);}}}catch{/* A damaged local envelope cannot authorize anything. */}
    void api.conversationSampleHistory(project,conversation,task,controller.signal).then(({items})=>{
      if(!controller.signal.aborted){const saved=items.find(item=>item.draft_id===draft && (!frozen.current || item.id===frozen.current.request_id));setOperation(saved);if(saved){clearPending();setUncertain(false);}setReady(true);}
    }).catch((error:Error)=>{if(!controller.signal.aborted)setError(error.message);});
    return ()=>{alive.current=false;controller.abort();};
  },[project,conversation,task,draft]);
  useEffect(()=>{
    if(!needsUpdate(operation))return;
    const controller=new AbortController();let timer:ReturnType<typeof setTimeout>;
    const poll=async()=>{
      try{const saved=await api.sampleOperation(project,operation!.id,controller.signal);
        if(controller.signal.aborted)return;
        if(saved.project_id!==project || saved.draft_id!==draft)throw new Error("Sample scope changed");
        setOperation(saved);if(needsUpdate(saved))timer=setTimeout(()=>void poll(),1000);
      }catch(error){if(!controller.signal.aborted){setError((error as Error).message);timer=setTimeout(()=>void poll(),2000);}}
    };
    void poll();return()=>{controller.abort();clearTimeout(timer);};
  },[project,draft,operation?.id,operation?.status,operation?.assistance?.status]);
  useEffect(()=>{
    if(operation?.assistance?.status==="completed" && notified.current!==operation.id){notified.current=operation.id;onAssistance?.();}
  },[operation?.id,operation?.assistance?.status,onAssistance]);
  async function prepare(){
    if(pending.current || disabled || active(operation) || uncertain)return;
    pending.current=true;setBusy(true);setConfirmed(false);setError("");
    try{const value=await api.conversationSamplePreview(project,conversation,task,draft,crypto.randomUUID());
      if(value.project_id!==project)throw new Error("Sample belongs to another Project");
      if(alive.current){setPreview(value);setConfirmed(false);}
    }catch(error){if(alive.current)setError((error as Error).message);}
    finally{pending.current=false;if(alive.current)setBusy(false);}
  }
  async function launch(){
    if(pending.current || disabled || (!frozen.current && (!preview || !confirmed || projectBudgetAvailability(preview.project_call_limit).blocked)))return;
    pending.current=true;setBusy(true);setError("");
    if(!frozen.current && preview){const budget=preview.conversation_budget;
      frozen.current={request_id:preview.request_id,draft_id:draft,expected_revision:preview.revision,image_indices:Array.from({length:preview.image_count},(_,i)=>i),authorization_fingerprint:preview.authorization_fingerprint,conversation:{conversation_id:conversation,task_id:task,previous_grant_id:budget.previous_grant_id,scope_hash:budget.scope_hash,expires_at:budget.expires_at,allow_unknown_cost:true,human_review:true}};
    }
    try{sessionStorage.setItem(pendingKey,JSON.stringify(frozen.current));}catch{/* Explicit retry in this mounted view still uses the frozen envelope. */}
    try{const saved=await api.startSampleOperation(project,frozen.current!);
      if(alive.current){setOperation(saved);setPreview(undefined);setUncertain(false);clearPending();}
    }catch(error){if(alive.current){setError((error as Error).message);
      try{const saved=await api.sampleOperation(project,frozen.current!.request_id);
        if(alive.current){setOperation(saved);setPreview(undefined);setUncertain(false);clearPending();}
      }catch{if(alive.current){if(error instanceof ApiRequestError && [400,401,403,404,409,422,429].includes(error.status)){clearPending();setPreview(undefined);setUncertain(false);}else setUncertain(true);}}
    }}finally{pending.current=false;if(alive.current)setBusy(false);}
  }
  async function stop(){if(!operation)return;try{const saved=await api.cancelSampleOperation(project,operation.id);if(alive.current)setOperation(saved);}catch(error){if(alive.current)setError((error as Error).message);}}
  return <section className="conversation-sample-card" aria-label="Test annotation samples">
    <h3>Try this plan on your images</h3><p>Sample results and corrections stay in the evaluation sandbox. This does not publish or accept dataset annotations.</p>
    {!active(operation) && !preview && !uncertain && <button disabled={disabled || busy || !ready} onClick={()=>void prepare()}>Review sample authorization</button>}
    {preview && <div className="conversation-consent" aria-label="Sample model authorization">
      <ConversationBudgetNotice value={preview.project_call_limit} maximumCalls={preview.request_limit} busy={busy||uncertain} onRefresh={()=>void prepare()}/>
      <p>Draft revision {preview.revision} · {preview.image_count} images · Up to {preview.request_limit} model calls</p>
      <p>Results needing human judgment can create a saved request here. This does not accept annotations or authorize another model call.</p>
      <ul>{preview.models.map(model=><li key={model.id}>{model.name} · {model.destination}</li>)}</ul>
      <p>{preview.conversation_budget.used_calls} calls already used · Cumulative limit {preview.conversation_budget.maximum_calls} · Cost unknown</p>
      {(!preview.supported || !preview.image_count) && <p role="status">{!preview.image_count ? "Upload images before testing." : "This Draft needs compatible model bindings before bounded testing."}</p>}
      <label><input type="checkbox" checked={confirmed} disabled={busy || uncertain} onChange={event=>setConfirmed(event.target.checked)}/>Allow these sample images to be sent to the listed models; actual cost is unknown</label>
      <div className="button-row"><button disabled={busy || uncertain} onClick={()=>setPreview(undefined)}>Back</button><button className="primary" disabled={busy || disabled || !confirmed || !preview.supported || !preview.image_count || projectBudgetAvailability(preview.project_call_limit).blocked} onClick={()=>void launch()}>Test these samples</button></div>
    </div>}
    {active(operation) && <><p role="status">Sample task: {operation!.status}</p><button disabled={operation!.status==="cancelling"} onClick={()=>void stop()}>Stop sample test</button><p>Leaving does not stop the task. An in-flight remote request may still be billed.</p></>}
    {operation?.status==="succeeded" && operation.assistance?.status==="waiting" && <p role="status">Preparing saved requests for human judgment… No additional inference is running.</p>}
    {operation?.assistance?.status==="failed" && <p role="alert">Sample report saved, but human-request preparation failed: {operation.assistance.error}. You can still inspect and correct the saved sample.</p>}
    {operation && !active(operation) && <div className="conversation-builder-result"><strong>{operation.status==="succeeded" ? "Sample report saved" : `Sample task: ${operation.status}`}</strong>{operation.error && <p role="alert">{operation.error}</p>}{operation.status==="succeeded" && <><small>Open the report to inspect results, quality risks and any failed nodes.</small><button onClick={()=>onOpen(draft,operation.id)}>View sample results in canvas</button></>}</div>}
    {uncertain && <button disabled={busy || disabled} onClick={()=>void launch()}>Retry the same sample request</button>}
    {error && <p role="alert">{error} Saved tasks remain on the server. Reloading never starts a test.</p>}
  </section>;
}
