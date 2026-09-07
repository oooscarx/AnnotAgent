import { useEffect, useRef, useState } from "react";
import { api, ApiRequestError, type SampleOperation } from "../api";
import { t } from "../i18n";
import { JourneySampleStart } from "./JourneySampleStart";
import type { ImageItem } from "../types";

/** A durable receipt for the existing sandbox executor, not a second execution path. */
export function JourneySampleTask({ projectId, draftId, operationId, images, stale, onOperation, onComplete, onBack }: {
  projectId: string; draftId: string; operationId?: string; stale: boolean;
  images: ImageItem[];
  onOperation: (id?: string) => void; onComplete: (id: string) => void; onBack: () => void;
}) {
  const [operation, setOperation] = useState<SampleOperation>();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [reload, setReload] = useState(0);
  const [canRetryRequest, setCanRetryRequest] = useState(false);
  const pending = useRef(false);
  const callbacks = useRef({ onComplete });
  callbacks.current = { onComplete };
  const pendingKey = (id: string) => `annotagent.sample-request:${projectId}:${id}`;
  useEffect(() => {
    setOperation(undefined); setError(""); setCanRetryRequest(false);
    if (!operationId) return;
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      try {
        const value = await api.sampleOperation(projectId, operationId, controller.signal);
        if (controller.signal.aborted) return;
        if (value.project_id !== projectId || value.draft_id !== draftId) throw new Error("This sample task belongs to a different Project or plan.");
        setOperation(value); setError("");
        try { sessionStorage.removeItem(pendingKey(operationId)); } catch { /* Receipt is authoritative. */ }
        if (value.status === "succeeded") callbacks.current.onComplete(value.id);
        else if (["queued", "running", "cancelling"].includes(value.status)) timer = setTimeout(poll, 1000);
      } catch (failure) {
        if (!controller.signal.aborted) {
          setError((failure as Error).message);
          if (failure instanceof ApiRequestError && failure.status === 404) {
            try { setCanRetryRequest(!!sessionStorage.getItem(pendingKey(operationId))); } catch { /* No pending browser receipt. */ }
          }
        }
      }
    };
    // The URL is saved before POST; allow its receipt to be persisted before the first GET.
    timer = setTimeout(poll, 250);
    return () => { controller.abort(); clearTimeout(timer); };
  }, [projectId, draftId, operationId, reload]);
  const start = async (revision: number, count: number, fingerprint: string) => {
    if (pending.current) return;
    pending.current = true; setBusy(true); setError("");
    const id = crypto.randomUUID();
    const input = { request_id: id, draft_id: draftId, image_indices: Array.from({ length: count }, (_, index) => index), expected_revision: revision, authorization_fingerprint: fingerprint };
    // Only a bounded, non-secret request envelope; never replayed by mount or GET.
    try { sessionStorage.setItem(pendingKey(id), JSON.stringify(input)); } catch { /* Server receipt still survives a successful POST. */ }
    onOperation(id);
    try {
      await api.startSampleOperation(projectId, input);
      setReload((value) => value + 1);
    } catch (failure) {
      setError((failure as Error).message);
    } finally { pending.current = false; setBusy(false); }
  };
  const retryRequest = async () => {
    if (!operationId || pending.current) return;
    pending.current = true; setBusy(true);
    try {
      const raw = sessionStorage.getItem(pendingKey(operationId));
      if (!raw) throw new Error("The pending request is unavailable. Return to the goal and review the sample scope.");
      const input = JSON.parse(raw) as Parameters<typeof api.startSampleOperation>[1];
      if (input.request_id !== operationId || input.draft_id !== draftId) throw new Error("The pending request does not match this task.");
      await api.startSampleOperation(projectId, input);
      setReload((value) => value + 1);
    } catch (failure) { setError((failure as Error).message); }
    finally { pending.current = false; setBusy(false); }
  };
  const stop = async () => {
    if (!operationId || pending.current) return;
    pending.current = true; setBusy(true);
    try { setOperation(await api.cancelSampleOperation(projectId, operationId)); setReload((value) => value + 1); }
    catch (failure) { setError((failure as Error).message); }
    finally { pending.current = false; setBusy(false); }
  };
  if (!operationId) return <JourneySampleStart projectId={projectId} draftId={draftId} busy={busy} stale={stale} onTest={start} onBack={onBack} />;
  const active = !!operation && ["queued", "running", "cancelling"].includes(operation.status);
  return <section className="journey-scene" aria-label={t("Sample task")}>
    <div className="journey-intro"><h2>{t(active ? "Testing your images." : "Sample task status")}</h2>
      <p role="status" aria-live="polite">{operation ? t({ queued: "Waiting to start", running: "Testing samples — no percentage is available", cancelling: "Stopping local execution", cancelled: "Stopped", interrupted: "Interrupted by server restart", failed: "Sample task failed", succeeded: "Sample test completed" }[operation.status]) : t("Restoring the saved sample task…")}</p>
      <p>{t("Leaving this page does not stop the task. Refreshing only reads its saved status.")}</p>
      <p>{t("Stopping cancels local execution. Requests already sent to a remote model may still be billed. Samples never write formal annotations.")}</p>
    </div>
    {!!images.length && <section aria-label={t("Sample input previews")}>
      <p>{t("Project image previews — these are not model results.")}</p>
      <div className="journey-image-grid journey-sample-inputs">{images.slice(0, 3).map((image) => <figure key={image.image_id}><img src={image.url} alt={image.name} /><figcaption>{image.name}</figcaption></figure>)}</div>
    </section>}
    {(error || (operation?.error && operation.status !== "cancelled")) && <p role="alert">{error || operation?.error}</p>}
    <footer className="journey-actions"><button onClick={onBack}>{t("Back to goal")}</button>
      {active && <button disabled={busy || operation.status === "cancelling"} onClick={stop}>{t("Stop sample test")}</button>}
      {error && <button disabled={busy} onClick={() => setReload((value) => value + 1)}>{t("Reload task status")}</button>}
      {canRetryRequest && <button disabled={busy} onClick={retryRequest}>{t("Retry this authorized request")}</button>}
      {operation && !active && operation.status !== "succeeded" && <button className="primary" onClick={() => onOperation(undefined)}>{t("Review scope for a new test")}</button>}
    </footer>
  </section>;
}
