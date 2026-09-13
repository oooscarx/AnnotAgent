import { useEffect, useRef, useState } from "react";
import { Dialog } from "./Dialog";
import { Disclosure } from "./Disclosure";
import { TraceRecord } from "./TraceRecord";
import type { TaskHistoryService } from "./taskHistory";
import type {
  TaskLifecycleAction,
  TaskLifecycleEntry,
  TaskLifecycleReceipt,
  TaskLifecycleService,
  TaskLifecycleState,
} from "./taskLifecycle";

export function TaskLifecycleManagement({
  project,
  conversation,
  state,
  service,
  history,
  onChanged,
}: {
  project: string;
  conversation: string;
  state: Exclude<TaskLifecycleState, "active">;
  service: TaskLifecycleService;
  history: TaskHistoryService;
  onChanged?: (receipt: TaskLifecycleReceipt) => Promise<void>;
}) {
  const [items, setItems] = useState<TaskLifecycleEntry[]>();
  const [error, setError] = useState("");
  const [status, setStatus] = useState("");
  const [generation, setGeneration] = useState(0);
  const [pending, setPending] = useState<{ entry: TaskLifecycleEntry; action: TaskLifecycleAction; commandId: string }>();
  const [confirmed, setConfirmed] = useState(false);
  const [busyTask, setBusyTask] = useState("");
  const [inspected, setInspected] = useState<TaskLifecycleEntry>();
  const lock = useRef(false);

  useEffect(() => {
    const controller = new AbortController();
    setItems(undefined);
    setError("");
    void service.list(project, conversation, state, controller.signal)
      .then(value => { if (!controller.signal.aborted) setItems(value); })
      .catch(cause => { if (!controller.signal.aborted) setError((cause as Error).message); });
    return () => controller.abort();
  }, [conversation, generation, project, service, state]);

  const run = async () => {
    if (!pending || lock.current || (pending.action === "move_to_trash" && !confirmed)) return;
    lock.current = true;
    setBusyTask(pending.entry.task_id);
    setError("");
    setStatus("");
    try {
      const receipt = await service.mutate(project, conversation, pending.entry.task_id, {
        command_id: pending.commandId,
        expected_revision: pending.entry.lifecycle_revision,
        action: pending.action,
      });
      await onChanged?.(receipt);
      setInspected(undefined);
      setPending(undefined);
      setConfirmed(false);
      setStatus(receipt.warnings.length
        ? `操作已保存。注意：${receipt.warnings.join("；")}`
        : "操作已保存，并已从服务器重新读取任务列表。");
      setGeneration(value => value + 1);
    } catch (cause) {
      setError(`${(cause as Error).message}。服务器结果未确认时不会自动重复提交。`);
    } finally {
      lock.current = false;
      setBusyTask("");
    }
  };

  const request = (entry: TaskLifecycleEntry, action: TaskLifecycleAction) => {
    setPending({ entry, action, commandId: crypto.randomUUID() });
    setConfirmed(false);
    setError("");
    setStatus("");
  };

  return (
    <section className="task-lifecycle-management" aria-label={state === "archived" ? "已归档任务" : "任务回收站"}>
      <p>{state === "archived"
        ? "归档任务不显示在项目树中；完整对话、轨迹和上下文仍保存在服务器。"
        : "这里是可恢复的软删除任务。完整对话和执行轨迹仍被保留，除非未来执行单独的永久清理。"}</p>
      {status && <p role="status" className="notice">{status}</p>}
      {error && <p role="alert" className="error">{error}</p>}
      <button disabled={!!busyTask} onClick={() => setGeneration(value => value + 1)}>刷新列表</button>
      {!items && !error && <p role="status">正在读取服务器任务…</p>}
      {items?.length === 0 && <p>{state === "archived" ? "没有已归档任务。" : "任务回收站为空。"}</p>}
      <div className="settings-rows">
        {items?.map(entry => (
          <article className="settings-row" key={entry.task_id}>
            <div>
              <strong>{entry.title}</strong>
              <p>{entry.state} · {state === "archived" ? `归档于 ${entry.archived_at || "时间未知"}` : `移入回收站于 ${entry.trashed_at || "时间未知"}`}</p>
            </div>
            <div className="actions">
              <button aria-pressed={inspected?.task_id === entry.task_id} onClick={() => setInspected(current => current?.task_id === entry.task_id ? undefined : entry)}>查看只读轨迹</button>
              <button disabled={!!busyTask} onClick={() => request(entry, "restore")}>恢复…</button>
              {state === "archived" && <button className="danger-text" disabled={!!busyTask} onClick={() => request(entry, "move_to_trash")}>移入回收站…</button>}
            </div>
          </article>
        ))}
      </div>
      {inspected && <ReadOnlyTaskTrace key={inspected.task_id} project={project} conversation={conversation} entry={inspected} service={history} />}
      {pending && (
        <Dialog
          title={pending.action === "restore" ? "恢复任务" : "将任务移入回收站"}
          onClose={() => { if (!busyTask) setPending(undefined); }}
        >
          <p><strong>{pending.entry.title}</strong></p>
          {pending.action === "restore" ? (
            <p>恢复后，任务会重新出现在所属项目的任务树中，完整对话和轨迹保持不变。</p>
          ) : (
            <>
              <p>这是可恢复的软删除。任务会进入回收站，完整对话、执行轨迹和 JSON 上下文仍被保留。</p>
              <label className="confirm-check"><input type="checkbox" checked={confirmed} disabled={!!busyTask} onChange={event => setConfirmed(event.target.checked)} />我知道这不会立即永久清理数据</label>
            </>
          )}
          {error && <p role="alert" className="error">{error}</p>}
          <div className="actions">
            <button disabled={!!busyTask} onClick={() => setPending(undefined)}>取消</button>
            <button
              className={pending.action === "move_to_trash" ? "danger" : "primary"}
              disabled={!!busyTask || (pending.action === "move_to_trash" && !confirmed)}
              onClick={() => void run()}
            >
              {busyTask ? "正在提交…" : pending.action === "restore" ? "恢复任务" : "移入回收站"}
            </button>
          </div>
        </Dialog>
      )}
    </section>
  );
}

