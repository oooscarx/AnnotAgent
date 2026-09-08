import { useEffect, useRef, useState } from "react";
import { ConversationBatchStatus } from "./ConversationBatchStatus";
import { api, type ConfirmProcessingRequest, type ProcessingAuthorization, type ProcessingReceipt } from "../api";
import { t } from "../i18n";
import { projectBatchPath, projectJourneyPath } from "../navigation";

export function JourneyConfirm({ projectId, draftId, testId, imageId, operationId, onNavigate, confirmationPath, backPath, stayOnReceipt=false, expectedConversation }: {
  projectId: string; draftId?: string; testId?: string; imageId?: string; operationId?: string;
  onNavigate: (path: string, replace?: boolean) => void;
  confirmationPath?: (id:string) => string; backPath?:string; stayOnReceipt?:boolean; expectedConversation?:string;
}) {
  const [preview, setPreview] = useState<ProcessingAuthorization>();
  const [receipt, setReceipt] = useState<ProcessingReceipt>();
  const [limit, setLimit] = useState<number>();
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [reload, setReload] = useState(0);
  const pending = useRef<string | undefined>(undefined);
  const pendingContext = useRef("");
  const taskIdentity = JSON.stringify([projectId, draftId, testId]);
  const alive = useRef(false);
  const task = useRef({ projectId, draftId, testId, operationId });
  task.current = { projectId, draftId, testId, operationId };
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const navigate = useRef(onNavigate); navigate.current = onNavigate;
  const key = (id: string) => `annotagent.processing-request:${projectId}:${id}`;
  const context = { draftId, sampleTestId: testId, imageId };
  const routing=useRef({confirmationPath,backPath,stayOnReceipt,expectedConversation});
  routing.current={confirmationPath,backPath,stayOnReceipt,expectedConversation};
  const requireConversation=(value:ProcessingAuthorization)=>{
    if(routing.current.expectedConversation && value.conversation?.conversation_id!==routing.current.expectedConversation)throw new Error("This processing proposal does not belong to the current conversation.");
  };
  useEffect(() => {
    const controller = new AbortController(); setError(""); setConfirmed(false); setReceipt(undefined); setPreview(undefined);
    // Moving from the preview to our newly assigned receipt is one request.
    // Any other receipt/preview is a different task, even in the same Project.
    if (pending.current && (pending.current !== operationId || pendingContext.current !== taskIdentity)) {
      pending.current = undefined; setBusy(false);
    }
    if (operationId) {
      if(pending.current===operationId)return()=>controller.abort();
      void api.processingOperation(projectId, operationId, controller.signal).then((value) => {
        if (controller.signal.aborted) return;
        if (value.id !== operationId || value.project_id !== projectId || value.draft_id !== draftId || value.request?.selection.sample_test_id !== testId) throw new Error(t("This confirmation belongs to a different task or sample test. Return to the matching samples."));
        requireConversation(value.authorization); setReceipt(value); setPreview(value.authorization);
        if (!routing.current.stayOnReceipt && value.phase === "started" && value.batch_id) navigate.current(projectBatchPath(projectId, value.batch_id), true);
      }).catch((failure: Error) => { if (!controller.signal.aborted) setError(failure.message); });
    } else if (draftId && testId) {
      void api.processingPreview(projectId, { draft_id: draftId, sample_test_id: testId, limit }, controller.signal).then((value) => {
        if (!controller.signal.aborted) {requireConversation(value);setPreview(value);}
      }).catch((failure: Error) => { if (!controller.signal.aborted) setError(failure.message); });
    } else setError(t("Return to your samples and choose a tested plan first."));
    return () => controller.abort();
  }, [projectId, draftId, testId, limit, operationId, reload]);
  const submit = async () => {
    if (pending.current || !draftId || !testId) return;
    let input: ConfirmProcessingRequest;
    try {
      if (operationId) {
        const saved = receipt?.request ?? JSON.parse(sessionStorage.getItem(key(operationId)) ?? "null") as ConfirmProcessingRequest | null;
        if (!saved || saved.request_id !== operationId || saved.selection.draft_id !== draftId || saved.selection.sample_test_id !== testId) throw new Error(t("The pending confirmation is unavailable. Return to the samples and review the scope again."));
        input = saved;
      } else {
        if (!preview || !confirmed) return;
        input = { request_id: crypto.randomUUID(), selection: { draft_id: draftId, sample_test_id: testId, limit }, expected_revision: preview.revision, authorization_fingerprint: preview.authorization_fingerprint };
        try { sessionStorage.setItem(key(input.request_id), JSON.stringify(input)); } catch { /* Server receipts remain authoritative. */ }
      }
    } catch (failure) { setError((failure as Error).message); return; }
    pending.current = input.request_id; pendingContext.current = taskIdentity; setBusy(true); setError("");
    const isCurrent = () => alive.current && pending.current === input.request_id && task.current.projectId === projectId && task.current.draftId === draftId && task.current.testId === testId && task.current.operationId === input.request_id;
    navigate.current(routing.current.confirmationPath?.(input.request_id) ?? projectJourneyPath(projectId, "confirm", { ...context, processingOperationId: input.request_id }), true);
    try {
      const value = await api.confirmProcessing(projectId, input);
      if (!isCurrent()) return;
      requireConversation(value.authorization); setReceipt(value); setPreview(value.authorization);
      if (value.phase === "started" && value.batch_id) {if(!routing.current.stayOnReceipt)navigate.current(projectBatchPath(projectId, value.batch_id), true);}
      else setError(value.error ?? t("Processing did not start. Your saved sample is unchanged."));
    } catch (failure) { if (isCurrent()) setError((failure as Error).message); }
    finally { if (isCurrent()) setBusy(false); if (pending.current === input.request_id) pending.current = undefined; }
  };
  return <section className="journey-scene" aria-label={t("Processing confirmation")}>
    <div className="journey-intro"><h2>{receipt?.phase === "started" ? t("Processing started") : receipt?.phase === "published_start_failed" ? t("Plan saved. Processing has not started.") : preview ? t("Ready to process {count} images.", { count: preview.image_count }) : t("Checking your processing scope…")}</h2>
      <p>{t("Sample tests are sandbox evaluations. Processing these images creates formal results; uncertain results still require review.")}</p></div>
    {preview && <div className="journey-consent"><h3>{preview.goal.goal || preview.plan_name}</h3><p>{t("Uses the exact plan tested in your selected samples.")}</p>
      {!operationId && <label>{t("Images to process, in dataset order")}<input type="number" min="1" max={preview.available_images} value={limit ?? preview.available_images} disabled={busy} onChange={(event) => setLimit(Math.max(1, Math.min(preview.available_images, Number(event.target.value))))} /></label>}
      <p>{t("{count} images, including any sample images. Samples did not create formal annotations.", { count: preview.image_count })}</p>
      {preview.models.map((model) => <p key={model.model_profile_id}><strong>{model.remote_model_id}</strong><br />{model.provider_base_url}</p>)}
      {preview.native_models?.map((model) => <p key={model.id}><strong>{model.name}</strong><br />{t(model.destination)}</p>)}
      <p>{t(preview.conversation ? "Cost is unknown. At most {count} model calls are authorized for this processing task, including local plugins and runtime retry attempts. Provider transport retries are disabled. This is not a monetary cap." : "Cost is unknown. At most {count} model calls are authorized across this processing task, including local plugins. Provider-internal retries may add network requests; this is not a monetary cap.", { count: preview.maximum_model_calls })}</p>
      {preview.conversation && <p>Schema revision {preview.conversation.schema.revision}: {preview.conversation.schema.definition.task.labels.join(", ")} · Tested plan revision {preview.revision}</p>}
      {preview.task_budget && <p>Before this confirmation: {preview.task_budget.total_reserved_calls} model calls reserved across this annotation task. This processing allocation adds at most {preview.maximum_model_calls} calls; prior usage is not reset. Unknown or failed requests remain counted.</p>}
      <p>{t("This does not automatically approve all future results. No-target and automatically accepted images remain available for inspection.")}</p>
      {preview.sample_feedback_count > 0 && <p className="journey-risk">{t("Your sample corrections are saved evaluation feedback. They do not change this plan's future predictions; processing may repeat the issues you identified.")}</p>}
      {!operationId && <label><input type="checkbox" checked={confirmed} disabled={busy} onChange={(event) => setConfirmed(event.target.checked)} />{t("I authorize this image, model and call-budget scope")}</label>}
    </div>}
    {busy && <p role="status">{t("Saving the confirmed plan and starting processing…")}</p>}
    {(error || receipt?.error) && <p role="alert">{error || receipt?.error}</p>}
    {stayOnReceipt && receipt?.batch_id && <ConversationBatchStatus projectId={projectId} batchId={receipt.batch_id} />}
    <footer className="journey-actions"><button onClick={() => navigate.current(routing.current.backPath ?? projectJourneyPath(projectId, "samples", context))}>{t("Back to samples")}</button>
      {operationId && <button disabled={busy} onClick={() => setReload((value) => value + 1)}>{t("Reload task status")}</button>}
      {receipt?.phase === "started" && receipt.batch_id ? <button className="primary" onClick={()=>navigate.current(projectBatchPath(projectId,receipt.batch_id!))}>{t("Open processing results")}</button> : <button className="primary" disabled={busy || (!operationId && (!preview || !confirmed))} onClick={submit}>{t(operationId ? receipt?.phase === "published_start_failed" ? "Retry starting processing" : "Retry this confirmed action" : "Confirm and start processing")}</button>}
    </footer>
  </section>;
}
