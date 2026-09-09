import { useEffect, useRef, useState } from "react";
import { isStopMessage, makeStopMessage, parsePendingStop, type StopMessageInput } from "../conversation-control";
import { stopApi, type StopRequestRecord } from "../conversation-stop-api";

/** Independent from Composer text/Send. Existing stop API owns target selection
 * and safe-boundary observations; this button never infers stopped from fetch. */
export function AgentStopControl({ project, conversation, task, onRecord }: {
  project: string; conversation: string; task: string; onRecord: (record: StopRequestRecord) => void;
}) {
  const key = `annotagent.composer-stop:${project}:${conversation}:${task}`;
  const command = useRef<StopMessageInput | undefined>(undefined);
  const [busy, setBusy] = useState(false), [ready, setReady] = useState(false), [error, setError] = useState("");
  const [retry, setRetry] = useState(false);
  const pending = useRef(false), alive = useRef(true), receive = useRef(onRecord);
  receive.current = onRecord;
  function accept(record: StopRequestRecord, input: StopMessageInput) {
    if (record.message.conversation_id !== conversation || !isStopMessage(record.message.input) || record.message.input.id !== input.id || record.message.input.text !== input.text || record.message.input.reference.task_id !== task) throw new Error("Stop receipt does not match this task and command.");
    receive.current(record); sessionStorage.removeItem(key); command.current = undefined; setRetry(false); setError("");
  }
  useEffect(() => {
    alive.current = true;
    const controller = new AbortController();
    async function recover() {
      try {
        const saved = parsePendingStop(sessionStorage.getItem(key));
        if (saved && saved.conversation_id === conversation && saved.input.reference.task_id === task) {
          command.current = saved.input; setRetry(true);
          const record = await stopApi.read(project, conversation, saved.input.id, controller.signal);
          if (!controller.signal.aborted && record) accept(record, saved.input);
        }
      } catch (reason) { if (!controller.signal.aborted) setError((reason as Error).message); }
      finally { if (!controller.signal.aborted) setReady(true); }
    }
    void recover();
    return () => { alive.current = false; controller.abort(); };
  }, [key]);
  async function stop() {
    if (pending.current || !ready) return;
    pending.current = true; setBusy(true); setError("");
    const input = command.current ?? makeStopMessage(crypto.randomUUID(), "stop", task);
    command.current = input;
    try {
      sessionStorage.setItem(key, JSON.stringify({ conversation_id: conversation, input }));
      const record = await stopApi.begin(project, conversation, input);
      if (alive.current) accept(record, input);
    } catch (reason) {
      if (alive.current) {
        setRetry(true); setError(`Stop acknowledgement unconfirmed: ${(reason as Error).message}`);
        try { const record = await stopApi.read(project, conversation, input.id); if (alive.current && record) accept(record, input); } catch { /* Same command only; no automatic POST. */ }
      }
    } finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  return <div className="agent-stop-control">
    <button type="button" disabled={busy || !ready} onClick={()=>void stop()}>{busy ? "Stopping…" : retry ? "Retry same stop" : "Stop task"}</button>
    {busy && <span className="sr-only" role="status">Stopping: waiting for the server. This does not confirm a safe boundary.</span>}
    {error && <p role="alert">{error}</p>}
  </div>;
}
