import {useEffect,useRef,useState} from "react";
import {api,type JourneyConsent,type JourneyPreview,type JourneyStatus} from "../api";
import {ConversationBudgetNotice} from "./ConversationBudgetNotice";
import {projectBudgetAvailability} from "../projectBudget";
import {conversationSettingsPath,projectBuildPath} from "../navigation";
import type {OpenConversationSample} from "./ConversationSampleCard";
import { consentMatchesSchema, journeyMatchesSchema, consentMatchesRepair } from "../conversation-schema-history";
import { journeyModelSelection, MAX_JOURNEY_MODELS } from "../journey-model-selection";

type Choice={id:string;name:string};
const active=(value?:JourneyStatus)=>value?.dispatch?.status==="running" || ["queued","running","cancelling"].includes(value?.sample?.status ?? "");
const needsUpdate=(value?:JourneyStatus)=>active(value)||value?.sample?.assistance?.status==="waiting";

/** One bounded consent, existing Builder/Sample services. Mount only reads saved work. */
type JourneyCardProps = {
  pendingAnswer?:boolean; onPendingConsent?:(id?:string)=>void;
  repairRequest?:{id:string;draft:string};
  project:string;conversation:string;task:string;schema?:{id:string;revision:number};disabled:boolean;
  onSample:OpenConversationSample;onAssistance?:()=>void;onActiveChange?:(active:boolean)=>void;
  onSchemaOutcome?:(receipt:import("../types").ConversationCallReceipt)=>void;prepareRequested?:boolean;
};
export function ConversationJourneyCard(props: JourneyCardProps) {
  return <JourneyCard key={`${props.project}:${props.conversation}:${props.task}:${props.schema?.id ?? "goal"}:${props.schema?.revision ?? ""}:${props.repairRequest?.id ?? ""}:${props.repairRequest?.draft ?? ""}`} {...props} />;
}
function JourneyCard({project,conversation,task,schema,repairRequest,pendingAnswer,onPendingConsent,disabled,onSample,onAssistance,onActiveChange,onSchemaOutcome,prepareRequested}: JourneyCardProps) {
  const [saved,setSaved]=useState<JourneyStatus>();
  const [preview,setPreview]=useState<JourneyPreview>();
  const [choices,setChoices]=useState<Choice[]>([]);
  const [selected,setSelected]=useState<string[]>();
  const [ready,setReady]=useState(false),[busy,setBusy]=useState(false),[confirmed,setConfirmed]=useState(false),[error,setError]=useState("");
  const alive=useRef(true),pending=useRef(false),frozen=useRef<JourneyConsent|undefined>(undefined);
  const prepared=useRef(false);
  const legacyStorageKey=`annotagent.journey:${project}:${conversation}:${task}`;
  const schemaStorageKey=schema ? `${legacyStorageKey}:${schema.id}:${schema.revision}` : legacyStorageKey;
  const storageKey=repairRequest ? `${schemaStorageKey}:repair:${repairRequest.id}:${repairRequest.draft}` : schemaStorageKey;
  const usableHistory=(items:JourneyStatus[])=>items.filter(item=>journeyMatchesSchema(item,schema)&&consentMatchesRepair(item.record.consent,repairRequest));
  const clearFrozen=()=>{frozen.current=undefined;try{sessionStorage.removeItem(storageKey);}catch{/* Server history owns saved consent. */}};
  const apply=(value:JourneyStatus)=>{setSaved(value);if(frozen.current?.id===value.record.consent.id)clearFrozen();};
  useEffect(()=>{
    alive.current=true;const controller=new AbortController();
    try{const raw=sessionStorage.getItem(storageKey) ?? (!repairRequest ? sessionStorage.getItem(legacyStorageKey) : null);if(raw){const value=JSON.parse(raw) as JourneyConsent;if(value.task_id===task&&consentMatchesRepair(value,repairRequest)&&(!schema||consentMatchesSchema(value,schema)))frozen.current=value;}}catch{/* Invalid local data grants no permission. */}
    void api.journeyHistory(project,conversation,task,controller.signal).then(({items})=>{
      if(controller.signal.aborted)return;
      const candidates=usableHistory(items);
      const value=candidates.find(item=>item.record.consent.id===frozen.current?.id) ?? candidates[0];
      if(value)apply(value);setReady(true);
    }).catch((reason:Error)=>{if(!controller.signal.aborted)setError(reason.message);});
    return()=>{alive.current=false;controller.abort();};
  },[project,conversation,task]);
  useEffect(()=>{setPreview(undefined);setConfirmed(false);},[schema?.id,schema?.revision]);
  useEffect(()=>{if(prepareRequested&&ready&&!saved&&!prepared.current){prepared.current=true;void prepare();}},[prepareRequested,ready,saved]);
  useEffect(()=>{if(saved?.schema?.status==="completed")onSchemaOutcome?.(saved.schema);},[saved?.schema?.id,saved?.schema?.status]);
  useEffect(()=>{onActiveChange?.(active(saved)||busy||Boolean(frozen.current));},[active(saved),busy,saved,onActiveChange]);
  useEffect(()=>{if(pendingAnswer)onPendingConsent?.(saved && !saved.record.revoked && saved.record.consent.repair_after_answer ? saved.record.consent.id : undefined);},[pendingAnswer,saved,onPendingConsent]);
  useEffect(()=>{
    if(!saved || !needsUpdate(saved))return;
    const controller=new AbortController();let timer:ReturnType<typeof setTimeout>;
    const poll=async()=>{
      try{const value=await api.journeyStatus(project,conversation,task,saved.record.consent.id,controller.signal);
        if(controller.signal.aborted)return;apply(value);if(needsUpdate(value))timer=setTimeout(()=>void poll(),1000);else onAssistance?.();
      }catch(reason){if(!controller.signal.aborted){setError((reason as Error).message);timer=setTimeout(()=>void poll(),2000);}}
    };
    void poll();return()=>{controller.abort();clearTimeout(timer);};
  },[project,conversation,task,saved?.record.consent.id,needsUpdate(saved)]);
  async function reload(){
    if(pending.current)return;pending.current=true;setBusy(true);
    try{const history=await api.journeyHistory(project,conversation,task);if(alive.current){const items=usableHistory(history.items);const item=items.find(item=>item.record.consent.id===(frozen.current?.id ?? saved?.record.consent.id)) ?? items[0];if(item)apply(item);setReady(true);setError("");}}
    catch(reason){if(alive.current)setError((reason as Error).message);}
    finally{pending.current=false;if(alive.current)setBusy(false);}
  }
  async function prepare(){
    if(pending.current||disabled||active(saved)||frozen.current)return;pending.current=true;setBusy(true);setError("");setConfirmed(false);setPreview(undefined);
    try{
      const [profiles,native,providers]=await Promise.all([api.modelProfiles(),api.modelInstances(),api.providers()]);
      const options:Choice[]=[...profiles.models.filter(model=>model.enabled&&model.status==="available"&&model.input_modalities.includes("image")&&providers.providers.some(provider=>provider.id===model.provider_id&&provider.adapter!=="mock")).map(model=>({id:`model-profile:${model.id}`,name:model.display_name})),...native.model_profiles.filter(model=>model.selectable).map(model=>({id:model.selection_id,name:model.display_name}))];
      const ids=journeyModelSelection(options.map(option=>option.id),selected);
      if(alive.current){setChoices(options);setSelected(ids);}
      if(!ids.length)throw new Error(options.length ? `Choose 1–${MAX_JOURNEY_MODELS} image models below, then review the authorization. No inference has started.` : "Select an available image model, or connect one in model settings. No inference has started.");
      if(ids.length>MAX_JOURNEY_MODELS)throw new Error(`Choose at most ${MAX_JOURNEY_MODELS} image models. No inference has started.`);
      const source:Record<string,string>=schema?{schema_id:schema.id,schema_revision:String(schema.revision)}:{schema_call_id:crypto.randomUUID()};
      if(repairRequest)source[pendingAnswer?"pending_request_id":"repair_request_id"]=repairRequest.id;
      const value=await api.journeyPreview(project,conversation,task,{consent_id:crypto.randomUUID(),builder_operation_id:crypto.randomUUID(),sample_operation_id:crypto.randomUUID(),...source,allowed_models:JSON.stringify(ids)});
      if(!consentMatchesRepair(value.consent,repairRequest))throw new Error("The repair authorization does not match this saved correction. No execution was started.");
      if(alive.current)setPreview(value);
    }catch(reason){if(alive.current)setError((reason as Error).message);}
    finally{pending.current=false;if(alive.current)setBusy(false);}
  }
  async function start(){
    if(pending.current||!ready||disabled||active(saved))return;
    const retry=saved && !saved.record.revoked && !saved.sample && !preview && !frozen.current;
    if(!retry&&!frozen.current&&(!preview||!confirmed||projectBudgetAvailability(preview.project_call_limit).blocked))return;
    if(preview&&schema&&(preview.consent.schema_id!==schema.id||preview.consent.schema_revision!==schema.revision)){setError("Labels changed. Review the updated authorization.");return;}
    pending.current=true;setBusy(true);setError("");
    const consent=retry?saved.record.consent:frozen.current ?? {...preview!.consent,allow_unknown_cost:true,...(preview!.consent.schema_proposal?{schema_proposal:{...preview!.consent.schema_proposal,allow_unknown_cost:true}}:{})};
    frozen.current=consent;try{sessionStorage.setItem(storageKey,JSON.stringify(consent));}catch{/* In-view retry retains the exact envelope. */}
    try{
      await api.saveJourney(project,conversation,task,consent);
      const value=await api.executeJourney(project,conversation,task,consent.id);
      if(alive.current){apply(value);setPreview(undefined);setConfirmed(false);}
    }catch(reason){if(alive.current){setError((reason as Error).message);try{const value=await api.journeyStatus(project,conversation,task,consent.id);if(alive.current){apply(value);setPreview(undefined);}}catch{/* Unknown receipt: retain exact consent, never create another request. */}}}
    finally{pending.current=false;if(alive.current)setBusy(false);}
  }
  async function stop(){
    if(!saved||pending.current)return;pending.current=true;setBusy(true);
    try{await api.revokeJourney(project,conversation,task,saved.record.consent.id);const value=await api.journeyStatus(project,conversation,task,saved.record.consent.id);if(alive.current)apply(value);}
    catch(reason){if(alive.current)setError((reason as Error).message);}
    finally{pending.current=false;if(alive.current)setBusy(false);}
  }
  const running=active(saved), draft=saved?.sample?.draft_id ?? saved?.builder?.evidence?.draft_id;
  const effective=saved?.record.resolved_consent ?? saved?.record.consent;
  const stale=Boolean(effective&&schema&&(!saved?.record.consent.schema_proposal||saved.record.resolved_consent)&&(effective.schema_id!==schema.id||effective.schema_revision!==schema.revision));
  return <section className="conversation-builder-card" aria-label="Build and test annotation plan">
    <h3>{pendingAnswer?"Continue after your correction":repairRequest?"Improve from your correction":schema?"Try an annotation plan":"Turn your goal into sample results"}</h3><p>{pendingAnswer?"Optionally authorize one repair and sample test after you submit this requested correction. Nothing runs while you are editing. You can also submit without authorizing inference.":repairRequest?"Repair the saved plan using this correction, then test up to three images within one explicit authorization.":schema?"Build a plan from your saved labels, then test up to three images.":"Propose labels from your goal, build a plan and test up to three images. If the goal needs clarification, ask before image processing."} Results stay in the sample sandbox.</p>
    {!ready&&<p role="status">Restoring saved work…</p>}
    {!running&&!preview&&!frozen.current&&(!saved||stale)&&<button className="primary" disabled={!ready||busy||disabled} onClick={()=>void prepare()}>Review build and sample authorization</button>}
    {choices.length>0&&!running&&<details open={choices.length>MAX_JOURNEY_MODELS ? true : undefined}><summary>Allowed image models · {selected?.length ?? 0} selected</summary><p>Choose 1–{MAX_JOURNEY_MODELS} models. Only these exact installed bindings may receive the sample images. Changing this list requires a new preview.</p>{choices.length>MAX_JOURNEY_MODELS&&<p>Your registry has {choices.length} available models. No arbitrary subset is selected automatically.</p>}<div className="journey-model-choices">{choices.map(choice=><label key={choice.id}><input type="checkbox" checked={selected?.includes(choice.id) ?? false} disabled={busy||Boolean(frozen.current)||(!selected?.includes(choice.id)&&(selected?.length ?? 0)>=MAX_JOURNEY_MODELS)} onChange={event=>{setSelected(current=>event.target.checked?[...(current??[]),choice.id]:(current??[]).filter(id=>id!==choice.id));setPreview(undefined);setConfirmed(false);}}/>{choice.name}</label>)}</div></details>}
    {preview&&<div className="conversation-consent" aria-label="Build and sample authorization">
      {preview.consent.repair&&<p>Repair only your saved correction's Draft · revision {preview.consent.repair.revision}. This authorization covers one bounded repair and sample test, not future corrections.</p>}
      {preview.consent.repair_after_answer&&<p>Only your answer to this request, image and feedback revision {preview.consent.repair_after_answer.expected_feedback_sequence} may resume this task. Submitting it will start the permitted repair and sample test; this does not authorize future corrections.</p>}
      <p><strong>Planner: {preview.builder.model_name}</strong> · {preview.builder.destination}</p>
      <p>Saved goal and labels go to the planner. {preview.consent.images.length} sample images may go to:</p>
      <ul>{preview.data.models.map(model=><li key={model.scope.model_id}>{model.display_name} · {model.destination}</li>)}</ul>
      <details><summary>Model permissions and frozen bindings</summary>{preview.data.models.map(model=><div key={model.scope.model_id}><strong>{model.display_name}</strong><pre>{JSON.stringify(model.permissions,null,2)}</pre></div>)}</details>
      <p>Up to {preview.consent.maximum_builder_calls+(preview.consent.schema_proposal?1:0)} planning calls{preview.consent.schema_proposal?" (including one label proposal)":""} + {preview.consent.maximum_sample_calls} image-model calls. Cost unknown. Permission expires at {new Date(preview.consent.expires_at).toLocaleTimeString()}.</p>
      <p>No publish, dataset run or annotation acceptance. If the generated plan needs another model or a different scope, testing stops for your decision.</p>
      {preview.consent.continue_after_clarification&&<p>If clarification is needed, saving your answer continues this same request only within the listed models, images, call limits and expiry. Changing that scope requires new authorization.</p>}
      <ConversationBudgetNotice value={preview.project_call_limit} maximumCalls={preview.consent.maximum_builder_calls+preview.consent.maximum_sample_calls+(preview.consent.schema_proposal?1:0)} busy={busy} onRefresh={()=>void prepare()}/>
      <label><input type="checkbox" checked={confirmed} disabled={busy} onChange={event=>setConfirmed(event.target.checked)}/>Allow this plan and sample test within the listed scope; actual cost is unknown</label>
      <div className="button-row"><button disabled={busy} onClick={()=>{setPreview(undefined);setConfirmed(false);}}>Back</button><button className="primary" disabled={!confirmed||busy||disabled||projectBudgetAvailability(preview.project_call_limit).blocked} onClick={()=>void start()}>{pendingAnswer?"Authorize continuation after this answer":"Build plan and test samples"}</button></div>
    </div>}
    {saved&&<div className="conversation-builder-result" aria-live="polite">
      {pendingAnswer&&!saved.record.revoked&&!running&&<><p>Authorization saved · waiting for your correction. No inference is running. Submit the correction in the canvas to continue within this scope.</p><button disabled={busy} onClick={()=>void stop()}>Revoke continuation permission</button></>}
      {(!pendingAnswer||running||saved.record.revoked)&&<strong>{running?saved.sample?"Testing saved sample images":saved.record.consent.schema_proposal&&!saved.record.resolved_consent?"Proposing labels from your goal":"Building the annotation plan":saved.sample?.status==="succeeded"?"Sample results saved":saved.record.revoked?"Authorization revoked":saved.dispatch?.error?"Execution needs attention":saved.dispatch?.status==="interrupted"?"Execution interrupted":saved.builder?.evidence?.outcome?.replaceAll("_"," ")??"Authorization saved; execution not started"}</strong>}
      {stale&&<p>These records use labels revision {effective?.schema_revision}, not your current labels. Nothing has been rebuilt automatically.</p>}
      <details><summary>Saved authorization scope</summary><p>{saved.record.consent.images.length} images · Up to {saved.record.consent.maximum_builder_calls+(saved.record.consent.schema_proposal?1:0)} planning calls + {saved.record.consent.maximum_sample_calls} image-model calls · Cost unknown. Expires {new Date(saved.record.consent.expires_at).toLocaleString()}.</p><p>Exact permitted image-model selections (not today's defaults):</p><ul>{saved.record.consent.allowed_models.map(model=><li key={model.model_id}>{model.model_id}</li>)}</ul><p>This authorization does not publish a plan or accept annotations.</p></details>
      {running&&<><button disabled={busy||saved.record.revoked} onClick={()=>void stop()}>Stop build and sample task</button><p>Leaving this page does not stop the task. In-flight calls may still be billed.</p></>}
      {saved.dispatch?.error&&<p role="alert">{saved.dispatch.error}</p>}{saved.sample?.error&&<p role="alert">{saved.sample.error}</p>}
      {saved.schema?.evidence?.error&&<p role="alert">{saved.schema.evidence.error}</p>}
      {saved.sample?.status==="succeeded"&&saved.sample.assistance?.status==="waiting"&&<p role="status">Preparing saved requests for human judgment. No additional inference is running.</p>}
      {saved.sample?.assistance?.status==="failed"&&<p role="alert">Review-request preparation failed: {saved.sample.assistance.error}. Saved sample results can still be opened.</p>}
      {saved.sample?.status==="succeeded"&&<button className="primary" onClick={()=>onSample(saved.sample!.draft_id,saved.sample!.id)}>View sample results in canvas</button>}
      {!pendingAnswer&&!running&&!saved.sample&&!saved.record.revoked&&<button className={preview?undefined:"primary"} disabled={busy||disabled||Boolean(stale)} onClick={()=>void start()}>Continue the same saved request</button>}
      {draft&&<a href={projectBuildPath(project,"pipeline",{draftId:draft})}>View actual plan and execution details</a>}
    </div>}
    {saved&&(!pendingAnswer||saved.record.revoked)&&!running&&!preview&&!frozen.current&&!stale&&<details><summary>Build a different plan</summary><p>This requires a new authorization and may incur additional cost. Saved results remain unchanged.</p><button disabled={!ready||busy||disabled} onClick={()=>void prepare()}>Review build and sample authorization</button></details>}
    {frozen.current&&!running&&<p role="status">Request outcome unknown. Reload saved state or explicitly retry the same request; no automatic retry is running.<button disabled={busy||!ready||disabled} onClick={()=>void start()}>Retry same build and sample request</button></p>}
    {error&&<p role="alert">{error} Saved work remains on the server.</p>}
    {(error||!ready)&&<div className="button-row"><button disabled={busy} onClick={()=>void reload()}>Reload saved journey state</button><a href={conversationSettingsPath(project,"models",window.location.pathname+window.location.search)}>Review model setup</a></div>}
  </section>;
}
