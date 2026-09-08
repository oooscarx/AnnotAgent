import { useEffect, useRef, useState } from "react";
import { t, useLocale } from "../i18n";
import { api, ApiRequestError } from "../api";
import { feedbackCanDiscardUnaccepted, feedbackCancellationMatches } from "../conversation-feedback";
import { futureProposalApi, type FutureProposalPreview, type FutureProposalSource, type FutureProposalStatus, type PendingFutureProposal } from "../conversation-future-proposal-api";
import { futureProposalPhase, futureProposalNeedsPolling, futureProposalSourceMatches, mergeFutureProposal, parseFutureProposalPending, futureProposalDefinition } from "../conversation-future-proposal";
import type { ConversationCallCancellation } from "../types";
import { ConversationBudgetNotice } from "./ConversationBudgetNotice";
import { projectBudgetAvailability } from "../projectBudget";
import { FutureSchemaDiff } from "./FutureSchemaDiff";

/** A bounded semantic suggestion; this component never creates or adopts a Schema. */
export function ConversationFutureSchemaProposal({ project, conversation, task, source, cancelled: parentCancelled, adopted, editing, onUse, onDirtyChange, onStatus, onSetup, onAssistance }: {
  project: string; conversation: string; task: string; source: FutureProposalSource; cancelled: boolean; adopted: boolean; editing: boolean;
  onUse: (value: FutureProposalStatus, edit: boolean) => Promise<void>; onDirtyChange: (dirty: boolean) => void;
  onStatus: (value: FutureProposalStatus) => void; onSetup?: () => void; onAssistance?: () => void;
}) {
  useLocale();
  const storageKey = `annotagent.future-rule-call:${project}:${conversation}:${task}:${source.feedback_call_id}`;
  const [saved, setSaved] = useState<FutureProposalStatus>();
  const [preview, setPreview] = useState<FutureProposalPreview>();
  const [ready, setReady] = useState(false), [busy, setBusy] = useState(false), [dispatching, setDispatching] = useState(false), [confirmed, setConfirmed] = useState(false), [error, setError] = useState("");
  const [cancellation, setCancellation] = useState<ConversationCallCancellation>();
  const cancellationReceipt = useRef<ConversationCallCancellation | undefined>(undefined);
  const pending = useRef(false), frozen = useRef<PendingFutureProposal | undefined>(undefined), revision = useRef(0), alive = useRef(true), dirtyCallback = useRef(onDirtyChange), statusCallback = useRef(onStatus);
  dirtyCallback.current = onDirtyChange; statusCallback.current = onStatus;
  const activeCall = saved?.authorization.consent.call_id ?? frozen.current?.consent.call_id;
  const cancelled = parentCancelled || Boolean(saved?.cancelled || feedbackCancellationMatches(cancellation, task, activeCall));
  const phase = cancelled ? "cancelled" : futureProposalPhase(saved);
  function retain(value: PendingFutureProposal) {
    frozen.current = value;
    try { sessionStorage.setItem(storageKey, JSON.stringify(value)); dirtyCallback.current(false); }
    catch { dirtyCallback.current(true); }
  }
  function clear() { frozen.current = undefined; dirtyCallback.current(false); try { sessionStorage.removeItem(storageKey); } catch { /* Saved authority remains server-owned. */ } }
  function recordCancellation(value: ConversationCallCancellation) {
    cancellationReceipt.current = value; setCancellation(value);
  }
  function apply(value: FutureProposalStatus) {
    if (!futureProposalSourceMatches(value.authorization.source, source) || !futureProposalSourceMatches(value.authorization.context.source, source) || value.authorization.context.scope !== "future_tasks_only" || value.authorization.context.base_schema.id !== source.base_schema_id || value.authorization.context.base_schema.revision !== source.base_schema_revision || value.authorization.context.base_schema.task_id !== task || value.authorization.grant.task_id !== task || (frozen.current && value.authorization.consent.call_id !== frozen.current.consent.call_id)) throw new Error("The saved future-rule proposal belongs to a different source. No rules were substituted.");
    setSaved(current => mergeFutureProposal(current, value)); statusCallback.current(value);
    if (frozen.current?.consent.call_id === value.authorization.consent.call_id) clear();
  }
  async function read(signal?: AbortSignal, ticket = revision.current) {
    const value = await futureProposalApi.read(project, conversation, task, source.feedback_call_id, signal);
    if (!alive.current || signal?.aborted || ticket !== revision.current) return value;
    if (value) apply(value);
    else if (frozen.current) {
      const records = await api.conversationSchemaCancellations(project, conversation, task, signal);
      if (!alive.current || signal?.aborted || ticket !== revision.current) return value;
      const stopped = records.find(item => feedbackCancellationMatches(item, task, frozen.current?.consent.call_id));
      if (stopped) recordCancellation(stopped);
    }
    return value;
  }
  useEffect(() => {
    alive.current = true; const controller = new AbortController(), ticket = ++revision.current;
    try { frozen.current = parseFutureProposalPending(sessionStorage.getItem(storageKey), source.feedback_call_id, source.scope_answer_command_id); } catch { /* No new identity inferred from storage failure. */ }
    void read(controller.signal, ticket).then(() => { if (!controller.signal.aborted && ticket === revision.current) setReady(true); }).catch((reason: Error) => { if (!controller.signal.aborted && ticket === revision.current) { setError(reason.message); setReady(true); } });
    return () => { alive.current = false; revision.current++; controller.abort(); dirtyCallback.current(false); };
  }, [storageKey]);
  const polling = !cancelled && futureProposalNeedsPolling(saved, dispatching);
  useEffect(() => {
    if (!polling) return;
    const controller = new AbortController(); let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      const ticket = revision.current;
      try { const value = await read(controller.signal, ticket); if (!controller.signal.aborted && ticket === revision.current && value && !futureProposalNeedsPolling(value, dispatching)) { setDispatching(false); onAssistance?.(); } }
      catch (reason) { if (!controller.signal.aborted && ticket === revision.current) setError((reason as Error).message); }
      if (!controller.signal.aborted) timer = setTimeout(() => void poll(), 1500);
    };
    timer = setTimeout(() => void poll(), 800);
    return () => { controller.abort(); clearTimeout(timer); };
  }, [polling, saved?.authorization.consent.call_id, dispatching]);
  async function reload() {
    if (pending.current) return; pending.current = true; setBusy(true); setError(""); const ticket = ++revision.current;
    try { await read(undefined, ticket); if (alive.current && ticket === revision.current) { setReady(true); onAssistance?.(); } }
    catch (reason) { if (alive.current && ticket === revision.current) setError((reason as Error).message); }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  async function prepare() {
    if (pending.current || !ready || cancelled || adopted || editing || saved || frozen.current) return;
    pending.current = true; setBusy(true); setConfirmed(false); setError(""); const ticket = ++revision.current;
    try {
      const value = await futureProposalApi.preview(project, conversation, task, source.feedback_call_id, preview?.consent.call_id ?? crypto.randomUUID());
      if (!futureProposalSourceMatches(value.source, source)) throw new Error("The tested rules or feedback scope changed. Reload the original source before authorizing.");
      if (alive.current && ticket === revision.current) setPreview(value);
    } catch (reason) { if (alive.current && ticket === revision.current) setError((reason as Error).message); }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  async function start() {
    if (pending.current || !ready || cancelled || adopted || editing || (saved && phase !== "authorized")) return;
    if (!saved && !frozen.current && (!preview || !confirmed || projectBudgetAvailability(preview.project_call_limit).blocked)) return;
    const envelope: PendingFutureProposal = saved ? { consent: saved.authorization.consent, source: saved.authorization.source, summary: saved.authorization.summary } : frozen.current ?? { consent: { ...preview!.consent, allow_unknown_cost: true }, source: preview!.source, summary: { model_name: preview!.model_name, remote_model: preview!.remote_model, destination: preview!.destination, data_scope: preview!.data_scope, operation: preview!.operation, maximum_output_tokens: preview!.maximum_output_tokens } };
    if (!futureProposalSourceMatches(envelope.source, source)) { setError("The saved request has a different tested source. It was not executed or replaced."); return; }
    if (!Number.isFinite(Date.parse(envelope.consent.expires_at)) || Date.parse(envelope.consent.expires_at) <= Date.now()) { setError("This future-rule authorization expired. It was not renewed or executed."); return; }
    pending.current = true; setBusy(true); setError(""); const ticket = ++revision.current; retain(envelope);
    let acknowledged = Boolean(saved);
    try {
      const authorization = await futureProposalApi.authorize(project, conversation, task, source.feedback_call_id, envelope.consent); acknowledged = true;
      if (!alive.current || ticket !== revision.current) return;
      apply(authorization); setPreview(undefined); setConfirmed(false);
      if (authorization.cancelled || authorization.receipt) { onAssistance?.(); return; }
      setDispatching(true);
      const value = await futureProposalApi.execute(project, conversation, task, source.feedback_call_id, envelope.consent.call_id);
      if (alive.current && ticket === revision.current) { apply(value); onAssistance?.(); }
    } catch (reason) {
      if (!alive.current || ticket !== revision.current) return;
      setError((reason as Error).message);
      try {
        const value = await read(undefined, ticket);
        if (!alive.current || ticket !== revision.current) return;
        // A different tab may have stopped this exact, not-yet-authorized call.
        // Its tombstone needs the original envelope to remain addressable after reload.
        if (!value && !feedbackCancellationMatches(cancellationReceipt.current, task, envelope.consent.call_id) && feedbackCanDiscardUnaccepted({ httpStatus: reason instanceof ApiRequestError ? reason.status : undefined, authorizationAcknowledged: acknowledged, lookupConfirmedEmpty: true })) { clear(); setPreview(undefined); setConfirmed(false); }
      } catch { /* Unknown acknowledgements keep the exact consent; no automatic execution. */ }
    } finally { pending.current = false; if (alive.current) { setBusy(false); if (ticket === revision.current) setDispatching(false); } }
  }
  async function stop() {
    const call = saved?.authorization.consent.call_id ?? frozen.current?.consent.call_id;
    if (!call) return;
    const ticket = ++revision.current; setError("");
    try {
      const receipt = await api.cancelConversationSchema(project, conversation, task, call);
      if (!alive.current || ticket !== revision.current) return;
      if (!feedbackCancellationMatches(receipt, task, call)) throw new Error("The cancellation receipt belongs to another request.");
      recordCancellation(receipt); setDispatching(false); onAssistance?.();
      await read(undefined, ticket);
    } catch (reason) { if (alive.current && ticket === revision.current) setError((reason as Error).message); }
  }
  async function useProposal(edit: boolean) {
    if (pending.current || !saved || cancelled || adopted || editing || !["draft", "clarify"].includes(phase)) return;
    pending.current = true; setBusy(true); setError("");
    try { await onUse(saved, edit); } catch (reason) { if (alive.current) setError((reason as Error).message); }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  const proposal = saved?.proposal && "Ok" in saved.proposal ? saved.proposal.Ok : undefined;
  let definition; let invalidDefinition = "";
  if (proposal?.decision.decision === "draft" && saved) { try { definition = futureProposalDefinition(saved.authorization.context.base_schema, proposal); } catch (reason) { invalidDefinition = (reason as Error).message; } }
  const checking = dispatching && !saved?.receipt && !cancelled;
  const recoverable = phase === "authorized" && !dispatching;
  if (adopted && !saved && !frozen.current && ready && !error) return null;
  return <section className="conversation-future-model" aria-label={t("Future rule model proposal")}>
    <h5>{t("Let AnnotAgent suggest future rules")}</h5><p>Use the saved feedback and tested rules for one text-only suggestion. No images are sent and no Schema, workflow or annotation is changed until you save a reviewed draft.</p>
    {!ready && <p role="status">{t("Restoring saved future-rule proposal…")}</p>}
    {!saved && !preview && !frozen.current && !cancelled && !adopted && <button disabled={!ready || busy || editing} onClick={() => void prepare()}>{t("Review model proposal authorization")}</button>}
    {preview && !saved && !frozen.current && !cancelled && !adopted && <section className="conversation-feedback-consent" aria-label={t("Future rule model authorization")}>
      <strong>{preview.model_name}</strong><span>{preview.remote_model} · {preview.destination}</span><p>{preview.data_scope}</p><p>{preview.operation}</p><p>One text-only call · No image pixels · Cost unknown · Expires {new Date(preview.consent.expires_at).toLocaleString()}</p><small>{preview.used_calls} calls already used · {preview.cumulative_maximum_calls} cumulative maximum</small>
      <ConversationBudgetNotice value={preview.project_call_limit} maximumCalls={1} busy={busy} onRefresh={() => void prepare()} />
      <label><input type="checkbox" checked={confirmed} disabled={busy} onChange={event => setConfirmed(event.target.checked)} /><span>{t("Allow this one future-rule text request; actual cost is unknown")}</span></label>
      <div className="conversation-feedback-actions"><button disabled={busy} onClick={() => { setPreview(undefined); setConfirmed(false); }}>{t("Back")}</button><button className="primary" disabled={busy || editing || !confirmed || projectBudgetAvailability(preview.project_call_limit).blocked} onClick={() => void start()}>{t("Propose future rules")}</button></div>
    </section>}
    {saved && <>{adopted && <p>A separate future-rule draft was saved. This model request is tracked independently; saving a draft does not stop it or change earlier results.</p>}<p role="status">{cancelled ? "Future-rule model request cancelled. Prior usage remains; cancellation does not undo a separately saved draft." : phase === "running" ? "Request admitted. Generating a future-rule suggestion; this request does not save a Schema." : checking ? "Execution submitted. Checking the saved admission and outcome; no automatic retry." : phase === "authorized" ? adopted ? "Future-rule authorization remains saved, but no model call is recorded. It can be cancelled; the separate draft does not start it." : "Future-rule authorization saved; no call is recorded. Continue explicitly to execute." : phase === "expired" ? "Future-rule authorization expired; no automatic renewal." : phase === "unknown" ? "Provider outcome unknown. The request may have been billed and will not be sent again." : phase === "failed" || phase === "invalid" ? "No valid future-rule suggestion was produced. The original rules are unchanged." : adopted ? "Original future-rule suggestion retained in history." : phase === "clarify" ? "A rule clarification is needed" : "Future-rule suggestion saved; not yet adopted"}</p>
      {proposal && <><p>{proposal.decision.rationale}</p>{proposal.decision.decision === "clarify" && <p>{proposal.decision.question}</p>}</>}
      {definition && !editing && !adopted && <FutureSchemaDiff before={saved.authorization.context.base_schema.definition} after={definition} />}
      {!cancelled && !adopted && !editing && definition && <div className="conversation-feedback-actions"><button disabled={busy} onClick={() => void useProposal(true)}>{t("Edit proposed future rules")}</button><button className="primary" disabled={busy} onClick={() => void useProposal(false)}>{t("Save proposed future rule draft")}</button></div>}
      {!cancelled && !adopted && !editing && phase === "clarify" && <button disabled={busy} onClick={() => void useProposal(true)}>{t("Answer by editing future rules")}</button>}
      {recoverable && !cancelled && !adopted && <><p>{saved.authorization.summary.model_name} · {saved.authorization.summary.remote_model} · {saved.authorization.summary.destination}</p><p>{saved.authorization.summary.data_scope}</p><small>One text request · Cost unknown · Expires {new Date(saved.authorization.consent.expires_at).toLocaleString()}</small><button disabled={busy || editing} onClick={() => void start()}>{t("Continue saved future rule request")}</button></>}
      {saved.proposal && "Err" in saved.proposal && <p role="alert">{saved.proposal.Err}</p>}{(saved.error || saved.receipt?.evidence?.error || invalidDefinition) && <p role="alert">{saved.error || saved.receipt?.evidence?.error || invalidDefinition}</p>}
    </>}
    {!saved && cancelled && <p role="status">Cancellation saved for this future-rule request. No model receipt is being assumed.</p>}
    {!saved && frozen.current && !cancelled && <><p role="status">Future-rule request acknowledgement unknown. The original source, model, call identity and expiry are retained.</p><button disabled={!ready || busy || editing} onClick={() => void start()}>{t("Retry same future rule request")}</button></>}
    {!cancelled && (phase === "running" || checking || recoverable || Boolean(frozen.current)) && <button onClick={() => void stop()}>{t("Stop future rule request")}</button>}
    {busy && <p role="status">{t("Checking or saving the current request…")}</p>}
    {error && <p role="alert">{error} Reload reads saved state without starting a model call.</p>}
    {(saved || frozen.current || error || !ready) && <button disabled={busy} onClick={() => void reload()}>{t("Reload saved future-rule proposal")}</button>}
    {onSetup && !saved && !frozen.current && !cancelled && !adopted && <button disabled={busy} onClick={onSetup}>{t("Review model setup")}</button>}
  </section>;
}
