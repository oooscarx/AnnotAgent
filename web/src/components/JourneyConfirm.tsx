import { useEffect, useRef, useState } from "react";
import { api, type ConfirmProcessingRequest, type ProcessingAuthorization, type ProcessingReceipt } from "../api";
import { t } from "../i18n";
import { projectBatchPath, projectJourneyPath } from "../navigation";

export function JourneyConfirm({ projectId, draftId, testId, imageId, operationId, onNavigate }: {
  projectId: string; draftId?: string; testId?: string; imageId?: string; operationId?: string;
  onNavigate: (path: string, replace?: boolean) => void;
}) {
  const [preview, setPreview] = useState<ProcessingAuthorization>();
  const [receipt, setReceipt] = useState<ProcessingReceipt>();
  const [limit, setLimit] = useState<number>();
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [reload, setReload] = useState(0);
  const pending = useRef(false);
  const alive = useRef(false);
  const task = useRef({ projectId, draftId, testId });
  task.current = { projectId, draftId, testId };
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const navigate = useRef(onNavigate); navigate.current = onNavigate;
  const key = (id: string) => `annotagent.processing-request:${projectId}:${id}`;
  const context = { draftId, sampleTestId: testId, imageId };
  useEffect(() => {
    const controller = new AbortController(); setError(""); setConfirmed(false); setReceipt(undefined); setPreview(undefined);
    if (operationId) {
      void api.processingOperation(projectId, operationId, controller.signal).then((value) => {
        if (controller.signal.aborted) return;
        if (value.project_id !== projectId || value.draft_id !== draftId) throw new Error("This confirmation belongs to a different task.");
        setReceipt(value); setPreview(value.authorization);
        if (value.phase === "started" && value.batch_id) navigate.current(projectBatchPath(projectId, value.batch_id), true);
      }).catch((failure: Error) => { if (!controller.signal.aborted) setError(failure.message); });
    } else if (draftId && testId) {
      void api.processingPreview(projectId, { draft_id: draftId, sample_test_id: testId, limit }, controller.signal).then((value) => {
        if (!controller.signal.aborted) setPreview(value);
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
    pending.current = true; setBusy(true); setError("");
    const isCurrent = () => alive.current && task.current.projectId === projectId && task.current.draftId === draftId && task.current.testId === testId;
    navigate.current(projectJourneyPath(projectId, "confirm", { ...context, processingOperationId: input.request_id }), true);
    try {
      const value = await api.confirmProcessing(projectId, input);
      if (!isCurrent()) return;
      setReceipt(value); setPreview(value.authorization);
      if (value.phase === "started" && value.batch_id) navigate.current(projectBatchPath(projectId, value.batch_id), true);
      else setError(value.error ?? t("Processing did not start. Your saved sample is unchanged."));
    } catch (failure) { if (isCurrent()) setError((failure as Error).message); }
    finally { pending.current = false; if (isCurrent()) setBusy(false); }
  };
  return <section className="journey-scene" aria-label={t("Processing confirmation")}>
    <div className="journey-intro"><h2>{receipt?.phase === "published_start_failed" ? t("Plan saved. Processing has not started.") : preview ? t("Ready to process {count} images.", { count: preview.image_count }) : t("Checking your processing scope…")}</h2>
      <p>{t("Sample tests are sandbox evaluations. Processing these images creates formal results; uncertain results still require review.")}</p></div>
    {preview && <div className="journey-consent"><h3>{preview.goal.goal || preview.plan_name}</h3><p>{t("Uses the exact plan tested in your selected samples.")}</p>
      {!operationId && <label>{t("Images to process, in dataset order")}<input type="number" min="1" max={preview.available_images} value={limit ?? preview.available_images} disabled={busy} onChange={(event) => setLimit(Math.max(1, Math.min(preview.available_images, Number(event.target.value))))} /></label>}
      <p>{t("{count} images, including any sample images. Samples did not create formal annotations.", { count: preview.image_count })}</p>
      {preview.models.map((model) => <p key={model.model_profile_id}><strong>{model.remote_model_id}</strong><br />{model.provider_base_url}</p>)}
      <p>{t("Cost is unknown. At most {count} external model calls are authorized across this processing task. Provider-internal retries may add network requests; this is not a monetary cap.", { count: preview.maximum_model_calls })}</p>
      <p>{t("This does not automatically approve all future results. No-target and automatically accepted images remain available for inspection.")}</p>
      {!operationId && <label><input type="checkbox" checked={confirmed} disabled={busy} onChange={(event) => setConfirmed(event.target.checked)} />{t("I authorize this image, model and call-budget scope")}</label>}
    </div>}
    {busy && <p role="status">{t("Saving the confirmed plan and starting processing…")}</p>}
    {(error || receipt?.error) && <p role="alert">{error || receipt?.error}</p>}
    <footer className="journey-actions"><button onClick={() => navigate.current(projectJourneyPath(projectId, "samples", context))}>{t("Back to samples")}</button>
      {operationId && <button disabled={busy} onClick={() => setReload((value) => value + 1)}>{t("Reload task status")}</button>}
      <button className="primary" disabled={busy || (!operationId && (!preview || !confirmed))} onClick={submit}>{t(operationId ? receipt?.phase === "published_start_failed" ? "Retry starting processing" : "Retry this confirmed action" : "Confirm and start processing")}</button>
    </footer>
  </section>;
}
