import { useEffect, useId, useRef, useState } from "react";
import { api, ApiRequestError } from "../api";
import type {JourneyConsent} from "../api";
import { ConversationSchemaEditor } from "./ConversationSchemaEditor";
import type { OpenConversationSample } from "./ConversationSampleCard";

/** Human semantics use the same versioned Schema and Builder, without a model receipt. */
export function ConversationHumanSchema({project,conversation,task,schemaId,onSaved,onDirtyChange,onSample,onAssistance,clarification,onCancelClarification,cancelling}: {
  onCancelClarification?:()=>void; cancelling?:boolean;
  clarification?:{call_id:string;expected_schema_revision:string;question:string};
  project:string; conversation:string; task:string; schemaId?:string; onSaved:(id:string)=>void;
  onDirtyChange:(dirty:boolean)=>void; onSample:OpenConversationSample; onAssistance?:()=>void;
}) {
  const id=useId();
  const [kind,setKind]=useState("bounding_box");
  const [labels,setLabels]=useState("");
  const [rules,setRules]=useState("");
  const [busy,setBusy]=useState(false);
  const [uncertain,setUncertain]=useState(false);
  const [error,setError]=useState("");
  const [answered,setAnswered]=useState<{schema:string;call:string;question:string}>();
  const [journey,setJourney]=useState<JourneyConsent>();
  const [journeyReady,setJourneyReady]=useState(!clarification);
  const pending=useRef(false), alive=useRef(true);
  const request=useRef<Parameters<typeof api.saveHumanConversationSchema>[3] | undefined>(undefined);
  useEffect(()=>{alive.current=true; return()=>{alive.current=false;};},[]);
  useEffect(()=>{
    if(!clarification)return;
    const controller=new AbortController();
    void api.journeyHistory(project,conversation,task,controller.signal).then(({items})=>{
      if(controller.signal.aborted)return;
      setJourney(items.find(item=>item.record.consent.continue_after_clarification&&item.record.consent.schema_proposal?.call_id===clarification.call_id&&!item.record.revoked&&new Date(item.record.consent.expires_at).getTime()>Date.now())?.record.consent);
      setJourneyReady(true);
    }).catch((reason:Error)=>{if(!controller.signal.aborted){setError(`Could not restore continuation authorization: ${reason.message}. Saving labels will not start inference.`);setJourneyReady(true);}});
    return()=>controller.abort();
  },[project,conversation,task,clarification?.call_id]);
  useEffect(()=>{
    if(!schemaId||!clarification)return;
    const controller=new AbortController();
    void api.conversationSchemaClarification(project,conversation,task,clarification.call_id,controller.signal).then(saved=>{
      if(!controller.signal.aborted&&saved.status==="applied"&&saved.schema_draft_id===schemaId)
        setAnswered({schema:schemaId,call:saved.id,question:saved.question});
    }).catch(()=>{/* Do not claim an answer link when the owned server evidence is unavailable. */});
    return()=>controller.abort();
  },[project,conversation,task,schemaId,clarification?.call_id]);
  useEffect(()=>{if(schemaId)return; onDirtyChange(busy||uncertain||kind!=="bounding_box"||Boolean(labels||rules));return()=>onDirtyChange(false);},[schemaId,busy,uncertain,kind,labels,rules,onDirtyChange]);
  async function save() {
    if(pending.current||cancelling)return;
    pending.current=true; setBusy(true);setError("");
    request.current ??= {request_id:crypto.randomUUID(),journey_consent_id:journey?.id,clarification:clarification?{call_id:clarification.call_id,expected_schema_revision:clarification.expected_schema_revision}:undefined,decision:{decision:"draft",kind,
      labels:labels.split("\n").map(value=>value.trim()).filter(Boolean),multi_label:false,attributes:{},
      boundary_rules:rules.split("\n").map(value=>value.trim()).filter(Boolean),rationale:"Human-authored annotation semantics"}};
    try {const saved=await api.saveHumanConversationSchema(project,conversation,task,request.current);if(alive.current){setUncertain(false);if(saved.journey_resume?.error)setError(`Labels saved; continuation did not start: ${saved.journey_resume.error}`);onSaved(saved.id);}}
    catch(error){if(alive.current){setError((error as Error).message);const rejected=error instanceof ApiRequestError&&[400,401,403,404,409,422].includes(error.status);setUncertain(!rejected);if(rejected)request.current=undefined;}}
    finally{pending.current=false;if(alive.current)setBusy(false);}
  }
  if(schemaId)return <>
    {error&&<p role="alert">{error}</p>}
    {answered?.schema===schemaId&&answered?.call===clarification?.call_id ? <aside className="conversation-consent" aria-label="Saved clarification answer">
      <strong>Clarification answered · Labels saved</strong>
      <p>Your output type, labels and boundary rules are saved in this task's editable label draft. Writing labels itself uses no model. Any continued processing is recorded below under its own saved authorization; no additional permission or call limit was granted.</p>
      <details><summary>Why AnnotAgent asked</summary><p>{answered.question}</p><small>The original question remains unchanged. Later label edits create a new Schema revision; they do not rewrite the model's request.</small></details>
    </aside> : <p>Human-defined labels · No model call was used to create this Schema Draft.</p>}
    <ConversationSchemaEditor project={project} conversation={conversation} task={task} schemaId={schemaId} onDirtyChange={onDirtyChange} onSample={onSample} onAssistance={onAssistance}/>
  </>;
  return <section className="conversation-schema-editor" aria-label="Define labels without a model">
    {clarification&&<aside className="conversation-consent" aria-label="Answer annotation clarification"><h4>Clarify this annotation task</h4><p>{clarification.question}</p><small>Your answer defines this task's labels and output. No dataset annotations are accepted.</small></aside>}
    <p>{!journeyReady?"Checking this task's saved authorization…":journey?`Save your answer to continue the same authorized plan and sample task: ${journey.images.length} images, up to ${journey.maximum_builder_calls} Builder and ${journey.maximum_sample_calls} image-model calls; original expiry ${new Date(journey.expires_at).toLocaleTimeString()}. Cost remains unknown. Changed bindings or scope stop execution.`:"Choose the output and labels yourself. Saving only creates a label draft; building or testing a pipeline still requires compatible models and separate authorization."}</p>
    <label htmlFor={`${id}-kind`}>Output type</label><select id={`${id}-kind`} value={kind} disabled={busy||uncertain} onChange={event=>setKind(event.target.value)}><option value="bounding_box">Object boxes</option><option value="classification">Whole-image categories</option></select>
    <small>{kind==="bounding_box"?"Example: a box around each cup, labelled cup.":"Example: classify the entire photo as indoor or outdoor."}</small>
    <label htmlFor={`${id}-labels`}>Labels · one per line</label><textarea id={`${id}-labels`} rows={3} value={labels} disabled={busy||uncertain} onChange={event=>setLabels(event.target.value)}/>
    <label htmlFor={`${id}-rules`}>Boundary rules · optional</label><textarea id={`${id}-rules`} rows={2} value={rules} disabled={busy||uncertain} onChange={event=>setRules(event.target.value)}/>
    <p role="status">{busy?"Saving label draft…":uncertain?"Save outcome unknown. Retry keeps the same request; refresh checks saved drafts.":"These labels have not been saved yet."}</p>
    {error&&<p role="alert">{error} Your input remains here.</p>}
    <button className="primary" disabled={busy||cancelling||!journeyReady||!labels.trim()} onClick={()=>void save()}>{uncertain?"Retry same label save":clarification?"Save answer and continue":"Save label draft without a model"}</button>
    {onCancelClarification&&<button disabled={busy||uncertain||cancelling} onClick={onCancelClarification}>Cancel clarification</button>}
  </section>;
}
