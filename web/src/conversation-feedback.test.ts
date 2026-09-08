import { describe, expect, it } from "vitest";
import { feedbackCancellationMatches, feedbackCanDiscardUnaccepted, feedbackNavigationStillCurrent, feedbackNeedsPolling, feedbackPhase, feedbackWaitingRequest, mergeFeedbackStatus, parsePendingFeedback } from "./conversation-feedback";
import type { FeedbackStatus } from "./conversation-feedback-api";
import type { HumanRequest } from "./conversation-human-api";
import type { ConversationMessage } from "./types";

const message: ConversationMessage = { conversation_id: "conversation", sequence: 5, input: { id: "message", text: "TEST: the box is too big", image: { image_id: "image", sha256: "pixels" }, reference: { scope: "sample_candidate", task_id: "task", project_schema_revision: "schema", draft_id: "draft", draft_revision: 1, sample_test_id: "sample", candidate_id: "candidate", source_artifact_id: "artifact" } } };
const consent = { call_id: "call", message_id: "message", model_id: "model", previous_grant_id: "old-grant", scope_hash: "scope", expires_at: "2030-01-01T00:00:00Z", allow_unknown_cost: true };
const saved: FeedbackStatus = { authorization: { consent, context: { message, candidate: {}, expected_feedback_sequence: 0, sample_content_hash: "sample-hash", pixels_supplied: false }, grant: { id: "grant", task_id: "task", scope_hash: "scope", maximum_calls: 4, expires_at: consent.expires_at }, summary: { model_name: "TEST model", remote_model: "TEST", destination: "TEST server", data_scope: "Frozen message and metadata", operation: "Interpret feedback", maximum_output_tokens: 2048 } }, receipt: null, decision: null, cancelled: false, error: null, scope_context_digest: "TEST-digest", scope_answer: null };
const request: HumanRequest = { input: { id: "human", task_id: "task", conversation_id: "conversation", sample_test_id: "sample", image_id: "image", content_hash: "pixels", outcome_id: "candidate", expected_feedback_sequence: 0, reason_code: "poor_boundary", question: "TEST correction", resume_checkpoint_ref: "repair" }, status: "pending" };

