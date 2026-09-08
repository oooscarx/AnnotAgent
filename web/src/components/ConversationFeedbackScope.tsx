import { useEffect, useRef, useState } from "react";
import type { FeedbackCorrectionReason, FeedbackStatus, ScopeAnswerInput, ScopeAnswerRecord } from "../conversation-feedback-api";
import { candidateFeedbackKind, feedbackReasonLabel, feedbackScopeLabel, feedbackScopeVisible, makeFeedbackScopeChoice, parseLocalScopeAnswer, sameScopeAnswerInput, scopeAnswerConflicts, type FeedbackScope, type LocalScopeAnswer } from "../conversation-feedback-scope";

/** Human intent only. This form never calls a model, edits annotations or broadens permission. */
export function ConversationFeedbackScope({ project, value, cancelled, busy, onSave, onDirtyChange }: {
  project: string; value: FeedbackStatus; cancelled: boolean; busy: boolean;
  onSave: (input: ScopeAnswerInput) => Promise<ScopeAnswerRecord>;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const call = value.authorization.consent.call_id;
  const storageKey = `annotagent.feedback-scope:${project}:${value.authorization.context.message.conversation_id}:${value.authorization.grant.task_id}:${call}`;
  const initial = (): LocalScopeAnswer => ({ call_id: call, context_digest: value.scope_context_digest });
  const [local, setLocal] = useState<LocalScopeAnswer>(initial);
  const [ready, setReady] = useState(false), [durable, setDurable] = useState(true), [error, setError] = useState("");
  const alive = useRef(true), pending = useRef(false);
  const kind = candidateFeedbackKind(value.authorization.context.candidate);
  const answer = value.scope_answer;
  const conflict = Boolean(answer && scopeAnswerConflicts(local, answer));
  const stale = local.context_digest !== value.scope_context_digest;
  const choice = makeFeedbackScopeChoice(local.scope, local.reason, kind);

  function retain(next: LocalScopeAnswer) {
    setLocal(next);
    try { sessionStorage.setItem(storageKey, JSON.stringify(next)); setDurable(true); onDirtyChange?.(false); }
    catch { setDurable(false); onDirtyChange?.(Boolean(next.scope || next.frozen)); }
  }
  function clearLocal() {
    try { sessionStorage.removeItem(storageKey); } catch { /* The immutable server answer is already saved. */ }
    setLocal(initial()); onDirtyChange?.(false);
  }
  useEffect(() => {
    alive.current = true;
    try { const previous = parseLocalScopeAnswer(sessionStorage.getItem(storageKey), call); if (previous) setLocal(previous); }
    catch { setDurable(false); }
    setReady(true);
    return () => { alive.current = false; onDirtyChange?.(false); };
  }, [storageKey]);
  useEffect(() => {
    if (ready && answer && !conflict && (local.scope || local.frozen)) clearLocal();
  }, [ready, answer, conflict, local.scope, local.frozen]);

  async function submit() {
    if (!ready || pending.current || busy || cancelled || answer || stale || !choice) return;
    const command = local.frozen ?? { command_id: crypto.randomUUID(), expected_context_digest: value.scope_context_digest, choice: choice! };
    pending.current = true; setError(""); retain({ ...local, frozen: command });
    try {
      const saved = await onSave(command);
      if (!alive.current) return;
      if (saved.call_id !== call || !sameScopeAnswerInput(command, saved.input)) throw new Error("A different scope answer was saved. Your original choices have not been substituted.");
      clearLocal();
    } catch (reason) { if (alive.current) setError((reason as Error).message); }
    finally { pending.current = false; }
  }

  const scopes: FeedbackScope[] = ["current_candidate", "current_image_class", "project_future_rule"];
  const reasons: FeedbackCorrectionReason[] = kind === "bounding_box" ? ["poor_boundary", "wrong_label", "wrong_target"] : ["wrong_label", "wrong_target"];
  const readonly = busy || cancelled || Boolean(answer) || Boolean(local.frozen) || stale;
  const isClarification = Boolean(value.decision && "Ok" in value.decision && value.decision.Ok.decision === "clarify_scope");
  if (!feedbackScopeVisible({ isClarification, answer, cancelled, local })) return null;
  return <section className="conversation-feedback-scope" aria-label="Clarify feedback scope">
    <p>{cancelled ? "Scope choices are shown for reference only; this feedback request is cancelled." : "Choose what your message refers to. Saving records your intention only; it does not change an annotation, remove a class, call a model or run a plan."}</p>
    {!ready && <p role="status">Restoring your scope choices…</p>}
    {answer && <div className="conversation-feedback-scope-result">
      <p role="status">Scope answer saved</p><p>{feedbackScopeLabel(answer.input.choice.scope)}</p>
      {answer.input.choice.scope === "current_candidate" ? <><p>{feedbackReasonLabel(answer.input.choice.reason)}</p><p>No annotation was changed by this scope answer.{!cancelled && " Open the canvas to supply the actual correction."}</p></> : <><p>This intention is recorded, not applied.</p><p>A separate rule proposal is required; no labels or annotations have been changed.</p></>}
      {cancelled && <p>This feedback request is cancelled. Its saved scope answer is read-only.</p>}
    </div>}
    {ready && (!answer || conflict) && <form onSubmit={event => { event.preventDefault(); void submit(); }}>
      <fieldset disabled={readonly}><legend>What did you mean?</legend>{scopes.map(scope => <label key={scope}><input type="radio" name={`scope-${call}`} checked={local.scope === scope} onChange={() => { setError(""); retain({ ...local, scope, reason: undefined }); }} /><span>{feedbackScopeLabel(scope)}</span></label>)}</fieldset>
      {local.scope === "current_candidate" && <fieldset disabled={readonly || !kind}><legend>What needs correcting?</legend>{reasons.map(reason => <label key={reason}><input type="radio" name={`scope-reason-${call}`} checked={local.reason === reason} onChange={() => { setError(""); retain({ ...local, reason }); }} /><span>{feedbackReasonLabel(reason)}</span></label>)}</fieldset>}
      {local.scope === "current_candidate" && !kind && <p role="status">This saved candidate type does not support the current correction form. No substitute geometry or class is assumed.</p>}
      {stale && <p role="alert">The saved context has changed. Your original browser choices are shown but cannot be submitted against a different context.</p>}
      {conflict && <p role="alert">A different scope answer is already saved. Your browser choices remain shown below that answer and were not applied. They cannot overwrite the saved answer.</p>}
      {cancelled && <p role="status">This request is cancelled. Unsaved choices are retained for reference but cannot be submitted.</p>}
      {!answer && !cancelled && !stale && <button className="primary" type="submit" disabled={busy || !choice}>{local.frozen ? "Retry same scope answer" : "Save scope answer"}</button>}
      {!answer && local.frozen && !cancelled && <p role="status">The answer acknowledgement is not confirmed. The original command and choices are retained; retry sends exactly the same answer, not a new operation.</p>}
      {!answer && !local.frozen && local.scope && <small>{durable ? "Not submitted · Choices are kept in this browser tab." : "Not submitted · Browser storage is unavailable; these choices are only on this page. Leaving requires confirmation."}</small>}
    </form>}
    {error && <p role="alert">{error} Your original choices remain available. Reloading saved feedback checks the server without resubmitting.</p>}
  </section>;
}
