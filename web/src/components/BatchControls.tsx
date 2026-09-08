import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";

export function batchActions(status: string): ("pause" | "resume" | "cancel")[] {
  if (status === "running") return ["pause", "cancel"];
  if (status === "paused" || status === "pending") return ["resume", "cancel"];
  return [];
}

/** Explicit controls over the existing coordinator; mounting never performs an action. */
export function BatchControls({ batchId, status, onReload }: {
  batchId: string; status: string; onReload: () => Promise<unknown>;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const lock = useRef(false);
  const generation = useRef(0);
  useEffect(() => { generation.current++; return () => { generation.current++; }; }, [batchId]);
  const control = async (action: "pause" | "resume" | "cancel") => {
    if (lock.current) return;
    const current = generation.current;
    lock.current = true; setBusy(true); setError("");
    try { await api.controlBatch(batchId, action); if (current === generation.current) await onReload(); }
    catch (failure) { if (current === generation.current) setError((failure as Error).message); }
    finally { lock.current = false; if (current === generation.current) setBusy(false); }
  };
  return <>
    {batchActions(status).length > 0 && <div className="button-row" aria-label={t("Processing controls")}>
      {batchActions(status).map(action => <button key={action} disabled={busy} className={action === "cancel" ? "danger-button" : undefined} onClick={() => void control(action)}>
        {t(action === "cancel" ? "Cancel processing" : action === "pause" ? "Pause" : "Resume")}
      </button>)}
    </div>}
    {error && <p role="alert">{error}</p>}
  </>;
}
