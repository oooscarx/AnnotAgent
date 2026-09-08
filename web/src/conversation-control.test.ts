import { describe, expect, it } from "vitest";
import type { ConversationMessage } from "./types";
import { composerIntent, isStopCommand, makeStopMessage, mergeConversationMessages, parsePendingStop, parseStopSelection, stopTargetMatches, mergeStopRecord, stopSelectionConflicts, isAnnotationGoalMessage, compactStopTarget } from "./conversation-control";
import type { StopRequestRecord } from "./conversation-stop-api";

describe("standalone conversation controls", () => {
  it("recognizes only a standalone stop token and never prepares it as an annotation goal", () => {
    for (const text of ["stop", " STOP ", "\n停止\t", "Stop"]) {
      expect(isStopCommand(text)).toBe(true);
      expect(composerIntent(text, true)).toEqual({ kind: "stop", prepareGoal: false });
    }
    for (const text of ["", "不要停止", "请停止", "stop!", "停止。", "find stop signs", "stop after this image", "停止然后继续", "s\ntop"]) {
      expect(isStopCommand(text)).toBe(false);
      expect(composerIntent(text, true)).toEqual({ kind: "message", prepareGoal: true });
    }
  });
  it("freezes the selected task and carries no image or candidate reference", () => {
    const input = makeStopMessage("TEST-command", "停止", "TEST-task-A");
    expect(input).toEqual({ id: "TEST-command", text: "停止", image: null, reference: { scope: "stop_request", task_id: "TEST-task-A" } });
    const selectedTaskAfterSend = "TEST-task-B";
    expect(input.reference.task_id).not.toBe(selectedTaskAfterSend);
    expect(makeStopMessage("TEST-none", "stop", null).reference.task_id).toBeNull();
    expect(() => makeStopMessage("TEST-not-command", "stop signs", null)).toThrow();
  });
  it("restores only the original pending standalone control envelope, not an arbitrary image request", () => {
    const input = makeStopMessage("TEST-command", "stop", "TEST-task");
    const pending = { conversation_id: "TEST-conversation", input };
    expect(parsePendingStop(JSON.stringify(pending))).toEqual(pending);
    expect(parsePendingStop(JSON.stringify({ ...pending, input: { ...input, image: { image_id: "TEST-image", sha256: "TEST-hash" } } }))).toBeUndefined();
    expect(parsePendingStop(JSON.stringify({ ...pending, input: { ...input, text: "stop signs" } }))).toBeUndefined();
    expect(isAnnotationGoalMessage({ conversation_id: "TEST-conversation", sequence: 1, input })).toBe(false);
  });
});

describe("fixed stop target recovery", () => {
  const target = { kind: "call" as const, id: "TEST-call", task_id: "TEST-task" };
  const message = { conversation_id: "TEST-conversation", sequence: 1, input: makeStopMessage("TEST-command", "stop", null) };
  const record: StopRequestRecord = { message, targets: [{ ...target, state: "reserved", parent_journey_ids: [] }], selected_target: null, status: "needs_selection", created_at: "TEST-time" };
  it("keeps human-readable targets compact without changing their exact identities", () => {
    const full = { ...target, id: "e7b0fd24-91cb-443a-86ac-a7c3f38712c7", task_id: "edcbdc94-f510-4ea6-8cbb-7aa0ad1524ac" };
    expect(compactStopTarget(full, "  Find   yellow\nblocks  ")).toEqual({ title: "Find yellow blocks", operationId: "e7b0fd24", taskId: "edcbdc94" });
    expect(compactStopTarget(full, "球".repeat(120)).title).toBe(`${"球".repeat(72)}…`);
    expect(compactStopTarget(full, " ").title).toBe("");
    expect(full.id).toBe("e7b0fd24-91cb-443a-86ac-a7c3f38712c7");
  });
  it("does not reinterpret a saved or pending selection as another task or kind", () => {
    const local = { message_id: "TEST-command", target, pending: true };
    expect(parseStopSelection(JSON.stringify(local), "TEST-command")).toEqual(local);
    expect(parseStopSelection(JSON.stringify(local), "OTHER-command")).toBeUndefined();
    expect(stopTargetMatches(target, { ...target, task_id: "OTHER-task" })).toBe(false);
    expect(stopTargetMatches(target, { ...target, kind: "builder" })).toBe(false);
    expect(stopSelectionConflicts(local, { ...record, selected_target: { ...target, id: "OTHER-call" } })).toBe(true);
  });
  it("keeps the cancellation receipt when a late pre-selection GET arrives", () => {
    const selected = { ...record, selected_target: target, status: "cancel_requested" as const };
    expect(mergeStopRecord(selected, record)).toEqual(selected);
    expect(mergeStopRecord(selected, { ...selected, selected_target: { ...target, id: "OTHER-call" } })).toEqual(selected);
    expect(mergeStopRecord(selected, { ...selected, observation: { state: "cancelled", description: "TEST stopped" } }).observation?.state).toBe("cancelled");
    for (const state of ["finished", "unknown"]) {
      const ended = { ...selected, observation: { state, description: "TEST settled" } };
      expect(mergeStopRecord(ended, { ...selected, observation: { state: "cancel_pending", description: "TEST old" } }).observation).toEqual(ended.observation);
    }
  });
});

describe("append-only conversation message recovery", () => {
  const message = (id: string, sequence: number, conversation = "TEST-conversation"): ConversationMessage => ({ conversation_id: conversation, sequence, input: { id, text: `TEST ${id}`, image: null } });
  it("does not erase an acknowledged new message with an older GET snapshot", () => {
    const old = message("old", 1), acknowledged = message("stop-command", 2);
    expect(mergeConversationMessages([old, acknowledged], [old], "TEST-conversation")).toEqual([old, acknowledged]);
    expect(mergeConversationMessages([acknowledged], [old, acknowledged], "TEST-conversation")).toEqual([old, acknowledged]);
  });
  it("deduplicates same-ID replay and never retains another conversation's messages", () => {
    const current = message("current", 3), other = message("other", 1, "OTHER-conversation");
    expect(mergeConversationMessages([other, current], [other, current, current], "TEST-conversation")).toEqual([current]);
    expect(mergeConversationMessages([current], [other], "OTHER-conversation")).toEqual([other]);
  });
});
