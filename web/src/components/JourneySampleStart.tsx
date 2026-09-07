import { useEffect, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";

export function JourneySampleStart({ projectId, draftId, busy, stale, onTest, onBack }: {
  projectId: string; draftId?: string; busy: boolean; stale: boolean;
  onTest: (revision: number, imageCount: number, authorizationFingerprint: string) => void; onBack: () => void;
}) {
  const [preview, setPreview] = useState<Awaited<ReturnType<typeof api.samplePreview>>>();
  const [error, setError] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  useEffect(() => {
    let current = true; setPreview(undefined); setConfirmed(false); setError("");
    if (draftId) void api.samplePreview(draftId).then((value) => {
      if (!current) return;
      if (value.project_id !== projectId) throw new Error("This plan belongs to another Project.");
      setPreview(value);
    }).catch((error: Error) => { if (current) setError(error.message); });
    return () => { current = false; };
  }, [draftId, projectId]);
  return <section className="journey-scene" aria-label={t("Sample authorization")}>
    <div className="journey-intro"><h2>{t(busy ? "Testing your images." : "Try the plan on a few images.")}</h2><p>{t("Only sample images will be processed. This does not publish the plan or create formal annotations.")}</p></div>
    {busy ? <p role="status">{t("Waiting for the sample service. No percentage is available.")}</p> : <>
      {stale && <p role="alert">{t("The plan changed after the previous test. New authorization is needed.")}</p>}
      {!draftId && <p>{t("Save your goal and prepare a plan first.")}</p>}
      {preview && <section className="journey-consent"><h3>{t("{count} sample images", { count: preview.image_count })}</h3>{preview.models.map((model) => <p key={model.id}><strong>{model.name}</strong><br />{t(model.destination)}</p>)}
        {!!preview.other_bindings.length && <p>{t("Some model destinations could not be resolved. Testing is blocked until their connections are verified.")}</p>}
        <p>{t("Estimated cost: unknown. At most {count} model calls across these samples; service-internal retries may add network requests. This is not a monetary spending cap.", { count: preview.request_limit })}</p>
        {!preview.supported && <p>{t("This plan is not yet supported by bounded guided sampling. Saved results remain available; no inference has started.")}</p>}
        <label><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} />{t("I reviewed the sample scope")}</label>
      </section>}
    </>}
    {error && <p role="alert">{error}</p>}
    <footer className="journey-actions"><button disabled={busy} onClick={onBack}>{t("Back to goal")}</button>
      {preview?.supported && <button className="primary" disabled={!confirmed || busy || !preview.image_count || !!preview.other_bindings.length} onClick={() => onTest(preview.revision, preview.image_count, preview.authorization_fingerprint)}>{t("Test samples")}</button>}
    </footer>
  </section>;
}
