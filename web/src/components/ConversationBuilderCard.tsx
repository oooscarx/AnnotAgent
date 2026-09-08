import { useEffect, useRef, useState } from "react";
import { api, ApiRequestError } from "../api";
import { projectBuildPath } from "../navigation";
import { ConversationSampleCard, type OpenConversationSample } from "./ConversationSampleCard";
import type { ConversationBuilderConsent, ConversationBuilderItem, ConversationBuilderPreview, ConversationSchemaDraft } from "../types";

/** Restoring history only reads. Model work requires a new, explicit consent. */
export function ConversationBuilderCard({ project, conversation, task, schema, editing, onSample, repairRequest, onAssistance }: {
  project: string; conversation: string; task: string; schema: Pick<ConversationSchemaDraft,"id"|"revision">; editing: boolean; onAssistance?:()=>void; onSample: OpenConversationSample;
  repairRequest?: {id:string;draft:string};
}) {
  const [item,setItem]=useState<ConversationBuilderItem>();
  const [preview,setPreview]=useState<ConversationBuilderPreview>();
  const [confirmed,setConfirmed]=useState(false);
  const [ready,setReady]=useState(false);
  const [busy,setBusy]=useState(false);
  const [uncertain,setUncertain]=useState(false);
  const [cancelled,setCancelled]=useState(false);
  const [error,setError]=useState("");
  const pending=useRef(false);
  const alive=useRef(true);
  const frozen=useRef<ConversationBuilderConsent | undefined>(undefined);
  const running=item?.operation.status==="reserved" || Boolean(frozen.current && (busy || uncertain));
  useEffect(()=>{
    alive.current=true; const controller=new AbortController();
    void api.conversationBuilderHistory(project,conversation,task,controller.signal).then((history)=>{
      if(!controller.signal.aborted) { setItem(history.items.find(entry=>repairRequest ? entry.session?.working_draft?.draft_id===repairRequest.draft : entry.session?.working_draft?.build_mode.kind!=="repair_draft"));setReady(true); }
    }).catch((error:Error)=>{if(!controller.signal.aborted)setError(error.message);});
    return ()=>{alive.current=false;controller.abort();};
  },[project,conversation,task,repairRequest?.draft]);
  useEffect(()=>{
    if(!running)return;
    const controller=new AbortController(); let fetching=false;
    const poll=async()=>{
      if(fetching)return; fetching=true;
      try {
        const history=await api.conversationBuilderHistory(project,conversation,task,controller.signal);
        const id=frozen.current?.selection.operation_id ?? item?.operation.id;
        const current=history.items.find((entry)=>entry.operation.id===id);
        if(!controller.signal.aborted && current) { setItem(current); if(current.operation.status!=="reserved") {setUncertain(false);frozen.current=undefined;} }
      } catch(error) {if(!controller.signal.aborted)setError((error as Error).message);}
      finally {fetching=false;}
    };
    void poll(); const timer=window.setInterval(()=>void poll(),1200);
    return ()=>{controller.abort();window.clearInterval(timer);};
  },[running,project,conversation,task,item?.operation.id]);
  async function prepare() {
    if(pending.current || running || editing)return; pending.current=true;setBusy(true);setError("");
    try {
      const result=await api.conversationBuilderPreview(project,conversation,task,{operation_id:crypto.randomUUID(),schema_id:schema.id,schema_revision:schema.revision,repair_request_id:repairRequest?.id});
      if(alive.current){setPreview(result);setConfirmed(false);setCancelled(false);}
    } catch(error) {if(alive.current)setError((error as Error).message);}
    finally {pending.current=false;if(alive.current)setBusy(false);}
  }
  async function launch() {
    if(pending.current || !preview || !confirmed || editing)return;
    if(preview.selection.schema_revision!==schema.revision){setError("Labels changed. Review a fresh Builder authorization before continuing.");setPreview(undefined);return;}
    pending.current=true;setBusy(true);setError("");setItem(undefined);
    frozen.current ??= {selection:preview.selection,repair:preview.repair,scope_hash:preview.scope_hash,previous_grant_id:preview.previous_grant_id,expires_at:preview.expires_at,allow_unknown_cost:true};
    try {
      const operation=await api.launchConversationBuilder(project,conversation,task,frozen.current);
      const history=await api.conversationBuilderHistory(project,conversation,task);
      if(alive.current){setItem(history.items.find((entry)=>entry.operation.id===operation.id) ?? {operation});setPreview(undefined);setUncertain(false);frozen.current=undefined;}
    } catch(error) {if(alive.current){
      setError((error as Error).message);
      const id=frozen.current?.selection.operation_id;
      try {
        const history=await api.conversationBuilderHistory(project,conversation,task);
        if(!alive.current)return;
        const saved=history.items.find((entry)=>entry.operation.id===id);
        if(saved){setItem(saved);setUncertain(saved.operation.status==="reserved");if(saved.operation.status!=="reserved"){frozen.current=undefined;setPreview(undefined);}}
        else if(error instanceof ApiRequestError && [400,401,403,404,409,422,429].includes(error.status)){setUncertain(false);setPreview(undefined);frozen.current=undefined;}
        else setUncertain(Boolean(frozen.current));
      } catch {if(alive.current)setUncertain(Boolean(frozen.current));}
    }}
    finally {pending.current=false;if(alive.current)setBusy(false);}
  }
  async function stop() {
    const id=frozen.current?.selection.operation_id ?? item?.operation.id;if(!id)return;
    try {await api.cancelConversationSchema(project,conversation,task,id);if(alive.current)setCancelled(true);}
    catch(error){if(alive.current)setError((error as Error).message);}
  }
  const session=item?.session;
  const draftId=item?.operation.evidence?.draft_id ?? session?.working_draft?.draft_id;
  const builtRevision=preview?.selection.schema_revision ?? item?.schema_revision ?? item?.operation.evidence?.schema_revision;
  return <section className="conversation-builder-card" aria-label={repairRequest ? "Repair annotation pipeline" : "Build annotation pipeline"}>
    <h3>{repairRequest ? "Revise the plan from your correction" : "Build the annotation plan"}</h3>
    {repairRequest && <p>Your saved correction is evidence for revising this plan, not proof of improved accuracy. The original plan remains unchanged.</p>}
    <p>{builtRevision ? `This operation uses Schema revision ${builtRevision}.` : `A new build will use saved labels at revision ${schema.revision}.`} This step builds a Draft; it does not test images or publish.</p>
    {builtRevision && builtRevision!==schema.revision && <p role="status">Labels are now revision {schema.revision}; this saved operation has not been rebuilt for those changes.</p>}
    {!running && !preview && <button disabled={!ready || busy || editing} onClick={()=>void prepare()}>{item ? "Review another build request" : "Review Builder authorization"}</button>}
    {preview && !running && <div className="conversation-consent" aria-label="Builder model authorization"><strong>{preview.model_name}</strong><span>{preview.remote_model} · {preview.destination}</span><p>{preview.data_scope}</p><p>Up to {preview.maximum_builder_calls} text calls · {preview.used_calls} calls already used · Cumulative limit {preview.maximum_calls} · Cost unknown</p><p>{preview.operation}</p><label><input type="checkbox" checked={confirmed} onChange={(event)=>setConfirmed(event.target.checked)} />Allow this bounded Builder request; actual cost is unknown</label><div className="button-row"><button onClick={()=>setPreview(undefined)}>Back</button><button className="primary" disabled={!confirmed || busy || editing} onClick={()=>void launch()}>Build Pipeline Draft</button></div></div>}
    {running && <><p role="status">{session?.phase ? `Builder stage: ${session.phase.replaceAll("_"," ")}` : "Submitting or restoring the saved Builder operation…"}</p>{session?.next_action && <p>{session.next_action}</p>}<button onClick={()=>void stop()}>Stop Builder</button><small>Leaving this page does not stop the server task. No images are being tested.</small></>}
    {cancelled && <p role="status">{running ? "Cancellation saved. Waiting for the server to settle any in-flight call; its cost may be unknown." : "Cancellation saved. The operation has stopped; any prior call cost remains recorded separately."}</p>}
    {uncertain && !busy && <button onClick={()=>void launch()} disabled={cancelled}>Retry the same Builder request</button>}
    {item && item.operation.status!=="reserved" && <div className="conversation-builder-result"><strong>{item.operation.status==="interrupted" ? "Build interrupted" : "Builder outcome saved"}</strong><p>{session?.outcome?.replaceAll("_"," ") ?? item.operation.evidence?.outcome?.replaceAll("_"," ") ?? item.operation.status}</p>{item.operation.evidence?.error && <p>{item.operation.evidence.error}</p>}{session?.next_action && <p>{session.next_action}</p>}{session?.unresolved_bindings?.length ? <ul>{session.unresolved_bindings.map((binding,index)=><li key={index}>{binding}</li>)}</ul> : null}{draftId && <a href={projectBuildPath(project,"pipeline",{draftId,agentSessionId:session?.id})}>Open saved Pipeline details</a>}<small>No sample result or formal annotation was accepted.</small></div>}
    {error && <p role="alert">{error} Saved operations remain on the server; refreshing will not start another build.</p>}
    {draftId && !running && <ConversationSampleCard key={`${task}:${draftId}`} project={project} conversation={conversation} task={task} draft={draftId} disabled={editing || busy} onOpen={onSample} onAssistance={onAssistance} />}
  </section>;
}
