import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiRequestError } from "../api";
import { feedbackApi, type FeedbackConsent, type FeedbackPreview, type FeedbackStatus, type ScopeAnswerInput, type ScopeAnswerRecord } from "../conversation-feedback-api";
import { feedbackCancellationMatches, feedbackCanDiscardUnaccepted, feedbackNeedsPolling, feedbackPhase, feedbackWaitingRequest, mergeFeedbackStatus, parsePendingFeedback } from "../conversation-feedback";
import type { HumanRequest } from "../conversation-human-api";
import type { ConversationCallCancellation, ConversationMessage } from "../types";
import { projectBudgetAvailability } from "../projectBudget";
import { ConversationBudgetNotice } from "./ConversationBudgetNotice";
import { ConversationFeedbackScope } from "./ConversationFeedbackScope";
import { sameScopeAnswerInput } from "../conversation-feedback-scope";
import { ConversationFutureSchemaCard } from "./ConversationFutureSchemaCard";
import { ConversationImageClassCard } from "./ConversationImageClassCard";
import type { ImageClassReview } from "../conversation-image-class-api";
import type { OpenConversationSample } from "./ConversationSampleCard";
import "./conversation-feedback.css";

/** A saved message is context, not permission. Only explicit buttons authorize or execute. */
type FeedbackCardProps = {
  project: string; message: ConversationMessage; requests: HumanRequest[]; requestsReady: boolean;
  onOpen: (request: HumanRequest) => void; onAssistance: () => void;
  captureCanvasNavigation: () => (request: HumanRequest) => void;
  captureClassNavigation?: () => (review: ImageClassReview) => void;
  onScopeDirtyChange?: (dirty: boolean) => void;
  onSample: OpenConversationSample;
  onSetup?: () => void;
};
export function ConversationFeedbackCard(props: FeedbackCardProps) {
  const reference = props.message.input.reference;
  if (reference?.scope !== "sample_candidate") return null;
  return <FeedbackCard {...props} task={reference.task_id} />;
}
function FeedbackCard({ project, message, requests, requestsReady, onOpen, onAssistance, captureCanvasNavigation, captureClassNavigation, onScopeDirtyChange, onSample, onSetup, task }: FeedbackCardProps & { task: string }) {
  const conversation = message.conversation_id;
  const storageKey = `annotagent.feedback:${project}:${conversation}:${task}:${message.input.id}`;
  const [saved, setSaved] = useState<FeedbackStatus>();
  const [preview, setPreview] = useState<FeedbackPreview>();
  const [ready, setReady] = useState(false), [busy, setBusy] = useState(false), [confirmed, setConfirmed] = useState(false);
  const [dispatchPending, setDispatchPending] = useState(false);
  const [cancellation, setCancellation] = useState<ConversationCallCancellation>();
  const [error, setError] = useState("");
  const alive = useRef(true), pending = useRef(false), frozen = useRef<FeedbackConsent | undefined>(undefined);
  const revision = useRef(0);
  const dirtySources = useRef(new Set<string>()), dirtyCallback = useRef(onScopeDirtyChange);
  dirtyCallback.current = onScopeDirtyChange;
  const setDirtySource = useCallback((source: string, dirty: boolean) => { if (dirty) dirtySources.current.add(source); else dirtySources.current.delete(source); dirtyCallback.current?.(dirtySources.current.size > 0); }, []);
  const scopeDirty = useCallback((dirty: boolean) => setDirtySource("scope", dirty), [setDirtySource]);
  const futureDirty = useCallback((dirty: boolean) => setDirtySource("future", dirty), [setDirtySource]);
  const classDirty = useCallback((dirty: boolean) => setDirtySource("image-class", dirty), [setDirtySource]);
  const waiting = feedbackWaitingRequest(message, requests);
  const cancelled = Boolean(saved?.cancelled || feedbackCancellationMatches(cancellation, task, saved?.authorization.consent.call_id ?? frozen.current?.call_id));
  const phase = cancelled ? "cancelled" : feedbackPhase(saved);
  const running = phase === "running";
  const canCorrect = !cancelled && (phase === "correction" || (phase === "clarify" && saved?.scope_answer?.input.choice.scope === "current_candidate"));

  function clearFrozen() {
    frozen.current = undefined;
    try { sessionStorage.removeItem(storageKey); } catch { /* Saved server authorization remains authoritative. */ }
  }
  function apply(value: FeedbackStatus) {
    if (value.authorization.consent.message_id !== message.input.id || value.authorization.context.message.conversation_id !== conversation || value.authorization.context.message.input.reference?.task_id !== task) {
      throw new Error("The saved feedback does not belong to this message and task. No different candidate was substituted.");
    }
    setSaved(current => mergeFeedbackStatus(current, value));
    if (frozen.current?.call_id === value.authorization.consent.call_id) clearFrozen();
  }
  useEffect(() => {
    alive.current = true; const controller = new AbortController(), ticket = ++revision.current;
    try { frozen.current = parsePendingFeedback(sessionStorage.getItem(storageKey), message.input.id); } catch { /* In-memory and server state still work when browser storage is unavailable. */ }
    void feedbackApi.forMessage(project, conversation, task, message.input.id, controller.signal).then(async value => {
      if (controller.signal.aborted || ticket !== revision.current) return;
      if (value) apply(value);
      else if (frozen.current) {
        const call = frozen.current.call_id;
        const cancellations = await api.conversationSchemaCancellations(project, conversation, task, controller.signal);
        if (controller.signal.aborted || ticket !== revision.current) return;
        const receipt = cancellations.find(item => feedbackCancellationMatches(item, task, call));
        if (receipt) setCancellation(receipt);
      }
      setReady(true);
    }).catch((reason: Error) => { if (!controller.signal.aborted && ticket === revision.current) setError(reason.message); });
    return () => { alive.current = false; revision.current++; controller.abort(); };
  }, [project, conversation, task, message.input.id]);
  useEffect(() => {
    if (cancelled || !feedbackNeedsPolling(saved, dispatchPending) || !saved) return;
    const controller = new AbortController(); let timer: ReturnType<typeof setTimeout>;
    const call = saved.authorization.consent.call_id;
    const poll = async () => {
      const ticket = revision.current;
      try {
        const value = await feedbackApi.status(project, conversation, task, call, controller.signal);
        if (controller.signal.aborted) return;
        if (ticket !== revision.current) { timer = setTimeout(() => void poll(), 1200); return; }
        apply(value);
        if (feedbackNeedsPolling(value, dispatchPending)) timer = setTimeout(() => void poll(), 1200);
        else { setDispatchPending(false); onAssistance(); }
      } catch (reason) {
        if (!controller.signal.aborted) { if (ticket === revision.current) setError((reason as Error).message); timer = setTimeout(() => void poll(), 2000); }
      }
    };
    void poll();
    return () => { controller.abort(); clearTimeout(timer); };
  }, [project, conversation, task, saved?.authorization.consent.call_id, running, dispatchPending, cancelled]);

  async function reload() {
    if (pending.current) return;
    pending.current = true; setBusy(true); const ticket = ++revision.current;
    try {
      const value = await feedbackApi.forMessage(project, conversation, task, message.input.id);
      if (!alive.current || ticket !== revision.current) return;
      if (value) apply(value);
      else if (frozen.current) {
        const call = frozen.current.call_id;
        const cancellations = await api.conversationSchemaCancellations(project, conversation, task);
        if (!alive.current || ticket !== revision.current) return;
        const receipt = cancellations.find(item => feedbackCancellationMatches(item, task, call));
        if (receipt) setCancellation(receipt);
      }
      setReady(true); setError(""); onAssistance();
    } catch (reason) { if (alive.current && ticket === revision.current) setError((reason as Error).message); }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  async function prepare() {
    if (pending.current || !ready || !requestsReady || waiting || saved || frozen.current || cancelled) return;
    pending.current = true; setBusy(true); setError(""); setConfirmed(false); const ticket = ++revision.current;
    try {
      const value = await feedbackApi.preview(project, conversation, task, message.input.id, preview?.consent.call_id ?? crypto.randomUUID());
      if (alive.current && ticket === revision.current) setPreview(value);
    } catch (reason) { if (alive.current && ticket === revision.current) setError((reason as Error).message); }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  async function start() {
    if (pending.current || !ready || !requestsReady || waiting || running || cancelled || (saved && phase !== "authorized")) return;
    if (!saved && !frozen.current && (!preview || !confirmed || projectBudgetAvailability(preview.project_call_limit).blocked)) return;
    const consent = saved?.authorization.consent ?? frozen.current ?? { ...preview!.consent, allow_unknown_cost: true };
    if (!Number.isFinite(Date.parse(consent.expires_at)) || Date.parse(consent.expires_at) <= Date.now()) { setError("This authorization expired. It was not renewed or executed."); return; }
    pending.current = true; setBusy(true); setError(""); const ticket = ++revision.current;
    frozen.current = consent;
    try { sessionStorage.setItem(storageKey, JSON.stringify(consent)); } catch { /* The same envelope is retained in memory; no retry is automatic. */ }
    let authorizationAcknowledged = Boolean(saved);
    try {
      const authorized = await feedbackApi.authorize(project, conversation, task, consent);
      authorizationAcknowledged = true;
      if (!alive.current || ticket !== revision.current) return; // Leaving after save never implicitly executes.
      apply(authorized); setPreview(undefined); setConfirmed(false);
      if (authorized.cancelled || authorized.receipt) { onAssistance(); return; }
      setDispatchPending(true);
      const value = await feedbackApi.execute(project, conversation, task, consent.call_id);
      if (alive.current && ticket === revision.current) { apply(value); onAssistance(); }
    } catch (reason) {
      if (alive.current && ticket === revision.current) {
        setError((reason as Error).message);
        try {
          const rejection = { httpStatus: reason instanceof ApiRequestError ? reason.status : undefined, authorizationAcknowledged, lookupConfirmedEmpty: true };
          if (feedbackCanDiscardUnaccepted(rejection)) {
            const value = await feedbackApi.forMessage(project, conversation, task, message.input.id);
            if (!alive.current || ticket !== revision.current) return;
            if (value) { apply(value); setPreview(undefined); }
            else {
              clearFrozen(); setPreview(undefined); setConfirmed(false);
              setError(`${(reason as Error).message} Authorization was not saved. Review updated authorization before retrying.`);
            }
            return;
          }
          const value = await feedbackApi.status(project, conversation, task, consent.call_id);
          if (alive.current && ticket === revision.current) { apply(value); setPreview(undefined); }
        } catch { /* Keep the original identity. A failed acknowledgement never starts another call. */ }
      }
    } finally { pending.current = false; if (alive.current) { setBusy(false); if (ticket === revision.current) setDispatchPending(false); } }
  }
  async function stop() {
    const call = saved?.authorization.consent.call_id ?? frozen.current?.call_id;
    if (!call) return;
    // Stop is deliberately independent of pending start/authorization acknowledgement.
    const ticket = ++revision.current; setError("");
    let acknowledged = false;
    try {
      const receipt = await api.cancelConversationSchema(project, conversation, task, call);
      if (!alive.current || ticket !== revision.current) return;
      if (!feedbackCancellationMatches(receipt, task, call)) throw new Error("Cancellation receipt does not match this feedback request.");
      acknowledged = true; setCancellation(receipt); setDispatchPending(false); onAssistance();
      const value = await feedbackApi.forMessage(project, conversation, task, message.input.id);
      if (alive.current && ticket === revision.current && value) apply(value);
    } catch (reason) { if (alive.current && ticket === revision.current) setError(`${acknowledged ? "Cancellation is saved, but the remaining feedback status could not be loaded. " : ""}${(reason as Error).message}`); }
  }
  async function correct() {
    if (pending.current || !saved || !canCorrect) return;
    pending.current = true; setBusy(true); setError(""); const ticket = ++revision.current;
    const openIfStillCurrent = captureCanvasNavigation();
    try {
      const value = await feedbackApi.prepareCorrection(project, conversation, task, saved.authorization.consent.call_id);
      if (!alive.current || ticket !== revision.current) return;
      if (!value) throw new Error("No correction request was created. This response needs scope clarification, not a fabricated annotation.");
      onAssistance(); openIfStillCurrent(value);
    } catch (reason) { if (alive.current && ticket === revision.current) setError((reason as Error).message); }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  async function saveScope(input: ScopeAnswerInput): Promise<ScopeAnswerRecord> {
    if (pending.current || !saved || phase !== "clarify" || cancelled || saved.scope_answer) throw new Error("This scope question is not currently available for an answer. Reload its saved state before continuing.");
    const call = saved.authorization.consent.call_id;
    if (input.expected_context_digest !== saved.scope_context_digest) throw new Error("The scope answer does not match this frozen context. Your original choices were retained.");
    pending.current = true; setBusy(true); const ticket = ++revision.current;
    const owned = (answer: ScopeAnswerRecord) => answer.call_id === call && answer.task_id === task && answer.conversation_id === conversation;
    try {
      const answer = await feedbackApi.answerScope(project, conversation, task, call, input);
      if (!owned(answer) || !sameScopeAnswerInput(answer.input, input)) throw new Error("The returned scope answer does not match this task and command. No different choice was substituted.");
      if (alive.current && ticket === revision.current) { apply({ ...saved, scope_answer: answer }); onAssistance(); }
      return answer;
    } catch (reason) {
      try {
        const status = await feedbackApi.status(project, conversation, task, call);
        if (alive.current && ticket === revision.current) { apply(status); onAssistance(); }
        if (status.scope_answer && owned(status.scope_answer) && sameScopeAnswerInput(status.scope_answer.input, input)) return status.scope_answer;
      } catch { /* A missing acknowledgement keeps the exact command; it never resubmits. */ }
      throw reason;
    } finally { pending.current = false; if (alive.current) setBusy(false); }
  }

  const decision = saved?.decision && "Ok" in saved.decision ? saved.decision.Ok : undefined;
  const invalid = saved?.decision && "Err" in saved.decision ? saved.decision.Err : undefined;
  const checking = dispatchPending && !saved?.receipt && !cancelled;
  const recoverable = Boolean(saved && phase === "authorized" && !dispatchPending);
  return <section className="conversation-feedback-card" aria-label="Feedback on selected candidate">
    <h3>Feedback on this candidate</h3>
    {!ready && <p role="status">Restoring saved feedback…</p>}
    {!saved && !preview && !frozen.current && !waiting && !cancelled && <><p>Interpret this saved message and result metadata, then ask you for a correction if needed. The text model receives no image pixels.</p><button disabled={!ready || !requestsReady || busy} onClick={() => void prepare()}>Review feedback authorization</button></>}
    {waiting && !running && <aside className="conversation-feedback-waiting"><p>{waiting.sameSubject ? "A correction request for this candidate is already waiting. You can answer it directly without another model call." : "This task already has a waiting human request. No new feedback call can start until it is resolved."}{waiting.request.deferred ? " It is deferred; opening it does not reopen or answer it." : ""}</p><button disabled={busy} onClick={() => onOpen(waiting.request)}>{waiting.sameSubject ? "Open existing correction request" : "Open task’s waiting request"}</button></aside>}
    {preview && !saved && !frozen.current && !cancelled && <section className="conversation-feedback-consent" aria-label="Feedback interpretation authorization">
      <strong>{preview.model_name}</strong><span>{preview.remote_model} · {preview.destination}</span>
      <p>{preview.data_scope}</p><p>{preview.operation}</p><p>One text-only call · No image pixels · Cost unknown. Permission expires {new Date(preview.consent.expires_at).toLocaleString()}.</p>
      <small>{preview.used_calls} task calls already used · {preview.cumulative_maximum_calls} cumulative maximum after authorization. No annotation, label rule, plan or formal result will be changed by this interpretation.</small>
      <ConversationBudgetNotice value={preview.project_call_limit} maximumCalls={1} busy={busy} onRefresh={() => void prepare()} />
      <label><input type="checkbox" checked={confirmed} disabled={busy || Boolean(waiting)} onChange={event => setConfirmed(event.target.checked)} /><span>Allow this one text request; actual cost is unknown</span></label>
      <div className="conversation-feedback-actions"><button disabled={busy} onClick={() => { setPreview(undefined); setConfirmed(false); }}>Back</button><button className="primary" disabled={!confirmed || busy || !requestsReady || Boolean(waiting) || projectBudgetAvailability(preview.project_call_limit).blocked} onClick={() => void start()}>Interpret saved feedback</button></div>
    </section>}
    {saved && <div className="conversation-feedback-result">
      <p role="status">{running ? "Interpreting the saved feedback. Leaving this page does not stop an admitted call." : checking ? "Feedback execution submitted. Checking the saved admission and outcome; no automatic retry." : phase === "cancelled" ? "Cancellation saved. No automatic retry will run; in-flight usage may still be billed." : phase === "expired" ? "Authorization expired before execution. Nothing was automatically renewed." : phase === "authorized" ? "Authorization saved; no model call is recorded. Execution requires your explicit action." : phase === "unknown" ? "Provider outcome unknown. A call may have been billed. This request will not be sent again." : phase === "failed" ? "The feedback request did not produce a result. No correction was applied." : phase === "invalid" ? "The model returned no valid feedback proposal. No correction was applied." : phase === "correction" ? "Correction proposed" : saved.scope_answer ? "Feedback scope recorded" : "Clarify the intended scope"}</p>
      {decision && !cancelled && <><p>{decision.question}</p><p>{decision.rationale}</p><small>Text-only interpretation of your saved message and candidate metadata, not a visual accuracy assessment.</small></>}
      <ConversationFeedbackScope key={saved.authorization.consent.call_id} project={project} value={saved} cancelled={cancelled} busy={busy} onSave={saveScope} onDirtyChange={scopeDirty} />
      {saved.scope_answer?.input.choice.scope === "project_future_rule" && <ConversationFutureSchemaCard key={`${saved.authorization.consent.call_id}:${saved.scope_answer.input.command_id}`} project={project} conversation={conversation} task={task} call={saved.authorization.consent.call_id} sourceAnswer={saved.scope_answer.input.command_id} cancelled={cancelled} onDirtyChange={futureDirty} onSample={onSample} onAssistance={onAssistance} onSetup={onSetup} />}
      {saved.scope_answer?.input.choice.scope === "current_image_class" && captureClassNavigation && <ConversationImageClassCard key={`${saved.authorization.consent.call_id}:${saved.scope_answer.input.command_id}`} project={project} value={saved} cancelled={cancelled} blocked={!requestsReady || Boolean(waiting)} captureOpen={captureClassNavigation} onDirtyChange={classDirty} onSample={onSample} onAssistance={onAssistance} />}
      {canCorrect && !waiting && <button className="primary" disabled={busy} onClick={() => void correct()}>Correct in canvas</button>}
      {phase === "clarify" && !waiting && (!saved.scope_answer || saved.scope_answer.input.choice.scope === "current_candidate") && <><button disabled={busy || !requestsReady} onClick={() => void stop()}>{saved.scope_answer ? "Cancel feedback action" : "Cancel scope question"}</button><small>Cancels this feedback action only. Saved answers, annotations and any existing correction requests remain unchanged.</small></>}
      {(running || checking || recoverable) && <button onClick={() => void stop()}>{running || checking ? "Stop feedback request" : "Cancel saved feedback request"}</button>}
      {recoverable && <><p>{saved.authorization.summary.model_name} · {saved.authorization.summary.remote_model} · {saved.authorization.summary.destination}</p><p>{saved.authorization.summary.data_scope}</p><small>One text request · Cost unknown · Expires {new Date(saved.authorization.consent.expires_at).toLocaleString()}. The frozen message and candidate have not been replaced.</small><button disabled={busy || !requestsReady || Boolean(waiting)} onClick={() => void start()}>Continue saved feedback request</button></>}
      {invalid && <p role="alert">{invalid}</p>}{(saved.error || saved.receipt?.evidence?.error) && <p role="alert">{saved.error || saved.receipt?.evidence?.error}</p>}
    </div>}
    {!saved && cancelled && <div className="conversation-feedback-result"><p role="status">Cancellation saved for this feedback request.</p><p>No automatic retry will run. Authorization and call records remain separate; no inference receipt is being assumed.</p><small>Cancellation recorded at {new Date(cancellation!.requested_at).toLocaleString()}. The original request identity is retained for recovery.</small></div>}
    {frozen.current && !saved && !cancelled && <div className="conversation-feedback-result"><p role="status">The save acknowledgement is unknown. The original message, model, call identity and expiry are retained. No automatic model retry is running.</p><button disabled={!ready || !requestsReady || busy || Boolean(waiting)} onClick={() => void start()}>Retry same feedback request</button><button onClick={() => void stop()}>Stop feedback request</button></div>}
    {error && <p role="alert">{error} Saved data remains on the server. Reload checks status without starting inference.</p>}
    {(saved || frozen.current || error || !ready) && <button disabled={busy} onClick={() => void reload()}>Reload saved feedback state</button>}
  </section>;
}
