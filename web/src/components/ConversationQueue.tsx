import { useEffect, useState } from "react";
import { api } from "../api";
import type { SendCommand, SendReceipt } from "../conversation-send";
export type QueuedMessage = { input: SendCommand; receipt: SendReceipt; status: "waiting_for_dispatch" | "authorized" | "running" | "completed" | "failed" | "in_doubt" | "cancelled"; cancelled_at: string | null; planning_call_id: string | null };
const labels: Record<QueuedMessage["status"], string> = { waiting_for_dispatch: "Queued · not executed", authorized: "Authorized · not started", running: "Running", completed: "Planning response saved", failed: "Planning failed", in_doubt: "Outcome unknown · do not resend", cancelled: "Cancelled · not resumed" };
export function QueueRows({ rows, busy, onCancel }: { rows: QueuedMessage[]; busy: boolean; onCancel: (id: string) => void }) {
  return <ol className="agent-queue-rows">{rows.map(row => <li key={row.input.message.id}>
    <p>{row.input.message.text}</p><small>{labels[row.status]} · {row.receipt.mode === "execute" ? "Execute requested" : "Plan requested"}</small>
    {row.input.message.image && <small>Saved image reference: {row.input.message.image.image_id}</small>}
    <small>Agent model: {row.receipt.agent_model?.model_profile_id ?? "Default selection at Send"}</small>
    {row.status === "running" ? <small>Use the task stop control to interrupt this call.</small> : ["waiting_for_dispatch", "authorized", "in_doubt"].includes(row.status) && <button type="button" disabled={busy} onClick={() => onCancel(row.input.message.id)}>Cancel queued instruction</button>}
  </li>)}</ol>;
}
/** Journal-backed inbox. Reads never dispatch a call or grant permission. */
export function ConversationQueue({ project, conversation, task, journalRevision }: { project: string; conversation: string; task: string; journalRevision: number }) {
  const [rows, setRows] = useState<QueuedMessage[]>([]), [error, setError] = useState("");
  const [actionError, setActionError] = useState("");
  const [busy, setBusy] = useState(false), [revision, setRevision] = useState(0);
  const [limit, setLimit] = useState(100), [more, setMore] = useState(false);
  useEffect(() => {
    const controller = new AbortController(); let timer: ReturnType<typeof setTimeout>;
    async function read() {
      try {
        const next: QueuedMessage[] = []; let page: QueuedMessage[];
        do {
          page = await api.conversationMessageQueue(project, conversation, task, next.at(-1)?.receipt.message.sequence ?? 0, controller.signal);
          next.push(...page);
        } while (page.length === 100 && next.length < limit);
        if (controller.signal.aborted) return;
        setRows(next); setMore(page.length === 100); setError("");
        timer = setTimeout(() => void read(), 3000);
      } catch (reason) { if (!controller.signal.aborted) setError(reason instanceof Error ? reason.message : "Cannot read saved queue"); }
    }
    void read(); return () => { controller.abort(); clearTimeout(timer); };
  }, [project, conversation, task, journalRevision, revision, limit]);
  async function cancel(id: string) {
    if (busy) return; setBusy(true); setActionError("");
    try { await api.cancelConversationQueuedMessage(project, conversation, task, id); setRevision(value => value + 1); }
    catch (reason) { setActionError(reason instanceof Error ? reason.message : "Cancellation was not confirmed; refresh before retrying."); }
    finally { setBusy(false); }
  }
  if (!rows.length && !error && !actionError) return null;
  return <details className="agent-message-queue"><summary>Queued instructions and history · {rows.length}{more ? "+" : ""}</summary>
    <p>Saved supplements do not change an in-flight request. Queued instructions have not yet been applied.</p>
    {(error || actionError) && <div role="alert"><p>{actionError || error}</p><button type="button" onClick={() => { setActionError(""); setRevision(value => value + 1); }}>Refresh queue</button></div>}
    <QueueRows rows={rows} busy={busy} onCancel={id => void cancel(id)} />
    {more && <button type="button" onClick={() => setLimit(value => value + 100)}>Load more queued history</button>}
  </details>;
}
