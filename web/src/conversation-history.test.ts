import { expect, test, vi } from "vitest";
import { loadConversationHistory } from "./conversation-history";
import type { ConversationMessage } from "./types";

const message = (sequence: number): ConversationMessage => ({ conversation_id: "TEST", sequence, input: { id: `TEST-${sequence}`, text: `TEST note ${sequence}`, image: null } });
test("old selected task and candidate reference are fetched exactly without draining history", async () => {
  const reader = { latest: vi.fn(async () => [message(300)]), firstGoal: vi.fn(async () => undefined), exact: vi.fn(async (id: string) => message(Number(id.split("-")[1]))) };
  const result = await loadConversationHistory(reader, "TEST-1", "TEST-2", new AbortController().signal);
  expect(result.messages).toEqual([message(300)]);
  expect(result.context).toEqual([message(1), message(2)]);
  expect(reader.firstGoal).not.toHaveBeenCalled();
  expect(reader.exact).toHaveBeenCalledTimes(2);
});
test("the default goal remains the oldest note rather than the visible tail", async () => {
  const reader = { latest: vi.fn(async () => [message(300)]), firstGoal: vi.fn(async () => message(1)), exact: vi.fn() };
  expect((await loadConversationHistory(reader, undefined, undefined, new AbortController().signal)).defaultGoal).toEqual(message(1));
  expect(reader.firstGoal).toHaveBeenCalledTimes(1);
});
test("visible references are reused and an aborted read admits no context requests", async () => {
  const reader = { latest: vi.fn(async () => [message(300)]), firstGoal: vi.fn(), exact: vi.fn() };
  await loadConversationHistory(reader, "TEST-300", "TEST-300", new AbortController().signal);
  expect(reader.exact).not.toHaveBeenCalled();
  const controller = new AbortController();
  reader.latest.mockImplementationOnce(async () => { controller.abort(); return [message(300)]; });
  await expect(loadConversationHistory(reader, "TEST-1", undefined, controller.signal)).rejects.toMatchObject({ name: "AbortError" });
  expect(reader.exact).not.toHaveBeenCalled();
});
test("an unavailable exact source is an error, never replaced with a newer goal", async () => {
  const reader = { latest: vi.fn(async () => [message(300)]), firstGoal: vi.fn(), exact: vi.fn(async () => { throw new Error("Source unavailable"); }) };
  await expect(loadConversationHistory(reader, "TEST-1", undefined, new AbortController().signal)).rejects.toThrow("Source unavailable");
  expect(reader.firstGoal).not.toHaveBeenCalled();
});
test("an unavailable optional reference preserves the valid task and exposes the read error", async () => {
  const reader = { latest: vi.fn(async () => [message(1)]), firstGoal: vi.fn(), exact: vi.fn(async () => { throw new Error("Reference missing"); }) };
  const result = await loadConversationHistory(reader, "TEST-1", "TEST-missing", new AbortController().signal);
  expect(result.context).toEqual([message(1)]);
  expect(result.messages).toEqual([message(1)]);
  expect(result.referenceError).toBe("Reference missing");
  expect(reader.firstGoal).not.toHaveBeenCalled();
});
test("a reference-only journal returns no goal with one bounded discovery request", async () => {
  const reader = { latest: vi.fn(async () => [message(10_000)]), firstGoal: vi.fn(async () => undefined), exact: vi.fn() };
  const result = await loadConversationHistory(reader, undefined, undefined, new AbortController().signal);
  expect(result.defaultGoal).toBeUndefined();
  expect(reader.firstGoal).toHaveBeenCalledTimes(1);
  expect(reader.exact).not.toHaveBeenCalled();
});
test("a late goal read cannot replace a newly selected task", async () => {
  const controller = new AbortController();
  const reader = { latest: vi.fn(async () => []), firstGoal: vi.fn(async () => { controller.abort(); return message(1); }), exact: vi.fn() };
  await expect(loadConversationHistory(reader, undefined, undefined, controller.signal)).rejects.toMatchObject({ name: "AbortError" });
});
