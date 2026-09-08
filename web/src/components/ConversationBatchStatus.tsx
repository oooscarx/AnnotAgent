import { useEffect, useRef } from "react";
import { api } from "../api";
import { queryKeys } from "../queryCache";
import { useRouteQuery } from "../useRouteQuery";
import { BatchControls } from "./BatchControls";

/** Read the owned Batch, not the historical start receipt. Never selects a canvas image. */
export function ConversationBatchStatus({ projectId, batchId }: { projectId: string; batchId: string }) {
  const query = useRouteQuery(queryKeys.batch(batchId), signal => api.batch(batchId, signal));
  const batch = query.data?.batch.id === batchId && query.data.batch.project_id === projectId ? query.data.batch : undefined;
  const retry = useRef(query.retry); retry.current = query.retry;
  const polling = Boolean(batch && ["pending", "running", "paused", "awaiting_review"].includes(batch.status));
  useEffect(() => {
    if (!polling) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try { await retry.current(); } catch { /* The query renders the error; do not restart execution. */ }
      if (!stopped) timer = setTimeout(poll, 2000);
    };
    timer = setTimeout(poll, 2000);
    return () => { stopped = true; clearTimeout(timer); };
  }, [batchId, projectId, polling]);
  return <section className="conversation-batch-status" aria-label="Current processing status">
    {query.error && <p role="alert">Current progress could not be refreshed: {query.error.message}</p>}
    {!batch && <p role="status">{query.loading ? "Loading current processing status…" : "This processing task is unavailable in this Project."}</p>}
    {batch && <>
      <p role="status">Current status: <strong>{batch.status.replaceAll("_", " ")}</strong></p>
      <p>{batch.progress.completed_images} completed · {batch.progress.review_images} awaiting review · {batch.progress.failed_images} failed · {batch.progress.cancelled_images} cancelled · {batch.progress.total_images} total</p>
      {polling && <p>Leaving this page does not cancel processing. Stopping cannot undo model requests already sent.</p>}
      {batch.status === "awaiting_review" && <p>Human review is required. Open processing results to inspect and review the saved candidates.</p>}
      <BatchControls key={batch.id} batchId={batch.id} status={batch.status} onReload={() => retry.current()} />
    </>}
    {query.error && <button onClick={() => void query.retry().catch(() => undefined)}>Reload processing status</button>}
  </section>;
}
