import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { FeedbackStatus } from "../conversation-feedback-api";
import { imageClassApi, type ImageClassCreateInput, type ImageClassPreview, type ImageClassReview } from "../conversation-image-class-api";
import { mergeImageClassReview, sameImageClassInput } from "../conversation-image-class";
import { sampleAnnotations } from "../sampleAnnotations";
import { sampleFeedbackOverlay } from "../sampleFeedbackOverlay";
import type { SampleTestOutcomeRecord } from "../types";
import { t } from "../i18n";

export function ConversationImageClassCard({ project, value, cancelled, blocked, captureOpen, onDirtyChange }: {
  project: string; value: FeedbackStatus; cancelled: boolean; blocked: boolean;
  captureOpen: () => (review: ImageClassReview) => void; onDirtyChange?: (dirty: boolean) => void;
}) {
  const call = value.authorization.consent.call_id, conversation = value.authorization.context.message.conversation_id, task = value.authorization.grant.task_id;
  const storageKey = `annotagent.image-class-create:${project}:${conversation}:${task}:${call}`;
  const [saved, setSaved] = useState<ImageClassReview>(), [preview, setPreview] = useState<ImageClassPreview>();
  const [tokens, setTokens] = useState<string[]>([]), [token, setToken] = useState(""), [classification, setClassification] = useState(false);
  const [ready, setReady] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState("");
  const [frozen, setFrozen] = useState<ImageClassCreateInput>();
  const alive = useRef(true), pending = useRef(false), version = useRef(0);
  const dirty = useRef(onDirtyChange); dirty.current = onDirtyChange;
  function retain(input: ImageClassCreateInput) { setFrozen(input); try { sessionStorage.setItem(storageKey, JSON.stringify(input)); dirty.current?.(false); } catch { dirty.current?.(true); } }
  function apply(review: ImageClassReview) {
    if (review.conversation_id !== conversation || review.task_id !== task || review.input.feedback_call_id !== call || review.scope.feedback_call_id !== call || review.scope.scope_answer.input.command_id !== value.scope_answer?.input.command_id) throw new Error("The image-class review belongs to another saved scope. No replacement was opened.");
    setSaved(previous => mergeImageClassReview(previous, review));
    try { sessionStorage.removeItem(storageKey); } catch { /* Saved review remains authoritative. */ }
    setFrozen(undefined); dirty.current?.(false);
  }
  useEffect(() => {
    alive.current = true; const controller = new AbortController(), ticket = ++version.current;
    try { const raw = sessionStorage.getItem(storageKey); if (raw) { const input = JSON.parse(raw) as ImageClassCreateInput; if (input.feedback_call_id === call && typeof input.id === "string" && typeof input.expected_scope_digest === "string" && (input.target_label == null || typeof input.target_label === "string")) { setFrozen(input); setToken(input.target_label ?? ""); } } } catch { /* No request is reconstructed from incomplete storage. */ }
    void (async () => {
      const record = await imageClassApi.forFeedback(project, conversation, task, call, controller.signal);
      if (controller.signal.aborted || ticket !== version.current) return;
      if (record) { apply(record); setReady(true); return; }
      const candidate = value.authorization.context.candidate as { outcome?: SampleTestOutcomeRecord } | null;
      const source = value.authorization.context.message.input;
      if (!candidate?.outcome?.value || !source.image || source.reference?.scope !== "sample_candidate") throw new Error("The original candidate metadata is not available. No class was guessed.");
      const feedback = await api.sampleFeedback(source.reference.sample_test_id, source.image.image_id);
      if (controller.signal.aborted || ticket !== version.current) return;
      const effective = sampleFeedbackOverlay(sampleAnnotations([candidate.outcome], source.image.image_id, source.reference.sample_test_id), feedback.revisions).annotations.find(item => item.id === candidate.outcome!.id);
      const isClassification = effective?.value.kind === "classification";
      setClassification(isClassification);
      const choices = effective?.value.kind === "classification" ? effective.value.labels : effective?.value.kind === "bounding_box" ? [effective.label] : [];
      setTokens([...new Set(choices.filter((item): item is string => typeof item === "string" && item.length > 0))]); if (!isClassification) setToken(choices[0] ?? "");
      setReady(true);
    })().catch((reason: Error) => { if (!controller.signal.aborted && ticket === version.current) { setError(reason.message); setReady(true); } });
    return () => { alive.current = false; version.current++; controller.abort(); dirty.current?.(false); };
  }, [project, conversation, task, call]);
  async function loadPreview() {
    if (pending.current || cancelled || blocked || !token) return;
    pending.current = true; setBusy(true); setError(""); const ticket = ++version.current;
    try { const result = await imageClassApi.preview(project, conversation, task, call, token); if (alive.current && ticket === version.current) { if (result.scope.feedback_call_id !== call || result.scope.target_label !== token) throw new Error("The returned scope differs from the selected class."); setPreview(result); } }
    catch (reason) { if (alive.current && ticket === version.current) setError((reason as Error).message); }
    finally { pending.current = false; if (alive.current && ticket === version.current) setBusy(false); }
  }
  async function create() {
    if (pending.current || cancelled || blocked || (!frozen && !preview)) return;
    const input = frozen ?? { id: crypto.randomUUID(), feedback_call_id: call, target_label: preview!.scope.target_label, expected_scope_digest: preview!.scope_digest };
    const open = captureOpen(), ticket = ++version.current; retain(input); pending.current = true; setBusy(true); setError("");
    try { const result = await imageClassApi.create(project, conversation, task, call, input); if (!alive.current || ticket !== version.current) return; if (!sameImageClassInput(result.input, input)) throw new Error("A different review command is already saved. The original selection remains unchanged."); apply(result); open(result); }
    catch (reason) {
      if (!alive.current || ticket !== version.current) return;
      try { const result = await imageClassApi.forFeedback(project, conversation, task, call); if (!alive.current || ticket !== version.current) return; if (result && sameImageClassInput(result.input, input)) { apply(result); open(result); return; } }
      catch { /* Unknown acknowledgment retains exactly the original create command. */ }
      if (alive.current && ticket === version.current) setError((reason as Error).message);
    } finally { pending.current = false; if (alive.current && ticket === version.current) setBusy(false); }
  }
  return <section className="conversation-image-class-card" aria-label={t("Review this class in the current image")}>
    <h4>{t("Review this class in the current image")}</h4><p>{t("Review a frozen set of this class in one saved sample image. No model runs and no formal annotation changes.")}</p>
    {!ready && <p role="status">{t("Loading saved review scope…")}</p>}
    {saved ? <><p role="status">{t(saved.status === "cancelled" ? "Image-class review cancelled" : saved.answer ? "Image-class decisions saved" : "Image-class review ready")}</p><p>{saved.scope.target_label} · {saved.scope.members.length} {t("candidates")} · {saved.scope.image_id}</p><button onClick={() => captureOpen()(saved)}>{t("Open image-class review")}</button></> : <>
      {classification && <label>{t("Class to review")}<select aria-label={t("Class to review")} disabled={busy || cancelled || !!frozen} value={token} onChange={event => { setToken(event.target.value); setPreview(undefined); }}><option value="">{t("Choose one original class")}</option>{tokens.map(item => <option key={item} value={item}>{item}</option>)}</select></label>}
      {ready && !tokens.length && <p role="status">{t("No supported class remains in this saved candidate. No substitute target was selected.")}</p>}
      {frozen ? <><p role="status">{t("The review creation acknowledgement is unknown. Retry keeps the same class and command.")}</p><button disabled={busy || cancelled || blocked} onClick={() => void create()}>{t("Retry same image-class review")}</button></> : !preview ? <button disabled={!ready || busy || cancelled || blocked || !token} onClick={() => void loadPreview()}>{t("Review this class in the current image")}</button> : <section aria-label={t("Image-class review scope")}><p>{preview.scope.target_label} · {preview.scope.members.length} {t("candidates")} · {preview.scope.image_id}</p><p>{t("Only these candidates can change. Every other class and image remains unchanged.")}</p><button className="primary" disabled={busy || cancelled || blocked || !preview.scope.members.length} onClick={() => void create()}>{t("Open image-class review")}</button></section>}
    </>}
    {cancelled && <p>{t(saved ? "The source feedback is cancelled. This existing image-class review keeps its own state. To stop it, open the review and cancel it explicitly." : "The source feedback is cancelled. No new image-class review will be created.")}</p>}
    {blocked && !saved && <p>{t("Resolve the existing task request before opening another correction.")}</p>}
    {error && <p role="alert">{error}</p>}
  </section>;
}
