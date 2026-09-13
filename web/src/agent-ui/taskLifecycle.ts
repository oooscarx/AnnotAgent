import { request } from "../api";

export type TaskLifecycleState = "active" | "archived" | "trashed";
export type TaskLifecycleAction = "archive" | "move_to_trash" | "restore";

export type TaskLifecycle = {
  state: TaskLifecycleState;
  revision: number;
  archived_at: string | null;
  trashed_at: string | null;
  deletion_operation_id: string | null;
  updated_at: string;
};

export type TaskLifecycleEntry = {
  task_id: string;
  title: string;
  schema_revision: string;
  project_owner_id: string;
  conversation_id: string;
  state: string;
  lifecycle_state: TaskLifecycleState;
  lifecycle_revision: number;
  archived_at: string | null;
  trashed_at: string | null;
};

export type TaskLifecycleReceipt = {
  command_id: string;
  action: TaskLifecycleAction;
  replayed: boolean;
  task: { input?: { id?: string }; task_id?: string };
  lifecycle: TaskLifecycle;
  warnings: string[];
};

export type TaskLifecycleService = {
  list(
    project: string,
    conversation: string,
    state: TaskLifecycleState,
    signal?: AbortSignal,
  ): Promise<TaskLifecycleEntry[]>;
  mutate(
    project: string,
    conversation: string,
    task: string,
    input: {
      command_id: string;
      expected_revision: number;
      action: TaskLifecycleAction;
    },
  ): Promise<TaskLifecycleReceipt>;
  receipt(
    project: string,
    conversation: string,
    commandId: string,
    signal?: AbortSignal,
  ): Promise<TaskLifecycleReceipt>;
};

export type TaskLifecycleTransport = <T>(path: string, init?: RequestInit) => Promise<T>;
type NavigationPage = {
  items: TaskLifecycleEntry[];
  next_cursor: string | number | null;
};

const segment = encodeURIComponent;
const root = (project: string, conversation: string) =>
  `/api/projects/${segment(project)}/conversations/${segment(conversation)}`;

function assertEntry(
  entry: TaskLifecycleEntry,
  conversation: string,
  expectedState: TaskLifecycleState,
) {
  if (
    entry.conversation_id !== conversation ||
    entry.lifecycle_state !== expectedState ||
    !entry.task_id ||
    !Number.isSafeInteger(entry.lifecycle_revision)
  ) {
    throw new Error("任务生命周期列表的归属、状态或版本不匹配");
  }
}

function receiptTaskId(receipt: TaskLifecycleReceipt) {
  return receipt.task.task_id || receipt.task.input?.id;
}

function assertReceipt(
  receipt: TaskLifecycleReceipt,
  task: string | undefined,
  commandId: string,
) {
  if (
    receipt.command_id !== commandId ||
    (task !== undefined && receiptTaskId(receipt) !== task) ||
    !Number.isSafeInteger(receipt.lifecycle.revision)
  ) {
    throw new Error("任务生命周期回执与原操作不匹配；请刷新后核实，未自动重试");
  }
  return receipt;
}

export function createTaskLifecycleService(
  transport: TaskLifecycleTransport = request,
): TaskLifecycleService {
  return {
    async list(project, conversation, state, signal) {
      const items: TaskLifecycleEntry[] = [];
      const cursors = new Set<string>();
      let cursor: string | number | null = null;
      do {
        const query = new URLSearchParams({ state, limit: "100" });
        if (cursor !== null) query.set("cursor", String(cursor));
        const page = await transport<NavigationPage>(
          `${root(project, conversation)}/task-navigation?${query}`,
          { signal },
        );
        for (const entry of page.items) {
          assertEntry(entry, conversation, state);
          items.push(entry);
        }
        cursor = page.next_cursor;
        if (cursor !== null) {
          if (cursors.has(String(cursor))) {
            throw new Error("任务生命周期列表分页未前进");
          }
          cursors.add(String(cursor));
        }
      } while (cursor !== null);
      return items;
    },
    async mutate(project, conversation, task, input) {
      const receipt = await transport<TaskLifecycleReceipt>(
        `${root(project, conversation)}/tasks/${segment(task)}/lifecycle`,
        { method: "POST", body: JSON.stringify(input) },
      );
      return assertReceipt(receipt, task, input.command_id);
    },
    async receipt(project, conversation, commandId, signal) {
      const value = await transport<TaskLifecycleReceipt>(
        `${root(project, conversation)}/task-lifecycle-operations/${segment(commandId)}`,
        { signal },
      );
      return assertReceipt(value, undefined, commandId);
    },
  };
}