function ReadOnlyTaskTrace({
  project,
  conversation,
  entry,
  service,
}: {
  project: string;
  conversation: string;
  entry: TaskLifecycleEntry;
  service: TaskHistoryService;
}) {
  const [data, setData] = useState<Awaited<ReturnType<TaskHistoryService["read"]>>>();
  const [error, setError] = useState("");
  useEffect(() => {
    const controller = new AbortController();
    void service.read(project, conversation, entry.task_id, controller.signal)
      .then(value => { if (!controller.signal.aborted) setData(value); })
      .catch(cause => { if (!controller.signal.aborted) setError((cause as Error).message); });
    return () => controller.abort();
  }, [conversation, entry.task_id, project, service]);
  const groups = data ? [
    { title: "模型调用与决策回执", rows: data.workspace.calls || [] },
    { title: "方案构建与工具轨迹", rows: data.workspace.builder_operations?.items || [] },
    { title: "样例执行", rows: data.workspace.sample_operations || [] },
    { title: "全量处理", rows: data.workspace.processing_operations || [] },
    { title: "人工协助", rows: data.workspace.human_requests || [] },
    { title: "排队输入状态", rows: data.workspace.queue || [] },
  ] : [];
  return <section className="task-readonly-trace" aria-label={`只读任务轨迹 ${entry.title}`}>
    <h2>{entry.title} · 只读轨迹</h2>
    <p>归档或回收站中的任务只能查看；恢复为活动任务后才能继续发送、批准或恢复执行。</p>
    {error && <p role="alert" className="error">{error}</p>}
    {!data && !error && <p role="status">读取完整分页消息与任务快照…</p>}
    {data && <>
      <h3>多轮对话 · {data.messages.length}</h3>
      <ol>{data.messages.map(message => <li key={message.id}><p style={{ whiteSpace: "pre-wrap" }}>{message.message.input.text}</p></li>)}</ol>
      {groups.map(group => <Disclosure key={group.title} title={`${group.title} · ${group.rows.length}`}>
        {group.rows.length ? group.rows.map((row, index) => <TraceRecord key={index} value={row} index={index} />) : <p>没有保存的记录。</p>}
      </Disclosure>)}
    </>}
  </section>;
}
