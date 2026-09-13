import { useEffect, useRef, useState } from "react";
import type { Task } from "./adapter";
import { Dialog } from "./Dialog";
import { Icon } from "./Icon";
import type {
  TaskLifecycleAction,
  TaskLifecycleReceipt,
  TaskLifecycleService,
} from "./taskLifecycle";

type Intent = "archive" | "move_to_trash";

export function TaskLifecycleMenu({
  task,
  service,
  onChanged,
}: {
  task: Task;
  service: TaskLifecycleService;
  onChanged: (receipt: TaskLifecycleReceipt) => Promise<void>;
}) {
  const root = useRef<HTMLDetailsElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const [intent, setIntent] = useState<Intent>();
  const [commandId, setCommandId] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const lock = useRef(false);

  useEffect(() => {
    const place = () => {
      if (!root.current?.open || !panel.current) return;
      const trigger = root.current.getBoundingClientRect();
      panel.current.style.left = `${Math.max(8, Math.min(trigger.right - panel.current.offsetWidth, innerWidth - panel.current.offsetWidth - 8))}px`;
      panel.current.style.top = `${Math.max(8, Math.min(trigger.bottom + 4, innerHeight - panel.current.offsetHeight - 8))}px`;
    };
    const close = (event: PointerEvent) => {
      if (root.current?.open && !root.current.contains(event.target as Node)) {
        root.current.open = false;
      }
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && root.current?.open) {
        root.current.open = false;
        root.current.querySelector("summary")?.focus();
      }
    };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", escape);
    window.addEventListener("resize", place);
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", escape);
      window.removeEventListener("resize", place);
    };
  }, []);

  const open = (next: Intent) => {
    root.current?.removeAttribute("open");
    setError("");
    setConfirmed(false);
    setCommandId(crypto.randomUUID());
    setIntent(next);
  };
  const closeDialog = () => {
    if (busy) return;
    setIntent(undefined);
    requestAnimationFrame(() => root.current?.querySelector<HTMLElement>("summary")?.focus());
  };

  const run = async () => {
    if (!intent || lock.current || (intent === "move_to_trash" && !confirmed)) return;
    lock.current = true;
    setBusy(true);
    setError("");
    try {
      const receipt = await service.mutate(
        task.project,
        task.conversationId!,
        task.id,
        {
          command_id: commandId,
          expected_revision: task.lifecycleRevision!,
          action: intent satisfies TaskLifecycleAction,
        },
      );
      await onChanged(receipt);
      setIntent(undefined);
    } catch (cause) {
      setError(`${(cause as Error).message}。服务器结果未确认时不会自动重复提交。`);
    } finally {
      lock.current = false;
      setBusy(false);
    }
  };

  if (!task.conversationId || task.id.startsWith("new:") || !Number.isSafeInteger(task.lifecycleRevision)) {
    return null;
  }

  return (
    <>
      <details ref={root} className="task-lifecycle-menu" onToggle={() => {
        if (!root.current?.open || !panel.current) return;
        const trigger = root.current.getBoundingClientRect();
        panel.current.style.left = `${Math.max(8, Math.min(trigger.right - panel.current.offsetWidth, innerWidth - panel.current.offsetWidth - 8))}px`;
        panel.current.style.top = `${Math.max(8, Math.min(trigger.bottom + 4, innerHeight - panel.current.offsetHeight - 8))}px`;
      }}>
        <summary aria-label={`管理任务 ${task.title}`} title={`管理任务 ${task.title}`}>
          <Icon name="more" />
        </summary>
        <div ref={panel} role="menu" aria-label={`${task.title} 操作`}>
          <button role="menuitem" onClick={() => open("archive")}>归档</button>
          <button role="menuitem" className="danger-text" onClick={() => open("move_to_trash")}>移入回收站…</button>
        </div>
      </details>
      {intent && (
        <Dialog
          title={intent === "archive" ? "归档任务" : "将任务移入回收站"}
          onClose={closeDialog}
        >
          <p><strong>{task.title}</strong></p>
          {intent === "archive" ? (
            <p>任务将从项目树中隐藏，但完整对话、执行轨迹和 JSON 上下文都会保留，可在任务历史中恢复。</p>
          ) : (
            <>
              <p>这是可恢复的软删除。任务会从项目树中隐藏，完整对话和执行轨迹仍会保留在回收站，之后可以恢复。</p>
              <label className="confirm-check"><input type="checkbox" checked={confirmed} disabled={busy} onChange={event => setConfirmed(event.target.checked)} />我知道这不会立即永久清理数据</label>
            </>
          )}
          {error && <p role="alert" className="error">{error}</p>}
          <div className="actions">
            <button disabled={busy} onClick={closeDialog}>取消</button>
            <button
              className={intent === "move_to_trash" ? "danger" : "primary"}
              disabled={busy || (intent === "move_to_trash" && !confirmed)}
              onClick={() => void run()}
            >
              {busy ? "正在提交…" : intent === "archive" ? "归档任务" : "移入回收站"}
            </button>
          </div>
        </Dialog>
      )}
    </>
  );
}
