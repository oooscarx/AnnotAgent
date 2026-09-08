import { useEffect, useRef, useState } from "react";
import { api, ApiRequestError } from "../api";
import { ConversationBudgetNotice } from "./ConversationBudgetNotice";
import {projectBudgetAvailability} from "../projectBudget";
import { projectBuildPath } from "../navigation";
import { ConversationSampleCard, type OpenConversationSample } from "./ConversationSampleCard";
import {ConversationJourneyCard} from "./ConversationJourneyCard";
import type { ConversationBuilderConsent, ConversationBuilderItem, ConversationBuilderPreview, ConversationSchemaDraft } from "../types";
import { builderMatchesSchema, builderMatchesClassRepair, builderMatchesHumanRepair, consentMatchesSchema } from "../conversation-schema-history";
import { builderConsent, restoreBuilderPending } from "../conversation-builder-pending";

/** Restoring history only reads. Model work requires a new, explicit consent. */
type BuilderCardProps = {
  project: string; conversation: string; task: string; schema: Pick<ConversationSchemaDraft,"id"|"revision">; editing: boolean; onAssistance?:()=>void; onSample: OpenConversationSample;
  repairRequest?: {id:string;draft:string};
  imageClassRepair?: {id:string;draft:string};
};
export function ConversationBuilderCard(props: BuilderCardProps) {
  return <BuilderCard key={`${props.project}:${props.conversation}:${props.task}:${props.schema.id}:${props.schema.revision}:${props.repairRequest?.id ?? ""}:${props.imageClassRepair?.id ?? ""}`} {...props} />;
}
function BuilderCard({ project, conversation, task, schema, editing, onSample, repairRequest, imageClassRepair, onAssistance }: BuilderCardProps) {
  const repair=Boolean(repairRequest || imageClassRepair);
  const historyScope=imageClassRepair ? {image_class_review_id:imageClassRepair.id} : repairRequest ? {repair_request_id:repairRequest.id} : undefined;
  const matches=(entry:ConversationBuilderItem)=>entry.operation.task_id===task && (imageClassRepair ? builderMatchesClassRepair(entry,schema,imageClassRepair) : repairRequest ? builderMatchesHumanRepair(entry,schema,repairRequest) : builderMatchesSchema(entry,schema) && !entry.operation.evidence?.repair_source && entry.session?.working_draft?.build_mode.kind!=="repair_draft");
  const [advanced,setAdvanced]=useState(false);
  const [journeyActive,setJourneyActive]=useState(false);
  const [item,setItem]=useState<ConversationBuilderItem>();
  const [preview,setPreview]=useState<ConversationBuilderPreview>();
  const [confirmed,setConfirmed]=useState(false);
  const [ready,setReady]=useState(false);
  const [busy,setBusy]=useState(false);
  const [uncertain,setUncertain]=useState(false);
  const [cancelled,setCancelled]=useState(false);
  const [error,setError]=useState("");
  const [invalidRetry,setInvalidRetry]=useState<string>();
  const pending=useRef(false);
  const alive=useRef(true);
  const frozen=useRef<ConversationBuilderConsent | undefined>(undefined);
  const repairContext=imageClassRepair ? {...imageClassRepair,kind:"image_class_review" as const} : repairRequest ? {...repairRequest,kind:"human_request" as const} : undefined;
  const pendingKey=`annotagent.builder-pending:${project}:${conversation}:${task}:${schema.id}:${schema.revision}:${repairContext?.kind ?? "new"}:${repairContext?.id ?? ""}`;
  const clearPending=()=>{const hadPending=Boolean(frozen.current);frozen.current=undefined;if(hadPending)try{sessionStorage.removeItem(pendingKey);}catch{/* Exact server history remains authoritative. */}};
  const running=item?.operation.status==="reserved" || Boolean(frozen.current && (busy || uncertain));
  useEffect(()=>{
    alive.current=true; const controller=new AbortController();
    try{const raw=sessionStorage.getItem(pendingKey);if(raw){const saved=restoreBuilderPending(raw,schema,repairContext);if(saved){frozen.current=saved.consent;setPreview(saved.preview);setConfirmed(true);setUncertain(true);}else {setInvalidRetry(raw);setError("The saved Builder retry envelope is invalid. No model request was restored.");}}}catch{/* No model request is reconstructed from unreadable browser storage. */}
    const scope=frozen.current ? {operation_id:frozen.current.selection.operation_id} : historyScope;
    void api.conversationBuilderHistory(project,conversation,task,controller.signal,scope).then(async(history)=>{
      const entry=history.items.find(entry=>matches(entry)&&(!frozen.current||entry.operation.id===frozen.current.selection.operation_id));
      const journeys=entry&&!repair ? await api.journeyHistory(project,conversation,task,controller.signal) : undefined;
      if(!controller.signal.aborted) { setItem(entry);if(entry?.operation.status!=="reserved"&&entry){clearPending();setPreview(undefined);setUncertain(false);}if(entry&&journeys&&!journeys.items.some(journey=>journey.record.consent.builder_operation_id===entry.operation.id))setAdvanced(true);if(frozen.current)setAdvanced(true);setReady(true); }
    }).catch((error:Error)=>{if(!controller.signal.aborted)setError(error.message);});
    return ()=>{alive.current=false;controller.abort();};
  },[project,conversation,task,repairRequest?.draft,imageClassRepair?.id]);
  useEffect(()=>{
    if(!running)return;
    const controller=new AbortController(); let fetching=false;
    const poll=async()=>{
      if(fetching)return; fetching=true;
      try {
        const id=frozen.current?.selection.operation_id ?? item?.operation.id;
        if(!id)return;
        const history=await api.conversationBuilderHistory(project,conversation,task,controller.signal,{operation_id:id});
        const current=history.items.find((entry)=>entry.operation.id===id && matches(entry));
        if(!controller.signal.aborted && current) { setItem(current);setError(""); if(current.operation.status!=="reserved") {setUncertain(false);clearPending();setPreview(undefined);} }
      } catch(error) {if(!controller.signal.aborted)setError((error as Error).message);}
      finally {fetching=false;}
    };
    void poll(); const timer=window.setInterval(()=>void poll(),1200);
    return ()=>{controller.abort();window.clearInterval(timer);};
  },[running,project,conversation,task,item?.operation.id]);
  async function prepare() {
    if(pending.current || running || editing || invalidRetry!==undefined)return; pending.current=true;setBusy(true);setConfirmed(false);setError("");
    try {
      const result=await api.conversationBuilderPreview(project,conversation,task,{operation_id:crypto.randomUUID(),schema_id:schema.id,schema_revision:schema.revision,repair_request_id:repairRequest?.id,image_class_review_id:imageClassRepair?.id});
      if(alive.current){setPreview(result);setConfirmed(false);setCancelled(false);}
    } catch(error) {if(alive.current)setError((error as Error).message);}
    finally {pending.current=false;if(alive.current)setBusy(false);}
  }
  async function launch() {
    if(pending.current || !preview || !confirmed || editing || invalidRetry!==undefined || (!frozen.current && projectBudgetAvailability(preview.project_call_limit).blocked))return;
    if(!consentMatchesSchema(preview.selection,schema)){setError("Labels changed. Review a fresh Builder authorization before continuing.");setPreview(undefined);return;}
    pending.current=true;setBusy(true);setError("");setItem(undefined);
    const consent=frozen.current ?? builderConsent(preview);
    try{sessionStorage.setItem(pendingKey,JSON.stringify({preview,consent}));}
    catch{pending.current=false;setBusy(false);setError("The exact Builder request could not be saved in this browser. No model call was started. Enable local storage before retrying.");return;}
    frozen.current=consent;
    try {
      const operation=await api.launchConversationBuilder(project,conversation,task,frozen.current);
      const history=await api.conversationBuilderHistory(project,conversation,task,undefined,{operation_id:operation.id});
      const result=history.items.find((entry)=>entry.operation.id===operation.id && matches(entry)) ?? {operation};
      if(!matches(result)||operation.id!==consent.selection.operation_id)throw new Error("The Builder response belongs to another operation. Restoring the exact saved request.");
      if(alive.current){setItem(result);if(operation.status!=="reserved"){setPreview(undefined);setUncertain(false);clearPending();}}
    } catch(error) {if(alive.current){
      setError((error as Error).message);
      const id=frozen.current?.selection.operation_id;
      try {
        if(!id)throw error;
        const history=await api.conversationBuilderHistory(project,conversation,task,undefined,{operation_id:id});
        if(!alive.current)return;
        const saved=history.items.find((entry)=>entry.operation.id===id && matches(entry));
        if(saved){setItem(saved);setError("");setUncertain(saved.operation.status==="reserved");if(saved.operation.status!=="reserved"){clearPending();setPreview(undefined);}}
        else if(error instanceof ApiRequestError && [400,401,403,404,409,422,429].includes(error.status)){setUncertain(false);setPreview(undefined);clearPending();}
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
  const completed=Boolean(draftId && item && builderMatchesSchema(item,schema) && invalidRetry===undefined && !running && !preview && !error && !cancelled && !session?.unresolved_bindings?.length && !item?.operation.evidence?.error && item?.operation.status==="completed" && session?.outcome==="draft_ready_for_human_review");
  const buildDetails=<>
    <h3>{repair ? "Revise the plan from your correction" : "Build the annotation plan"}</h3>
    {repair && <p>Your saved correction is evidence for revising this plan, not proof of improved accuracy. The original plan remains unchanged.</p>}
    <p>{builtRevision ? `This operation uses Schema revision ${builtRevision}.` : `A new build will use saved labels at revision ${schema.revision}.`} This step builds a Draft; it does not test images or publish.</p>
    {builtRevision && builtRevision!==schema.revision && <p role="status">Labels are now revision {schema.revision}; this saved operation has not been rebuilt for those changes.</p>}
    {invalidRetry!==undefined && <aside role="alert"><p>The browser retry record cannot be safely reused. Server operations and costs may still exist; clearing this local record does not cancel them or authorize new calls.</p><button disabled={busy||running} onClick={()=>{try{if(sessionStorage.getItem(pendingKey)!==invalidRetry)throw new Error("The local retry record changed. Reload before clearing it.");sessionStorage.removeItem(pendingKey);setInvalidRetry(undefined);setError("");}catch(reason){setError((reason as Error).message);}}}>Discard unreadable local Builder retry record</button></aside>}
    {!running && !preview && <button disabled={!ready || busy || editing || invalidRetry!==undefined} onClick={()=>void prepare()}>{item ? "Review another build request" : "Review Builder authorization"}</button>}
    {preview && !running && <div className="conversation-consent" aria-label="Builder model authorization"><ConversationBudgetNotice value={preview.project_call_limit} maximumCalls={preview.maximum_builder_calls} busy={busy} onRefresh={()=>void prepare()}/><strong>{preview.model_name}</strong><span>{preview.remote_model} · {preview.destination}</span><p>{preview.data_scope}</p><p>Up to {preview.maximum_builder_calls} text calls · {preview.used_calls} calls already used · Cumulative limit {preview.maximum_calls} · Cost unknown</p><p>{preview.operation}</p><label><input type="checkbox" checked={confirmed} disabled={busy} onChange={(event)=>setConfirmed(event.target.checked)} />Allow this bounded Builder request; actual cost is unknown</label><div className="button-row"><button disabled={busy} onClick={()=>setPreview(undefined)}>Back</button><button className="primary" disabled={!confirmed || busy || editing || projectBudgetAvailability(preview.project_call_limit).blocked} onClick={()=>void launch()}>Build Pipeline Draft</button></div></div>}
    {running && <><p role="status">{session?.phase ? `Builder stage: ${session.phase.replaceAll("_"," ")}` : "Submitting or restoring the saved Builder operation…"}</p>{session?.next_action && <p>{session.next_action}</p>}<button onClick={()=>void stop()}>Stop Builder</button><small>Leaving this page does not stop the server task. No images are being tested.</small></>}
    {cancelled && <p role="status">{running ? "Cancellation saved. Waiting for the server to settle any in-flight call; its cost may be unknown." : "Cancellation saved. The operation has stopped; any prior call cost remains recorded separately."}</p>}
    {uncertain && !busy && <button onClick={()=>void launch()} disabled={cancelled}>Retry the same Builder request</button>}
    {item && item.operation.status!=="reserved" && <div className="conversation-builder-result"><strong>{item.operation.status==="interrupted" ? "Build interrupted" : completed ? "Saved execution record" : "Builder outcome saved"}</strong><p>{session?.outcome?.replaceAll("_"," ") ?? item.operation.evidence?.outcome?.replaceAll("_"," ") ?? item.operation.status}</p>{item.operation.evidence?.error && <p>{item.operation.evidence.error}</p>}{session?.next_action && <p>{session.next_action}</p>}{session?.unresolved_bindings?.length ? <ul>{session.unresolved_bindings.map((binding,index)=><li key={index}>{binding}</li>)}</ul> : null}<small>No sample result or formal annotation was accepted.</small></div>}
    {error && <p role="alert">{error} Saved operations remain on the server; refreshing will not start another build.</p>}
  </>;
  if(!repair&&!advanced&&invalidRetry===undefined)return <>
    <ConversationJourneyCard key={`${project}:${conversation}:${task}`} project={project} conversation={conversation} task={task} schema={schema} disabled={editing||running} onSample={onSample} onAssistance={onAssistance} onActiveChange={setJourneyActive}/>
    <button disabled={!ready||busy||editing||running||journeyActive} onClick={()=>{setAdvanced(true);void prepare();}}>Review Builder authorization</button>
    <small>Advanced: build only, then authorize samples separately.</small>
  </>;
  return <section className="conversation-builder-card" aria-label={repair ? "Repair annotation pipeline" : "Build annotation pipeline"}>
    {!repair&&!running&&!busy&&<button onClick={()=>setAdvanced(false)}>Back to build and sample task</button>}
    {completed ? <details className="conversation-completed-stage"><summary><strong>Builder outcome saved</strong><span>View build details</span></summary><div>{buildDetails}</div></details> : buildDetails}
    {draftId && !running && <a href={projectBuildPath(project,"pipeline",{draftId,agentSessionId:session?.id,workspaceReturn:window.location.pathname+window.location.search})}>Open saved Pipeline details</a>}
    {draftId && !running && <ConversationSampleCard key={`${task}:${draftId}`} project={project} conversation={conversation} task={task} draft={draftId} disabled={editing || busy} onOpen={onSample} onAssistance={onAssistance} />}
  </section>;
}
