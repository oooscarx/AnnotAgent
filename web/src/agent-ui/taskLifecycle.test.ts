import { describe, expect, it, vi } from "vitest";
import { createTaskLifecycleService, type TaskLifecycleTransport } from "./taskLifecycle";

const entry = (id: string, state: "active" | "archived" | "trashed") => ({
  task_id: id,
  title: `Task ${id}`,
  schema_revision: "schema-1",
  project_owner_id: "owner-1",
  conversation_id: "conversation-1",
  state: "idle",
  lifecycle_state: state,
  lifecycle_revision: 3,
  archived_at: state === "archived" ? "2026-09-13T00:00:00Z" : null,
  trashed_at: state === "trashed" ? "2026-09-13T00:00:00Z" : null,
});

describe("task lifecycle HTTP boundary", () => {
  it("reads every scoped navigation page without mixing lifecycle states", async () => {
    const calls: string[] = [];
    const transport = vi.fn(async (path: string) => {
      calls.push(path);
      if (path.includes("cursor=11")) return { items: [entry("second", "archived")], next_cursor: null };
      return { items: [entry("first", "archived")], next_cursor: 11 };
    });
    const service = createTaskLifecycleService(transport as unknown as TaskLifecycleTransport);
    expect((await service.list("project/1", "conversation-1", "archived")).map(item => item.task_id)).toEqual(["first", "second"]);
    expect(calls).toEqual([
      "/api/projects/project%2F1/conversations/conversation-1/task-navigation?state=archived&limit=100",
      "/api/projects/project%2F1/conversations/conversation-1/task-navigation?state=archived&limit=100&cursor=11",
    ]);
  });

  it("posts one exact CAS mutation and validates the returned task identity", async () => {
    const transport = vi.fn(async (_path: string, init?: RequestInit) => ({
      command_id: "command-1",
      action: "move_to_trash",
      replayed: false,
      task: { input: { id: "task-1" } },
      lifecycle: { state: "trashed", revision: 4, archived_at: null, trashed_at: "now", deletion_operation_id: "operation", updated_at: "now" },
      warnings: [],
      request: init?.body,
    }));
    const service = createTaskLifecycleService(transport as unknown as TaskLifecycleTransport);
    await expect(service.mutate("project-1", "conversation-1", "task-1", {
      command_id: "command-1",
      expected_revision: 3,
      action: "move_to_trash",
    })).resolves.toMatchObject({ lifecycle: { state: "trashed", revision: 4 } });
    expect(transport).toHaveBeenCalledTimes(1);
    expect(transport.mock.calls[0]?.[0]).toBe("/api/projects/project-1/conversations/conversation-1/tasks/task-1/lifecycle");
    expect(JSON.parse(String(transport.mock.calls[0]?.[1]?.body))).toEqual({ command_id: "command-1", expected_revision: 3, action: "move_to_trash" });
  });

  it("fails closed on a foreign receipt and never retries", async () => {
    const transport = vi.fn(async () => ({
      command_id: "command-1",
      action: "archive",
      replayed: false,
      task: { input: { id: "foreign" } },
      lifecycle: { state: "archived", revision: 4, archived_at: "now", trashed_at: null, deletion_operation_id: null, updated_at: "now" },
      warnings: [],
    }));
    const service = createTaskLifecycleService(transport as unknown as TaskLifecycleTransport);
    await expect(service.mutate("project-1", "conversation-1", "task-1", {
      command_id: "command-1",
      expected_revision: 3,
      action: "archive",
    })).rejects.toThrow("回执与原操作不匹配");
    expect(transport).toHaveBeenCalledTimes(1);
  });
});