describe("candidate feedback presentation follows server state", () => {
  it("releases only definitively rejected unsaved authorization, never an unknown or admitted action", () => {
    const rejected = { httpStatus: 400, authorizationAcknowledged: false, lookupConfirmedEmpty: true };
    expect(feedbackCanDiscardUnaccepted(rejected)).toBe(true);
    expect(feedbackCanDiscardUnaccepted({ ...rejected, httpStatus: 409 })).toBe(true);
    expect(feedbackCanDiscardUnaccepted({ ...rejected, authorizationAcknowledged: true })).toBe(false);
    expect(feedbackCanDiscardUnaccepted({ ...rejected, lookupConfirmedEmpty: false })).toBe(false);
    for (const httpStatus of [undefined, 408, 500, 502, 504]) expect(feedbackCanDiscardUnaccepted({ ...rejected, httpStatus })).toBe(false);
  });
  it("permits a prepared canvas jump only for the original click context", () => {
    expect(feedbackNavigationStillCurrent(3, 3, true)).toBe(true);
    expect(feedbackNavigationStillCurrent(3, 4, true)).toBe(false);
    expect(feedbackNavigationStillCurrent(3, 3, false)).toBe(false);
  });
  it("restores an early real cancellation only for the original task and call", () => {
    const receipt = { task_id: "task", call_id: "call", requested_at: "2026-09-08T08:00:00Z" };
    expect(feedbackCancellationMatches(receipt, "task", "call")).toBe(true);
    expect(feedbackCancellationMatches(receipt, "other-task", "call")).toBe(false);
    expect(feedbackCancellationMatches(receipt, "task", "other-call")).toBe(false);
    expect(feedbackCancellationMatches(receipt, "task", undefined)).toBe(false);
    expect(feedbackCancellationMatches(undefined, "task", "call")).toBe(false);
    // A tombstone is not synthesized into an authorized or completed feedback status.
    expect(feedbackPhase(undefined)).toBe("unsaved");
  });
  it("restores authorization without treating it as admitted inference", () => {
    expect(feedbackPhase()).toBe("unsaved");
    expect(feedbackPhase(saved, 0)).toBe("authorized");
    expect(feedbackPhase(saved, Date.parse("2031-01-01"))).toBe("expired");
    expect(feedbackPhase({ ...saved, authorization: { ...saved.authorization, consent: { ...consent, expires_at: "not a date" } } })).toBe("expired");
  });
  it("polls explicit submitted work before a receipt exists, never newly restored authorization", () => {
    expect(feedbackNeedsPolling(saved, false)).toBe(false);
    expect(feedbackNeedsPolling(saved, true)).toBe(true);
    expect(feedbackNeedsPolling({ ...saved, receipt: { id: "call", task_id: "task", status: "reserved" } }, false)).toBe(true);
    expect(feedbackNeedsPolling({ ...saved, cancelled: true }, true)).toBe(false);
    expect(feedbackNeedsPolling({ ...saved, receipt: { id: "call", task_id: "task", status: "completed" } }, true)).toBe(false);
  });
  it("does not let a slow same-call snapshot erase a completed receipt or cancellation", () => {
    const completed: FeedbackStatus = { ...saved, receipt: { id: "call", task_id: "task", status: "completed" }, decision: { Ok: { decision: "clarify_scope", question: "TEST scope?", rationale: "TEST" } } };
    expect(mergeFeedbackStatus(completed, saved)).toEqual(completed);
    expect(mergeFeedbackStatus(completed, { ...saved, receipt: { id: "call", task_id: "task", status: "reserved" } })).toEqual(completed);
    expect(mergeFeedbackStatus({ ...completed, cancelled: true }, completed).cancelled).toBe(true);
    expect(mergeFeedbackStatus({ ...saved, cancelled: true }, completed)).toEqual({ ...completed, cancelled: true });
    expect(mergeFeedbackStatus(completed, { ...saved, cancelled: true })).toEqual({ ...completed, cancelled: true });
    const reserved: FeedbackStatus = { ...saved, receipt: { id: "call", task_id: "task", status: "reserved" } };
    expect(mergeFeedbackStatus(reserved, saved)).toEqual(reserved);
    const cancelledReplay = mergeFeedbackStatus({ ...reserved, cancelled: true }, completed);
    expect(cancelledReplay.receipt?.status).toBe("completed");
    expect(feedbackPhase(cancelledReplay)).toBe("cancelled");
    expect(feedbackNeedsPolling(cancelledReplay, true)).toBe(false);
    expect(mergeFeedbackStatus(cancelledReplay, reserved)).toEqual(cancelledReplay);
  });
  it("does not hide reserved or unknown outcomes behind expiry or pretend invalid output is success", () => {
    const expired = Date.parse("2031-01-01");
    const receipt = { id: "call", task_id: "task", status: "reserved" as const };
    expect(feedbackPhase({ ...saved, receipt }, expired)).toBe("running");
    expect(feedbackPhase({ ...saved, receipt: { ...receipt, status: "in_doubt" } }, expired)).toBe("unknown");
    expect(feedbackPhase({ ...saved, receipt: { ...receipt, status: "completed" }, decision: { Err: "TEST invalid output" } }, expired)).toBe("invalid");
    expect(feedbackPhase({ ...saved, receipt: { ...receipt, status: "failed" } }, expired)).toBe("failed");
  });
  it("only a valid saved completed proposal offers correction or scope clarification", () => {
    const receipt = { id: "call", task_id: "task", status: "completed" as const };
    const question = { question: "TEST question", rationale: "Saved text only" };
    const correction = { ...saved, receipt, decision: { Ok: { ...question, decision: "request_correction" as const, reason: "poor_boundary" as const } } };
    expect(feedbackPhase(correction, 0)).toBe("correction");
    expect(feedbackPhase({ ...correction, cancelled: true }, 0)).toBe("cancelled");
    expect(feedbackPhase({ ...saved, receipt, decision: { Ok: { ...question, decision: "clarify_scope" } } }, 0)).toBe("clarify");
  });
  it("prioritizes an exact frozen candidate request, including deferred, instead of another call", () => {
    const other = { ...request, input: { ...request.input, id: "other-request", outcome_id: "other-candidate" } };
    const deferred = { ...request, deferred: true };
    expect(feedbackWaitingRequest(message, [other, deferred])).toEqual({ request: deferred, sameSubject: true });
    expect(feedbackWaitingRequest(message, [other])).toEqual({ request: other, sameSubject: false });
    expect(feedbackWaitingRequest(message, [{ ...request, status: "applied" }])).toBeUndefined();
  });
  it("does not mistake another task, conversation or changed pixels for the message subject", () => {
    expect(feedbackWaitingRequest(message, [{ ...request, input: { ...request.input, task_id: "other" } }])).toBeUndefined();
    expect(feedbackWaitingRequest(message, [{ ...request, input: { ...request.input, conversation_id: "other" } }])).toBeUndefined();
    expect(feedbackWaitingRequest(message, [{ ...request, input: { ...request.input, content_hash: "changed" } }])?.sameSubject).toBe(false);
  });
  it("restores only a well-shaped locally confirmed original envelope; it never invents consent", () => {
    expect(parsePendingFeedback(JSON.stringify(consent), "message")).toEqual(consent);
    expect(parsePendingFeedback(JSON.stringify(consent), "other-message")).toBeUndefined();
    expect(parsePendingFeedback(JSON.stringify({ ...consent, allow_unknown_cost: false }), "message")).toBeUndefined();
    expect(parsePendingFeedback(JSON.stringify({ ...consent, call_id: null }), "message")).toBeUndefined();
    expect(parsePendingFeedback(JSON.stringify({ ...consent, expires_at: "invalid" }), "message")).toBeUndefined();
    expect(parsePendingFeedback("garbage", "message")).toBeUndefined();
    expect(parsePendingFeedback(null, "message")).toBeUndefined();
  });
});
