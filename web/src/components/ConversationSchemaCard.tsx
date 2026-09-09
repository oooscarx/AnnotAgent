import { useEffect, useRef, useState } from "react";
import { api, ApiRequestError } from "../api";
import type { ConversationCallReceipt, ConversationSchemaPreview, ConversationTask, ConversationSchemaAuthorization } from "../types";
import { ConversationSchemaEditor } from "./ConversationSchemaEditor";
import { ConversationBudgetNotice } from "./ConversationBudgetNotice";
import {projectBudgetAvailability} from "../projectBudget";
import { ConversationHumanSchema } from "./ConversationHumanSchema";
import {ConversationJourneyCard} from "./ConversationJourneyCard";
import type { OpenConversationSample } from "./ConversationSampleCard";

/** Restores server objects; mounting never creates a task or invokes a model. */
export function ConversationSchemaCard({ project, conversation, message, onDirtyChange, onSample, onAssistance, onSetup,prepareRequested }: { prepareRequested?:boolean; project: string; conversation: string; message: string; onDirtyChange: (dirty: boolean) => void; onAssistance?:()=>void; onSample: OpenConversationSample; onSetup?:(task?:string)=>void }) {
  const [task, setTask] = useState<ConversationTask>();
  // New goals start with text-only planning. A combined image authorization is
  // restored only when that task actually has a saved Journey, never inferred
  // from the absence of previous calls.
  const [initialJourney,setInitialJourney]=useState(false);
  const [initialPrepare,setInitialPrepare]=useState(false);
  const [journeyActive,setJourneyActive]=useState(false);
  const [manual,setManual]=useState(false);
  const [humanSchema,setHumanSchema]=useState<string>();
  const [preview, setPreview] = useState<ConversationSchemaPreview>();
  const [receipt, setReceipt] = useState<ConversationCallReceipt>();
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [error, setError] = useState("");
  const [callId, setCallId] = useState("");
  const [cancelled, setCancelled] = useState(false);
  const [savedConsent,setSavedConsent]=useState<ConversationSchemaAuthorization>();
  const frozen=useRef<ConversationSchemaAuthorization|undefined>(undefined);
  const pending = useRef(false);
  const active = useRef(true);
  const preparationHandled=useRef(false);
  useEffect(()=>{
    // Only the explicit composer action requests this read-only preview. Mount and
    // reload never create tasks or authorize inference. Task identity was saved first.
    if(prepareRequested&&ready&&task&&!preparationHandled.current&&!receipt&&!manual&&!cancelled){
      preparationHandled.current=true;if(initialJourney)void prepareInitial();else void prepare();
    }
  },[prepareRequested,ready,task?.input.id,receipt,manual,cancelled]);
  useEffect(() => {
    active.current = true; const controller = new AbortController();
    void api.conversationTasks(project, conversation, controller.signal).then(async (tasks) => {
      const current = tasks.find((item) => item.input.source_message_id === message);
      const calls = current ? await api.conversationSchemaCalls(project, conversation, current.input.id, controller.signal) : [];
      const cancellations = current ? await api.conversationSchemaCancellations(project, conversation, current.input.id, controller.signal) : [];
      const human = current ? await api.humanConversationSchemas(project,conversation,current.input.id,controller.signal) : [];
      const authorization=current ? await api.pendingSchemaAuthorization(project,conversation,current.input.id,controller.signal) : null;
      const journeys=current ? await api.journeyHistory(project,conversation,current.input.id,controller.signal) : {items:[]};
      if (controller.signal.aborted) return;
      const id = calls[0]?.id ?? authorization?.call_id ?? cancellations.at(-1)?.call_id ?? "";
      setSavedConsent(authorization??undefined);frozen.current=authorization??undefined;
      setTask(current); setReceipt(calls[0]); setCallId(id); setCancelled(cancellations.some((item) => item.call_id === id)); setReady(true);
      setHumanSchema(human[0]?.id); setManual(human.length>0);
      const initial=journeys.items.find(item=>item.record.consent.schema_proposal);
      setInitialJourney(Boolean(initial && (!initial.schema || initial.schema.status!=="completed")));
    }).catch((error: Error) => { if (!controller.signal.aborted) setError(error.message); });
    return () => { active.current = false; controller.abort(); };
  }, [project, conversation, message,prepareRequested]);
  useEffect(() => {
    if (!task || !callId || (savedConsent&&!busy) || (cancelled && !receipt) || (receipt && receipt.status !== "reserved")) return;
    const controller = new AbortController();
    const poll = () => void api.conversationSchemaCalls(project, conversation, task.input.id, controller.signal).then((calls) => {
      if (!controller.signal.aborted) { const current = calls.find((item) => item.id === callId); if (current) setReceipt(current); }
    }).catch((error: Error) => { if (!controller.signal.aborted) setError(error.message); });
    poll(); const interval = window.setInterval(poll, 1500);
    return () => { controller.abort(); window.clearInterval(interval); };
  }, [project, conversation, task?.input.id, callId, receipt?.status, cancelled,savedConsent,busy]);
  async function prepareInitial(){
    if(pending.current)return;pending.current=true;setBusy(true);setError("");
    try{const goal=await api.projectGoal(project);const current=task??await api.beginConversationTask(project,conversation,{id:crypto.randomUUID(),source_message_id:message,schema_revision:goal.revision});
      if(active.current){setTask(current);setInitialJourney(true);setInitialPrepare(true);if(!task)onAssistance?.();}
    }catch(reason){if(active.current)setError((reason as Error).message);}
    finally{pending.current=false;if(active.current)setBusy(false);}
  }
  async function prepare(human=false) {
    if (pending.current) return; pending.current = true; setBusy(true); setConfirmed(false); setError("");
    try {
      const goal = await api.projectGoal(project);
      const current = task ?? await api.beginConversationTask(project,conversation,{id:crypto.randomUUID(),source_message_id:message,schema_revision:goal.revision});
      if (!active.current) return;
      setTask(current);
      // The legacy saved-message entry can create the task here rather than in
      // the composer. Refresh the parent's owned task/request snapshot as well;
      // otherwise its selected goal remains absent until a page reload.
      if (!task) onAssistance?.();
      if(human){setManual(true);setPreview(undefined);setConfirmed(false);return;}
      const next = await api.conversationSchemaPreview(project,conversation,current.input.id,savedConsent?.model_id);
      if(savedConsent&&next.scope_hash!==savedConsent.scope_hash)throw new Error("The saved Schema authorization no longer matches this model or data scope. It was not replaced or executed.");
      if (active.current) { setPreview(next); setConfirmed(false); }
    } catch (error) { if (active.current) setError((error as Error).message); }
    finally { pending.current = false; if (active.current) setBusy(false); }
  }
  async function propose() {
    if (pending.current || !task || !preview || (!savedConsent&&!confirmed) || ((!callId||savedConsent) && projectBudgetAvailability(preview.project_call_limit).blocked)) return;
    pending.current = true; setBusy(true); setError("");
    const id = callId || crypto.randomUUID(); setCallId(id);
    frozen.current??={call_id:id,model_id:preview.model_id,scope_hash:preview.scope_hash,expires_at:preview.expires_at,allow_unknown_cost:true};
    try {
      const saved = await api.proposeConversationSchema(project,conversation,task.input.id,frozen.current);
      if (active.current){setReceipt(saved);setSavedConsent(undefined);frozen.current=undefined;}
    } catch (error) { if (active.current) {
      setError((error as Error).message);
      try{const original=await api.pendingSchemaAuthorization(project,conversation,task.input.id);
        if(!active.current)return;
        if(original){setSavedConsent(original);frozen.current=original;setCallId(original.call_id);}
        else {
          const calls=await api.conversationSchemaCalls(project,conversation,task.input.id);
          if(!active.current)return;
          const completed=calls.find(value=>value.id===id);
          if(completed){setReceipt(completed);setSavedConsent(undefined);frozen.current=undefined;setError("");}
          else if(error instanceof ApiRequestError && [400,401,403,404,409,422,429].includes(error.status)){setCallId("");setSavedConsent(undefined);frozen.current=undefined;}
        }
      }catch{/* Keep the frozen envelope while the server outcome is unknown. */}
    } }
    finally { pending.current = false; if (active.current) setBusy(false); }
  }
  async function stop() {
    if (!task || !callId) return;
    try { await api.cancelConversationSchema(project,conversation,task.input.id,callId); if (active.current) setCancelled(true); }
    catch (error) { if (active.current) setError((error as Error).message); }
  }
  async function cancelQuestion(){
    if(!task||!receipt||pending.current)return;
    if(!window.confirm("Cancel this unanswered clarification? Unsaved label input will be discarded. The question and previous model usage stay saved; this task will not continue from this question."))return;
    pending.current=true;setBusy(true);setError("");
    try{
      await api.cancelSchemaClarification(project,conversation,task.input.id,{call_id:receipt.id,expected_schema_revision:task.input.schema_revision});
      if(active.current){setCancelled(true);setManual(false);onDirtyChange(false);}
    }catch(error){if(active.current)setError(`${(error as Error).message} Cancellation is not confirmed here. Retry the same cancellation or refresh to check saved state.`);}
    finally{pending.current=false;if(active.current)setBusy(false);}
  }
  const decision = receipt?.evidence?.decision?.Ok;
  const clarification = decision?.decision==="clarify"&&task&&receipt ? {call_id:receipt.id,expected_schema_revision:task.input.schema_revision,question:decision.question??""}:undefined;
  const waiting = Boolean(!manual && !cancelled && callId && (!savedConsent||busy) && (!receipt || receipt.status === "reserved"));
  const originalProposal = decision && <div className="conversation-proposal-result">{decision.question && <p>{decision.question}</p>}{decision.kind && <p>{decision.kind === "bounding_box" ? "Object boxes" : "Whole-image categories"}</p>}{decision.labels && <ul>{decision.labels.map((label) => <li key={label}>{label}</li>)}</ul>}<p>{decision.rationale}</p>{decision.boundary_rules?.map((rule) => <p key={rule}>{rule}</p>)}<small>Original model proposal. Editable Schema revisions and Pipeline Drafts are separate; no formal annotations have been accepted.</small></div>;
  if(initialJourney&&!manual)return <section className="conversation-schema-card" aria-label="Annotation Schema proposal">
    {task?<ConversationJourneyCard key={`${project}:${conversation}:${task.input.id}`} project={project} conversation={conversation} task={task.input.id} disabled={busy} prepareRequested={initialPrepare} onSample={onSample} onAssistance={onAssistance} onActiveChange={setJourneyActive} onSchemaOutcome={value=>{setReceipt(value);setCallId(value.id);setSavedConsent(undefined);setInitialJourney(false);setJourneyActive(false);}}/>:<><h3>Start from your annotation goal</h3><p>Review one bounded request for labels, an annotation plan and up to three sample images. No dataset annotations are accepted.</p><button className="primary" disabled={!ready||busy} onClick={()=>void prepareInitial()}>Prepare annotation request</button></>}
    {!journeyActive&&<div className="button-row"><button disabled={!ready||busy} onClick={()=>{setInitialJourney(false);void prepare();}}>Prepare label proposal</button><button disabled={!ready||busy} onClick={()=>{setInitialJourney(false);void prepare(true);}}>Define labels myself · no LLM needed</button></div>}
    {error&&<p role="alert">{error}</p>}
    {onSetup&&<button disabled={busy||journeyActive} onClick={()=>onSetup(task?.input.id)}>Review model setup</button>}
  </section>;
  return <section className="conversation-schema-card" aria-label="Annotation Schema proposal">
    <h3>Plan the annotation goal</h3>
    {!receipt&&!manual&&<p>Planning may use a paid text model. Image processing requires a separate authorization.</p>}
    {savedConsent&&!cancelled&&!receipt&&<aside className="conversation-consent" aria-label="Saved Schema authorization"><p>The original Schema request is saved, but no model call was admitted. No automatic retry is running.</p><small>Original model binding retained · Authorization expires {savedConsent.expires_at}. Refreshing the budget does not renew this consent.</small><button disabled={busy} onClick={()=>void prepare()}>Review saved Schema request</button><button disabled={busy} onClick={()=>void stop()}>Cancel saved Schema request</button></aside>}
    {!manual && cancelled && !clarification && <p role="status">Cancellation saved. No automatic retry will be started; any saved model outcome remains below.</p>}
    {!manual && !cancelled && !receipt && !preview && !savedConsent && <><p>AnnotAgent can propose labels and an output type from your saved goal. You will review the model and data scope before any call.</p><button disabled={!ready || busy} onClick={() => void prepare()}>{busy ? "Checking model…" : "Prepare label proposal"}</button></>}
    {!manual&&!cancelled&&!receipt&&!savedConsent&&!waiting&&<button disabled={!ready||busy} onClick={()=>void prepareInitial()}>Review combined planning and sample authorization</button>}
    {!manual && !waiting && !cancelled && (!receipt||clarification) && <button disabled={!ready||busy||Boolean(savedConsent)} onClick={()=>void prepare(true)}>{clarification?"Answer this clarification":"Define labels myself · no LLM needed"}</button>}
    {clarification&&!humanSchema&&!cancelled&&!manual&&<button disabled={busy} onClick={()=>void cancelQuestion()}>Cancel clarification</button>}
    {clarification&&!humanSchema&&cancelled&&<p role="status">Clarification cancelled. No answer was saved and this task will not continue. The original question and usage remain in history. Save a new goal to start a separate task.</p>}
    {manual && task && !(clarification&&cancelled&&!humanSchema) && <ConversationHumanSchema clarification={clarification} onCancelClarification={clarification?()=>void cancelQuestion():undefined} cancelling={busy} key={task.input.id} project={project} conversation={conversation} task={task.input.id} schemaId={humanSchema} onSaved={setHumanSchema} onDirtyChange={onDirtyChange} onSample={onSample} onAssistance={onAssistance}/>}
    {!manual && !cancelled && preview && !receipt && <div className="conversation-consent" aria-label="Schema model authorization"><ConversationBudgetNotice value={preview.project_call_limit} maximumCalls={preview.maximum_calls} busy={busy || Boolean(callId&&!savedConsent)} onRefresh={()=>void prepare()}/><strong>{preview.model_name}</strong><span>{preview.remote_model} · {preview.destination}</span><p>{preview.data_scope}</p><p>{preview.maximum_calls} model call · {preview.maximum_output_tokens} maximum output tokens · Cost unknown</p><p>{preview.operation}</p>{!savedConsent&&<label><input type="checkbox" checked={confirmed} disabled={busy} onChange={(event) => setConfirmed(event.target.checked)} />Allow this text request; I understand the actual cost is unknown</label>}<div className="button-row"><button disabled={busy} onClick={() => { setPreview(undefined); setConfirmed(false); }}>Back</button><button className="primary" disabled={(!savedConsent&&!confirmed) || busy || ((!callId||Boolean(savedConsent)) && projectBudgetAvailability(preview.project_call_limit).blocked)} onClick={() => void propose()}>{busy ? "Request submitted…" : savedConsent ? "Retry saved Schema request" : callId ? "Retry same request" : "Generate label proposal"}</button></div></div>}
    {waiting && <><p role="status">Request recorded or being submitted. Checking the saved outcome; no automatic model retry.</p><button onClick={() => void stop()}>Stop Schema request</button><small>You may return to the Project; leaving does not cancel this request.</small></>}
    {!manual && preview && !receipt && <p className="muted">A valid label proposal is saved as an editable Draft automatically. It does not change the Project's formal labels, build a pipeline or start image processing.</p>}
    {!manual && decision && <><strong>{decision.decision === "clarify" ? cancelled ? "Original question · Cancelled" : "Clarification needed" : "Schema proposal saved"}</strong>{decision.decision === "draft" ? <details className="conversation-proposal-details" key={receipt?.id}><summary>View original label proposal and rationale</summary>{originalProposal}</details> : originalProposal}</>}
    {!manual && receipt && receipt.status !== "reserved" && !decision && <p role="alert">{receipt.evidence?.error || receipt.evidence?.decision?.Err || "No valid Schema proposal was produced. Saved evidence is retained; no automatic retry."}</p>}
    {!manual && decision?.decision === "draft" && task && receipt && <ConversationSchemaEditor key={receipt.id} project={project} conversation={conversation} task={task.input.id} call={receipt.id} onDirtyChange={onDirtyChange} onSample={onSample} onAssistance={onAssistance} />}
    {error && <p role="alert">{error}</p>}
    {onSetup && <button disabled={busy || waiting} onClick={()=>onSetup(task?.input.id)}>Review model setup</button>}
  </section>;
}
