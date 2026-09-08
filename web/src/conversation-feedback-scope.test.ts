import { describe, expect, it } from "vitest";
import { candidateFeedbackKind, feedbackScopeVisible, makeFeedbackScopeChoice, parseLocalScopeAnswer, sameScopeAnswerInput, scopeAnswerConflicts } from "./conversation-feedback-scope";
import { mergeFeedbackStatus } from "./conversation-feedback";
import type { FeedbackStatus, ScopeAnswerRecord } from "./conversation-feedback-api";

const input = { command_id: "TEST-command", expected_context_digest: "TEST-digest", choice: { scope: "current_candidate" as const, reason: "poor_boundary" as const } };
const answer: ScopeAnswerRecord = { call_id: "TEST-call", task_id: "TEST-task", conversation_id: "TEST-conversation", input, created_at: "2026-09-08T00:00:00Z" };

describe("structured scope answers", () => {
  it("retains cancelled unsaved choices or an unknown answer command in the same mounted scope view", () => {
    const local = { call_id: "TEST-call", context_digest: "TEST-digest", scope: "current_candidate" as const, reason: "poor_boundary" as const };
    expect(feedbackScopeVisible({ isClarification: true, answer: null, cancelled: false, local })).toBe(true);
    // After the server cancellation, the model decision is no longer actionable.
    // The old parent condition hid/unmounted the only copy when browser storage failed.
    expect(feedbackScopeVisible({ isClarification: false, answer: null, cancelled: true, local })).toBe(true);
    expect(feedbackScopeVisible({ isClarification: false, answer: null, cancelled: true, local: { ...local, frozen: input } })).toBe(true);
  });
  it("does not show a made-up scope question for other saved feedback calls", () => {
    const local = { call_id: "TEST-call", context_digest: "TEST-digest" };
    expect(feedbackScopeVisible({ isClarification: false, answer: null, cancelled: false, local })).toBe(false);
    expect(feedbackScopeVisible({ isClarification: false, answer: null, cancelled: true, local })).toBe(false);
    expect(feedbackScopeVisible({ isClarification: true, answer: null, cancelled: true, local })).toBe(false);
    expect(feedbackScopeVisible({ isClarification: false, answer, cancelled: true, local })).toBe(true);
  });
  it("uses the frozen server outcome type rather than inferring it from prose or a label", () => {
    expect(candidateFeedbackKind({ outcome: { value: { kind: "bounding_box" } } })).toBe("bounding_box");
    expect(candidateFeedbackKind({ outcome: { value: { kind: "classification" } } })).toBe("classification");
    for (const invalid of [null, "bounding_box", [], { label: "bounding_box" }, { outcome: { value: { kind: "mask" } } }]) expect(candidateFeedbackKind(invalid)).toBeUndefined();
  });
  it("requires explicit scope and correction reason; only boxes support boundary answers", () => {
    expect(makeFeedbackScopeChoice(undefined, undefined, "bounding_box")).toBeUndefined();
    expect(makeFeedbackScopeChoice("current_candidate", undefined, "bounding_box")).toBeUndefined();
    expect(makeFeedbackScopeChoice("current_candidate", "poor_boundary", "classification")).toBeUndefined();
    expect(makeFeedbackScopeChoice("current_candidate", "wrong_label", undefined)).toBeUndefined();
    expect(makeFeedbackScopeChoice("current_candidate", "poor_boundary", "bounding_box")).toEqual(input.choice);
    expect(makeFeedbackScopeChoice("current_candidate", "wrong_target", "classification")).toEqual({ scope: "current_candidate", reason: "wrong_target" });
    expect(makeFeedbackScopeChoice("current_image_class", "poor_boundary", "bounding_box")).toEqual({ scope: "current_image_class" });
    expect(makeFeedbackScopeChoice("project_future_rule", undefined, undefined)).toEqual({ scope: "project_future_rule" });
  });
  it("restores unsaved choices and an exact frozen command without creating any new command", () => {
    const local = { call_id: "TEST-call", context_digest: "TEST-digest", scope: "current_candidate", reason: "poor_boundary", frozen: input };
    expect(parseLocalScopeAnswer(JSON.stringify(local), "TEST-call")).toEqual(local);
    expect(parseLocalScopeAnswer(JSON.stringify({ ...local, frozen: undefined }), "TEST-call")).toEqual({ ...local, frozen: undefined });
    expect(parseLocalScopeAnswer(JSON.stringify({ call_id: "TEST-call", context_digest: "TEST-digest", scope: "current_candidate" }), "TEST-call")?.reason).toBeUndefined();
    expect(parseLocalScopeAnswer(JSON.stringify(local), "other-call")).toBeUndefined();
    expect(parseLocalScopeAnswer("invalid", "TEST-call")).toBeUndefined();
  });
  it("does not accept malformed local scopes, commands or changed frozen references", () => {
    const local = { call_id: "TEST-call", context_digest: "TEST-digest", scope: "current_candidate", reason: "poor_boundary", frozen: input };
    expect(parseLocalScopeAnswer(JSON.stringify({ ...local, scope: "delete_everything" }), "TEST-call")).toBeUndefined();
    expect(parseLocalScopeAnswer(JSON.stringify({ ...local, frozen: { ...input, expected_context_digest: "changed" } }), "TEST-call")).toBeUndefined();
    expect(parseLocalScopeAnswer(JSON.stringify({ ...local, frozen: { ...input, command_id: "" } }), "TEST-call")).toBeUndefined();
    expect(parseLocalScopeAnswer(JSON.stringify({ ...local, frozen: { ...input, choice: { scope: "current_candidate" } } }), "TEST-call")).toBeUndefined();
    expect(parseLocalScopeAnswer(JSON.stringify({ ...local, frozen: { ...input, choice: { scope: "current_candidate", reason: "poor_boundary", coordinates: [1, 2, 3, 4] } } }), "TEST-call")).toBeUndefined();
  });
  it("identifies same-command replay and a conflicting answer without replacing the user selection", () => {
    expect(sameScopeAnswerInput(input, { ...input, choice: { reason: "poor_boundary", scope: "current_candidate" } })).toBe(true);
    expect(sameScopeAnswerInput(input, { ...input, command_id: "other-command" })).toBe(false);
    const local = { call_id: "TEST-call", context_digest: "TEST-digest", scope: "current_candidate" as const, reason: "poor_boundary" as const, frozen: input };
    expect(scopeAnswerConflicts(local, answer)).toBe(false);
    expect(scopeAnswerConflicts(local, { ...answer, input: { ...input, choice: { scope: "project_future_rule" } } })).toBe(true);
    expect(scopeAnswerConflicts({ ...local, frozen: undefined }, answer)).toBe(false);
    expect(scopeAnswerConflicts({ ...local, frozen: undefined, scope: "current_image_class" }, answer)).toBe(true);
  });
  it("does not erase or replace a saved immutable answer when a late status arrives", () => {
    const status = { authorization: { consent: { call_id: "TEST-call" } }, receipt: { status: "completed" }, cancelled: false, scope_context_digest: "TEST-digest", scope_answer: answer } as FeedbackStatus;
    expect(mergeFeedbackStatus(status, { ...status, scope_answer: null }).scope_answer).toEqual(answer);
    expect(mergeFeedbackStatus(status, { ...status, scope_answer: { ...answer, input: { ...input, choice: { scope: "project_future_rule" } } } }).scope_answer).toEqual(answer);
    const cancelled = mergeFeedbackStatus(status, { ...status, scope_answer: null, cancelled: true });
    expect(cancelled.scope_answer).toEqual(answer); expect(cancelled.cancelled).toBe(true);
  });
});
