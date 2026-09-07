import { useEffect, useRef, useState } from "react";
import { api, ApiRequestError } from "../api";
import type { ConversationCallReceipt, ConversationSchemaPreview, ConversationTask } from "../types";
import { ConversationSchemaEditor } from "./ConversationSchemaEditor";
import type { OpenConversationSample } from "./ConversationSampleCard";

/** Restores server objects; mounting never creates a task or invokes a model. */
export function ConversationSchemaCard({ project, conversation, message, onDirtyChange, onSample }: { project: string; conversation: string; message: string; onDirtyChange: (dirty: boolean) => void; onSample: OpenConversationSample }) {
  const [task, setTask] = useState<ConversationTask>();
  const [preview, setPreview] = useState<ConversationSchemaPreview>();
  const [receipt, setReceipt] = useState<ConversationCallReceipt>();
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [error, setError] = useState("");
  const [callId, setCallId] = useState("");
  const [cancelled, setCancelled] = useState(false);
  const pending = useRef(false);
  const active = useRef(true);
  useEffect(() => {
    active.current = true; const controller = new AbortController();
    void api.conversationTasks(project, conversation, controller.signal).then(async (tasks) => {
      const current = tasks.find((item) => item.input.source_message_id === message);
      const calls = current ? await api.conversationSchemaCalls(project, conversation, current.input.id, controller.signal) : [];
      const cancellations = current ? await api.conversationSchemaCancellations(project, conversation, current.input.id, controller.signal) : [];
      if (controller.signal.aborted) return;
      const id = calls[0]?.id ?? cancellations.at(-1)?.call_id ?? "";
      setTask(current); setReceipt(calls[0]); setCallId(id); setCancelled(cancellations.some((item) => item.call_id === id)); setReady(true);
    }).catch((error: Error) => { if (!controller.signal.aborted) setError(error.message); });
    return () => { active.current = false; controller.abort(); };
  }, [project, conversation, message]);
  useEffect(() => {
    if (!task || !callId || (cancelled && !receipt) || (receipt && receipt.status !== "reserved")) return;
    const controller = new AbortController();
    const poll = () => void api.conversationSchemaCalls(project, conversation, task.input.id, controller.signal).then((calls) => {
      if (!controller.signal.aborted) { const current = calls.find((item) => item.id === callId); if (current) setReceipt(current); }
    }).catch((error: Error) => { if (!controller.signal.aborted) setError(error.message); });
    poll(); const interval = window.setInterval(poll, 1500);
    return () => { controller.abort(); window.clearInterval(interval); };
  }, [project, conversation, task?.input.id, callId, receipt?.status, cancelled]);
  async function prepare() {
    if (pending.current) return; pending.current = true; setBusy(true); setError("");
    try {
      const goal = await api.projectGoal(project);
      const current = task ?? await api.beginConversationTask(project,conversation,{id:crypto.randomUUID(),source_message_id:message,schema_revision:goal.revision});
      if (!active.current) return;
      setTask(current);
      const next = await api.conversationSchemaPreview(project,conversation,current.input.id);
      if (active.current) { setPreview(next); setConfirmed(false); }
    } catch (error) { if (active.current) setError((error as Error).message); }
    finally { pending.current = false; if (active.current) setBusy(false); }
  }
  async function propose() {
    if (pending.current || !task || !preview || !confirmed) return;
    pending.current = true; setBusy(true); setError("");
    const id = callId || crypto.randomUUID(); setCallId(id);
    try {
      const saved = await api.proposeConversationSchema(project,conversation,task.input.id,{call_id:id,model_id:preview.model_id,scope_hash:preview.scope_hash,expires_at:preview.expires_at,allow_unknown_cost:true});
      if (active.current) setReceipt(saved);
    } catch (error) { if (active.current) {
      setError((error as Error).message);
      if (error instanceof ApiRequestError && [400,401,403,404,409,422,429].includes(error.status)) setCallId("");
    } }
    finally { pending.current = false; if (active.current) setBusy(false); }
  }
  async function stop() {
    if (!task || !callId) return;
    try { await api.cancelConversationSchema(project,conversation,task.input.id,callId); if (active.current) setCancelled(true); }
    catch (error) { if (active.current) setError((error as Error).message); }
  }
  const decision = receipt?.evidence?.decision?.Ok;
  const waiting = Boolean(!cancelled && callId && (!receipt || receipt.status === "reserved"));
  return <section className="conversation-schema-card" aria-label="Annotation Schema proposal">
    <h3>Define the annotation goal</h3>
    {cancelled && <p role="status">Cancellation saved. No automatic retry will be started; any saved model outcome remains below.</p>}
    {!cancelled && !receipt && !preview && <><p>AnnotAgent can propose labels and an output type from your saved goal. You will review the model and data scope before any call.</p><button disabled={!ready || busy} onClick={() => void prepare()}>{busy ? "Checking model…" : "Prepare label proposal"}</button></>}
    {!cancelled && preview && !receipt && <div className="conversation-consent" aria-label="Schema model authorization"><strong>{preview.model_name}</strong><span>{preview.remote_model} · {preview.destination}</span><p>{preview.data_scope}</p><p>{preview.maximum_calls} model call · {preview.maximum_output_tokens} maximum output tokens · Cost unknown</p><p>{preview.operation}</p><label><input type="checkbox" checked={confirmed} disabled={busy} onChange={(event) => setConfirmed(event.target.checked)} />Allow this text request; I understand the actual cost is unknown</label><div className="button-row"><button disabled={busy} onClick={() => { setPreview(undefined); setConfirmed(false); }}>Back</button><button className="primary" disabled={!confirmed || busy} onClick={() => void propose()}>{busy ? "Request submitted…" : callId ? "Retry same request" : "Generate label proposal"}</button></div></div>}
    {waiting && <><p role="status">Request recorded or being submitted. Checking the saved outcome; no automatic model retry.</p><button onClick={() => void stop()}>Stop Schema request</button><small>You may return to the Project; leaving does not cancel this request.</small></>}
    {decision && <div className="conversation-proposal-result"><strong>{decision.decision === "clarify" ? "Clarification needed" : "Schema proposal saved"}</strong>{decision.question && <p>{decision.question}</p>}{decision.kind && <p>{decision.kind === "bounding_box" ? "Object boxes" : "Whole-image categories"}</p>}{decision.labels && <ul>{decision.labels.map((label) => <li key={label}>{label}</li>)}</ul>}<p>{decision.rationale}</p>{decision.boundary_rules?.map((rule) => <p key={rule}>{rule}</p>)}<small>Original model proposal. Editable Schema revisions and Pipeline Drafts are separate; no formal annotations have been accepted.</small></div>}
    {receipt && receipt.status !== "reserved" && !decision && <p role="alert">{receipt.evidence?.error || receipt.evidence?.decision?.Err || "No valid Schema proposal was produced. Saved evidence is retained; no automatic retry."}</p>}
    {decision?.decision === "draft" && task && receipt && <ConversationSchemaEditor key={receipt.id} project={project} conversation={conversation} task={task.input.id} call={receipt.id} onDirtyChange={onDirtyChange} onSample={onSample} />}
    {error && <p role="alert">{error}</p>}
  </section>;
}
