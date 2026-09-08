import { useEffect, useRef, useState } from "react";
import { imageClassApi, type ImageClassAction, type ImageClassReview } from "../conversation-image-class-api";
import { imageClassOverlay, initialImageClassLocal, makeImageClassAnswer, mergeImageClassReview, parseImageClassLocal, sameImageClassInput, imageClassAnswerConflicts, imageClassLocalNeedsGuard, imageClassEditValue, imageClassDisplayOrigin, type ImageClassLocal } from "../conversation-image-class";
import { sampleAnnotations } from "../sampleAnnotations";
import { sampleFeedbackOverlay } from "../sampleFeedbackOverlay";
import type { Annotation, ImageItem, SampleTestOutcomeRecord } from "../types";
import { AnnotationCanvas } from "./AnnotationCanvas";
import { t } from "../i18n";

/** One Sandbox batch: local edits first, then one explicit atomic submission. */
export function ConversationImageClassReview({ project, review, image, outcomes, onChanged, onDirtyChange, onReturn, onRevision }: {
  project: string; review: ImageClassReview; image: ImageItem; outcomes: SampleTestOutcomeRecord[];
  onChanged: (review: ImageClassReview) => void; onDirtyChange: (dirty: boolean) => void; onReturn: () => void; onRevision: (draft: string) => void;
}) {
  const storageKey = `annotagent.image-class-edit:${project}:${review.conversation_id}:${review.task_id}:${review.id}`;
  const [local, setLocal] = useState<ImageClassLocal>(() => initialImageClassLocal(review));
  const [ready, setReady] = useState(false), [busy, setBusy] = useState(false), [durable, setDurable] = useState(true), [error, setError] = useState("");
  const [original, setOriginal] = useState(false), [history, setHistory] = useState<ImageClassAction[][]>([]);
  const [showSaved, setShowSaved] = useState(false);
  const alive = useRef(true), pending = useRef(false), version = useRef(0), current = useRef(local);
  current.current = local;
  const callback = useRef(onDirtyChange); callback.current = onDirtyChange;
  const conflict = imageClassAnswerConflicts(local, review);
  const origin = imageClassDisplayOrigin(local, review, showSaved);
  const shownActions = origin === "saved" && review.answer ? review.answer.actions : local.actions;
  const dirty = imageClassLocalNeedsGuard(local, review);
  const readOnly = !ready || busy || review.status !== "pending" || Boolean(local.frozen) || conflict || original;
  function retain(next: ImageClassLocal) {
    current.current = next; setLocal(next);
    try { sessionStorage.setItem(storageKey, JSON.stringify(next)); setDurable(true); }
    catch { setDurable(false); }
  }
  function accept(record: ImageClassReview) {
    if (record.id !== review.id || record.conversation_id !== review.conversation_id || record.task_id !== review.task_id || record.scope_digest !== review.scope_digest) throw new Error("The saved review belongs to a different image or scope.");
    onChanged(mergeImageClassReview(review, record));
    if (record.answer && (current.current.frozen ? sameImageClassInput(record.answer, current.current.frozen) : current.current.actions.every(item=>item.action==="keep") || sameImageClassInput(record.answer.actions,current.current.actions))) {
      retain({ ...initialImageClassLocal(record), selected_id: current.current.selected_id });
      setHistory([]); callback.current(false);
    }
  }
  useEffect(() => {
    alive.current = true;
    try { const raw = sessionStorage.getItem(storageKey); const restored = parseImageClassLocal(raw, review); if (restored) { if (review.answer && restored.frozen && sameImageClassInput(review.answer, restored.frozen)) retain({ ...initialImageClassLocal(review), selected_id: restored.selected_id }); else retain(restored); } else if (raw) setError("Saved browser edits do not match this frozen review. They were not applied to a different scope."); }
    catch { setDurable(false); }
    setReady(true);
    return () => { alive.current = false; version.current++; callback.current(false); };
  }, [storageKey]);
  useEffect(() => { callback.current(dirty); return () => callback.current(false); }, [dirty]);
  useEffect(() => {
    if (!dirty) return;
    const guard = (event: BeforeUnloadEvent) => event.preventDefault(); window.addEventListener("beforeunload", guard);
    return () => window.removeEventListener("beforeunload", guard);
  }, [dirty]);
  const base = sampleFeedbackOverlay(sampleAnnotations(outcomes, image.image_id, review.scope.sample_test_id), review.scope.baseline_feedback);
  let overlay = base, invalid = "";
  try { if (origin === "saved") overlay = sampleFeedbackOverlay(sampleAnnotations(outcomes, image.image_id, review.scope.sample_test_id), [...review.scope.baseline_feedback, ...review.revisions]); else overlay = imageClassOverlay(review.scope, local.actions, outcomes); makeImageClassAnswer(review, local, "validation-only"); }
  catch (reason) { invalid = (reason as Error).message; }
  const action = local.actions.find(item => item.outcome_id === local.selected_id);
  const selected = overlay.annotations.find(item => item.id === local.selected_id);
  const baseline = base.annotations.find(item => item.id === local.selected_id);
  const editingValue = imageClassEditValue(baseline, action);
  const member = review.scope.members.find(item => item.outcome.id === local.selected_id);
  const excluded = action?.action === "exclude";
  function change(next: ImageClassAction) {
    if (readOnly || !member || next.outcome_id !== member.outcome.id || next.source_artifact_id !== member.source_artifact_id) return;
    setHistory(items => [...items, local.actions]); setError("");
    retain({ ...local, actions: local.actions.map(item => item.outcome_id === next.outcome_id ? next : item) });
  }
  function edit(annotation: Annotation) {
    if (!selected || !member || annotation.id !== local.selected_id || annotation.value.kind !== "bounding_box") return;
    change({ action: "edit", outcome_id: member.outcome.id, source_artifact_id: member.source_artifact_id, corrected_label: annotation.label ?? "", corrected_value: annotation.value });
  }
  async function submit() {
    if (pending.current || review.status !== "pending" || !ready || conflict) return;
    let input; try { input = makeImageClassAnswer(review, local, crypto.randomUUID()); } catch (reason) { setError((reason as Error).message); return; }
    retain({ ...local, frozen: input }); pending.current = true; setBusy(true); setError(""); const ticket = ++version.current;
    try { const saved = await imageClassApi.answer(project, review.conversation_id, review.task_id, review.id, input); if (alive.current && ticket === version.current) { if (!saved.answer || !sameImageClassInput(saved.answer, input)) throw new Error("The saved answer differs from this batch. Your edits have not been substituted."); accept(saved); } }
    catch (reason) {
      if (!alive.current || ticket !== version.current) return;
      try { const saved = await imageClassApi.get(project, review.conversation_id, review.task_id, review.id); if (!alive.current || ticket !== version.current) return; accept(saved); if (saved.answer && sameImageClassInput(saved.answer, input)) return; }
      catch { /* Keep the exact batch command until its result is known. */ }
      if (alive.current && ticket === version.current) setError((reason as Error).message);
    } finally { pending.current = false; if (alive.current && ticket === version.current) setBusy(false); }
  }
  async function command(kind: "cancel" | "resume" | "get") {
    if (pending.current) return;
    pending.current = true; setBusy(true); setError(""); const ticket = ++version.current;
    try { const saved = await imageClassApi[kind](project, review.conversation_id, review.task_id, review.id); if (alive.current && ticket === version.current) accept(saved); }
    catch (reason) { if (alive.current && ticket === version.current) setError((reason as Error).message); }
    finally { pending.current = false; if (alive.current && ticket === version.current) setBusy(false); }
  }
  return <section className="conversation-image-class-review" aria-label={t("Image-class review")}>
    <header><h2>{image.name}</h2><p>{review.scope.target_label} · {review.scope.members.length} {t("candidates")} · {t("This sample image only")}</p><p>{t("Edit or exclude selected candidates, then save all decisions together. Other classes and images are read-only and unchanged.")}</p></header>
    <div className="button-row"><button onClick={onReturn}>{t("Return to sample results")}</button><button aria-pressed={original} onClick={() => setOriginal(!original)}>{t(original ? "Current class decisions" : "Original predictions")}</button><button disabled={busy} onClick={() => void command("get")}>{t("Reload saved review")}</button>{review.status === "pending" && <button disabled={busy} onClick={() => void command("cancel")}>{t("Cancel image-class review")}</button>}</div>
    <p role="status">{t(review.status === "cancelled" ? "Image-class review cancelled" : review.answer ? "Image-class decisions saved" : local.frozen ? "The batch acknowledgement is unknown. Retry sends the same decisions, not a new batch." : "Changes stay local until you save all class decisions.")}</p>
    {conflict && <section aria-label={t("Compare conflicting class decisions")}><div className="button-row"><button aria-pressed={!original && origin === "local"} onClick={() => { setOriginal(false); setShowSaved(false); }}>{t("My unsaved class decisions")}</button><button aria-pressed={!original && origin === "saved"} onClick={() => { setOriginal(false); setShowSaved(true); }}>{t("Saved class decisions")}</button></div><p>{t(origin === "local" ? "This canvas shows your unsaved decisions. They have not replaced the saved batch." : "This canvas shows the server-saved decisions. Your unsaved decisions are retained separately.")}</p></section>}
    {review.status === "cancelled" && <p>{t("Unsaved edits remain visible for reference only. Cancellation did not apply them.")}</p>}
    <label>{t("Candidate in this class")}<select aria-label={t("Candidate in this class")} value={local.selected_id} disabled={!ready || busy} onChange={event => retain({ ...local, selected_id: event.target.value })}>{review.scope.members.map((item, index) => <option key={item.outcome.id} value={item.outcome.id}>{index + 1}. {review.scope.target_label} · {item.outcome.id}</option>)}</select></label>
    <AnnotationCanvas compactList imageUrl={image.url} annotations={original ? sampleAnnotations(outcomes, image.image_id, review.scope.sample_test_id) : overlay.annotations} selectedId={local.selected_id} readOnly={readOnly || excluded} onSelect={id => { if (review.scope.members.some(item => item.outcome.id === id)) retain({ ...local, selected_id: id }); }} onChange={edit} />
    {member && !original && (!conflict || origin === "local") && <div className="image-class-edit-controls"><div className="button-row"><button disabled={readOnly} aria-pressed={action?.action === "keep"} onClick={() => change({ action: "keep", outcome_id: member.outcome.id, source_artifact_id: member.source_artifact_id })}>{t("Keep unchanged")}</button><button disabled={readOnly} aria-pressed={excluded} onClick={() => change({ action: "exclude", outcome_id: member.outcome.id, source_artifact_id: member.source_artifact_id })}>{t("Exclude selected target")}</button><button disabled={readOnly || !history.length} onClick={() => { retain({ ...local, actions: history.at(-1)! }); setHistory(items => items.slice(0, -1)); }}>{t("Undo edit")}</button></div>
      {review.scope.kind === "classification" && <><p>{t("Only the selected class token is replaced or removed; the other saved classes remain unchanged.")}</p><label>{t("Replacement class")}<input aria-label={t("Replacement class")} value={action?.action === "replace_label" ? action.replacement : review.scope.target_label} disabled={readOnly || excluded} onChange={event => change({ action: "replace_label", outcome_id: member.outcome.id, source_artifact_id: member.source_artifact_id, replacement: event.target.value })} /></label><p>{t("Other saved classes")}: {baseline?.value.kind === "classification" ? baseline.value.labels.filter(label => label !== review.scope.target_label).join(", ") || "—" : "—"}</p></>}
      {review.scope.kind === "bounding_box" && (selected ?? baseline)?.value.kind === "bounding_box" && <><label>{t("Correct label")}<input aria-label={t("Correct label")} value={action?.action === "edit" ? action.corrected_label : selected?.label ?? baseline?.label ?? ""} disabled={readOnly || excluded} onChange={event => { const item = editingValue; if (item) edit({ ...item, label: event.target.value }); }} /></label><div className="sample-feedback-coordinates">{((action?.action === "edit" && action.corrected_value.kind === "bounding_box" ? action.corrected_value.rect : selected?.value.kind === "bounding_box" ? selected.value.rect : baseline?.value.kind === "bounding_box" ? baseline.value.rect : [])).map((value, index) => <label key={index}>{["x", "y", "width", "height"][index]}<input aria-label={["x", "y", "width", "height"][index]} type="number" step="0.001" min="0" max="1" value={value} disabled={readOnly || excluded} onChange={event => { const item = editingValue; if (item?.value.kind !== "bounding_box") return; const rect = [...item.value.rect] as [number, number, number, number]; rect[index] = Number(event.target.value); edit({ ...item, value: { kind: "bounding_box", rect } }); }} /></label>)}</div></>}
    </div>}
    {overlay.excluded.length > 0 && <details className="sample-excluded-candidates"><summary>{t("Excluded sample candidates")} · {overlay.excluded.length}</summary><p>{t("Original predictions remain available above. Exclusion affects this Sandbox result view only.")}</p><ul>{overlay.excluded.map(item => <li key={item.annotation.id}>{item.annotation.label} · {item.annotation.id}</li>)}</ul></details>}
    <footer><p>{t("Keep")}: {shownActions.filter(item => item.action === "keep").length} · {t("Edit")}: {shownActions.filter(item => item.action === "edit" || item.action === "replace_label").length} · {t("Exclude")}: {shownActions.filter(item => item.action === "exclude").length}</p>
      {review.status === "pending" && <button className="primary" disabled={!ready || busy || !!invalid || conflict} onClick={() => void submit()}>{t(local.frozen ? "Retry same class decisions" : "Save all class decisions")}</button>}
      {review.status === "answered" && !review.repair_draft_id && <button disabled={busy} onClick={() => void command("resume")}>{t("Prepare revision Draft")}</button>}
      {review.repair_draft_id && <button onClick={() => onRevision(review.repair_draft_id!)}>{t("Inspect revision Draft")}</button>}
      {review.answer && <p>{t("One revision Draft is separate from the tested plan. Building or testing it requires fresh authorization; no model ran for this save.")}</p>}
      {!durable && dirty && <p role="alert">{t("Browser storage is unavailable. Keep this page open; leaving requires confirmation.")}</p>}
      {conflict && <p role="alert">{t("Another batch answer is saved. Your original edits are retained but cannot overwrite it.")}</p>}
      {(error || invalid || review.resume_error) && <p role="alert">{error || invalid || review.resume_error}</p>}
    </footer>
  </section>;
}
