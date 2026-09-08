import { useCallback, useEffect, useRef, useState } from "react";
import type { ConversationMessage } from "../types";
import { compactStopTarget, isStopMessage, mergeStopRecord, parseStopSelection, stopSelectionConflicts, stopTargetMatches, type LocalStopSelection } from "../conversation-control";
import { stopApi, type StopRequestRecord, type StopTargetRef } from "../conversation-stop-api";
import { t, useLocale } from "../i18n";

type StopCardProps = { project: string; message: ConversationMessage; initial?: StopRequestRecord; taskNames: Record<string, string>; onChanged: () => void; onDirtyChange: (dirty: boolean) => void };
export function ConversationStopCard(props: StopCardProps) {
  if (!isStopMessage(props.message.input)) return null;
  return <StopCard key={`${props.project}:${props.message.conversation_id}:${props.message.input.id}`} {...props} />;
}
function StopCard({ project, message, initial, taskNames, onChanged, onDirtyChange }: StopCardProps) {
  useLocale();
  const conversation = message.conversation_id, id = message.input.id;
  const storageKey = `annotagent.stop-selection:${project}:${conversation}:${id}`;
  const [record, setRecord] = useState<StopRequestRecord | undefined>(initial);
  const [local, setLocal] = useState<LocalStopSelection>();
  const [ready, setReady] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState("");
  const alive = useRef(true), pending = useRef(false), generation = useRef(0), localRef = useRef(local), dirtyCallback = useRef(onDirtyChange);
  localRef.current = local; dirtyCallback.current = onDirtyChange;
  const conflict = stopSelectionConflicts(local, record);
  const sourceTask = message.input.reference?.task_id;
  function retain(value: LocalStopSelection) {
    localRef.current = value; setLocal(value);
    try { sessionStorage.setItem(storageKey, JSON.stringify(value)); dirtyCallback.current(false); }
    catch { dirtyCallback.current(true); setError(t("Browser storage is unavailable. Keep this page open or confirm leaving with the unsaved stop selection.")); }
  }
  const clearLocal = useCallback(() => {
    localRef.current = undefined; setLocal(undefined); dirtyCallback.current(false);
    try { sessionStorage.removeItem(storageKey); } catch { /* Server cancellation receipts remain authoritative. */ }
  }, [storageKey]);
  function apply(value: StopRequestRecord) {
    const input = value.message.input;
    if (value.message.conversation_id !== conversation || !isStopMessage(input) || input.id !== id || input.text !== message.input.text || input.reference.task_id !== sourceTask) throw new Error(t("The stop receipt belongs to a different message or task. No other operation was selected."));
    setRecord(previous => mergeStopRecord(previous, value));
    if (value.selected_target && localRef.current && stopTargetMatches(value.selected_target, localRef.current.target)) clearLocal();
  }
  useEffect(() => {
    alive.current = true; const controller = new AbortController(), ticket = ++generation.current;
    try { const saved = parseStopSelection(sessionStorage.getItem(storageKey), id); if (saved) { localRef.current = saved; setLocal(saved); } } catch { /* No automatic selection or cancellation. */ }
    void stopApi.read(project, conversation, id, controller.signal).then(value => { if (!controller.signal.aborted && ticket === generation.current) { if (value) apply(value); setReady(true); } }).catch((reason: Error) => { if (!controller.signal.aborted && ticket === generation.current) { setError(reason.message); setReady(true); } });
    return () => { alive.current = false; generation.current++; controller.abort(); dirtyCallback.current(false); };
  }, [storageKey]);
  const active = record?.status === "cancel_requested" && ["running", "cancel_pending"].includes(record.observation?.state ?? "");
  useEffect(() => {
    if (!active) return;
    const controller = new AbortController(); let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      const ticket = generation.current;
      try { const value = await stopApi.read(project, conversation, id, controller.signal); if (controller.signal.aborted) return; if (value && ticket === generation.current) apply(value); }
      catch (reason) { if (!controller.signal.aborted && ticket === generation.current) setError((reason as Error).message); }
      if (!controller.signal.aborted) timer = setTimeout(() => void poll(), 1800);
    };
    timer = setTimeout(() => void poll(), 1800);
    return () => { controller.abort(); clearTimeout(timer); };
  }, [active, project, conversation, id]);
  async function reload() {
    if (pending.current) return; pending.current = true; setBusy(true); const ticket = ++generation.current;
    try { const value = await stopApi.read(project, conversation, id); if (alive.current && ticket === generation.current) { if (value) apply(value); setError(""); setReady(true); onChanged(); } }
    catch (reason) { if (alive.current && ticket === generation.current) setError((reason as Error).message); }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  async function submit(selection?: StopTargetRef) {
    if (pending.current || !ready || conflict || !isStopMessage(message.input)) return;
    if (selection && (!record || record.status !== "needs_selection" || !record.targets.some(target => stopTargetMatches(target, selection)))) return;
    pending.current = true; setBusy(true); setError(""); const ticket = ++generation.current;
    if (selection) retain({ message_id: id, target: selection, pending: true });
    try {
      const value = selection ? await stopApi.select(project, conversation, id, selection) : await stopApi.begin(project, conversation, message.input);
      if (alive.current && ticket === generation.current) { apply(value); onChanged(); }
    } catch (reason) {
      if (!alive.current || ticket !== generation.current) return;
      setError((reason as Error).message);
      try { const value = await stopApi.read(project, conversation, id); if (alive.current && ticket === generation.current && value) { apply(value); onChanged(); } }
      catch { /* Retain the original target and command; GET never re-dispatches. */ }
    } finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  function targetKind(target: StopTargetRef) { return t(target.kind === "call" ? "Model call" : target.kind === "builder" ? "Pipeline Builder" : target.kind === "journey" ? "Build and sample task" : target.kind === "sample" ? "Sample test" : target.kind === "processing" ? "Dataset processing" : "Saved authorization"); }
  function targetLabel(target: StopTargetRef) { return `${taskNames[target.task_id] ? `${taskNames[target.task_id]} · ` : ""}${targetKind(target)} · ${target.id} · ${t("Task")} ${target.task_id}`; }
  function targetSummary(target: StopTargetRef, state?: string) {
    const compact = compactStopTarget(target, taskNames[target.task_id]);
    return <span className="conversation-stop-target-summary"><strong title={taskNames[target.task_id]}>{compact.title || `${t("Task")} ${compact.taskId}`}</strong><small>{targetKind(target)} · {compact.operationId}{state ? ` · ${t(state)}` : ""}</small></span>;
  }
  function targetDetails(target: StopTargetRef) { return <details className="conversation-stop-target-details"><summary>{t("Operation details")}</summary><dl><dt>{t("Task")}</dt><dd>{taskNames[target.task_id] || `${t("Task")} ${target.task_id}`}</dd><dt>{t("Operation ID")}</dt><dd>{target.id}</dd><dt>{t("Task ID")}</dt><dd>{target.task_id}</dd></dl></details>; }
  return <section className="conversation-stop-card conversation-feedback-card" aria-label={t("Stop request")}>
    <h3>{t("Stop request")}</h3><p>{sourceTask ? `${t("Scope frozen when sent")}: ${taskNames[sourceTask] ?? sourceTask}` : t("Scope frozen when sent: active work in this conversation only.")}</p>
    <small>{t("This control does not call an LLM, change annotations or stop another project. Requests already sent to models may still be billed.")}</small>
    {!ready && <p role="status">{t("Restoring saved stop request…")}</p>}
    {record && <>
      <p role="status">{t(record.status === "no_active_work" ? "No active work to stop." : record.status === "needs_selection" ? "Choose the operation to stop. Nothing has been cancelled by this request yet." : record.status === "finished" ? "The selected operation is no longer active. No other operation was stopped." : "Cancellation request saved.")}</p>
      {record.observation && <p role="status">{t(record.observation.description)}</p>}
      {record.selected_target && <div className="conversation-stop-target">{targetSummary(record.selected_target)}{targetDetails(record.selected_target)}</div>}
      {record.status === "needs_selection" && <form onSubmit={event => { event.preventDefault(); if (local) void submit(local.target); }}>
        <fieldset disabled={busy || Boolean(local?.pending)}><legend>{t("Which operation should stop?")}</legend>{record.targets.map(target => <div className="conversation-stop-target" key={`${target.kind}:${target.task_id}:${target.id}`}><label><input type="radio" name={`stop-target-${id}`} aria-label={targetLabel(target)} checked={Boolean(local && stopTargetMatches(local.target, target))} onChange={() => retain({ message_id: id, target: { kind: target.kind, id: target.id, task_id: target.task_id }, pending: false })} />{targetSummary(target, target.state)}</label>{targetDetails(target)}</div>)}</fieldset>
        {local?.pending && <p role="status">{t("Selection acknowledgement is not confirmed. Retry keeps the same operation, not newly discovered work.")}</p>}
        <button className="danger-button" type="submit" disabled={!ready || busy || !local || conflict}>{t(local?.pending ? "Retry same stop selection" : "Stop selected operation")}</button>
      </form>}
      {conflict && <><p role="alert">{t("A different stop target is already saved. Your browser choice was not substituted or applied.")}</p>{local && <div className="conversation-stop-target"><p>{t("Your browser selection")}</p>{targetSummary(local.target)}{targetDetails(local.target)}</div>}</>}
      {record.dispatch_error && <><p role="alert">{record.dispatch_error}</p><button disabled={busy || !ready || conflict} onClick={() => void submit()}>{t("Retry same stop request")}</button></>}
    </>}
    {ready && !record && <><p role="status">{t("The stop request is not confirmed saved. Retry preserves this original message and task scope.")}</p><button disabled={busy} onClick={() => void submit()}>{t("Retry same stop request")}</button></>}
    {busy && <p role="status">{t("Saving stop request state…")}</p>}
    {error && <p role="alert">{error}</p>}
    <button disabled={busy} onClick={() => void reload()}>{t("Reload stop request status")}</button>
  </section>;
}
