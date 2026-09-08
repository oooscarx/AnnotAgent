import { describe, expect, it } from "vitest";
import { futureProposalPhase, futureProposalNeedsPolling, mergeFutureProposal, parseFutureProposalPending, futureProposalSourceMatches, futureProposalDefinition } from "./conversation-future-proposal";
import type { FutureProposalStatus } from "./conversation-future-proposal-api";

const source = { feedback_call_id: "TEST-feedback", scope_answer_command_id: "TEST-answer", context_digest: "TEST-context", base_schema_id: "TEST-base", base_schema_revision: 1 };
const base = { id: "TEST-base", task_id: "TEST-task", source_call_id: null, source_request_id: "TEST-request", base_schema_revision: "TEST-project", revision: 1, definition: { goal: "Find cups", task: { id: "TEST-config", kind: "bounding_box" as const, labels: ["cup"], multi_label: false, attributes: {} }, boundary_rules: [] } };
const consent = { call_id: "TEST-proposal", message_id: "TEST-message", model_id: "TEST-model", previous_grant_id: "TEST-prior", scope_hash: "TEST-scope", expires_at: "2099-01-01T00:00:00Z", allow_unknown_cost: true };
const status: FutureProposalStatus = { authorization: { consent, source, context: { source, base_schema: base, feedback: {}, scope: "future_tasks_only" }, grant: { id: consent.call_id, task_id: base.task_id, scope_hash: consent.scope_hash, maximum_calls: 1, expires_at: consent.expires_at }, summary: { model_name: "TEST model", remote_model: "TEST remote", destination: "TEST local transport", data_scope: "TEST text only", operation: "TEST future proposal only", maximum_output_tokens: 1000 } }, receipt: null, proposal: null, proposal_digest: null, cancelled: false, error: null };

describe("future-only model proposal identity and recovery", () => {
  it("never treats expiry as erasing an admitted call or cancellation", () => {
    expect(futureProposalPhase(status)).toBe("authorized");
    const expired = { ...status, authorization: { ...status.authorization, consent: { ...consent, expires_at: "2000-01-01T00:00:00Z" } } };
    expect(futureProposalPhase(expired)).toBe("expired");
    expect(futureProposalPhase({ ...expired, receipt: { id: consent.call_id, task_id: base.task_id, status: "in_doubt" } })).toBe("unknown");
    expect(futureProposalPhase({ ...status, cancelled: true })).toBe("cancelled");
    expect(futureProposalNeedsPolling(status, true)).toBe(true);
    expect(futureProposalNeedsPolling({ ...status, cancelled: true }, true)).toBe(false);
  });
  it("does not let late GET erase a proposal, cancellation or admitted result", () => {
    const completed: FutureProposalStatus = { ...status, receipt: { id: consent.call_id, task_id: base.task_id, status: "completed" }, proposal: { Ok: { goal: "Find only cups", decision: { decision: "draft", kind: "bounding_box", labels: ["cup"], multi_label: false, attributes: {}, boundary_rules: ["Exclude bottles"], rationale: "TEST reasoning" } } }, proposal_digest: "TEST-digest", cancelled: true };
    expect(mergeFutureProposal(completed, status)).toEqual(completed);
    expect(futureProposalPhase(completed)).toBe("cancelled");
  });
  it("restores an exact pending authorization without replacing its source or call", () => {
    const pending = { consent, source, summary: status.authorization.summary };
    expect(parseFutureProposalPending(JSON.stringify(pending), source.feedback_call_id, source.scope_answer_command_id)).toEqual(pending);
    expect(parseFutureProposalPending(JSON.stringify(pending), "OTHER-feedback", source.scope_answer_command_id)).toBeUndefined();
    expect(parseFutureProposalPending(JSON.stringify({ ...pending, consent: { ...consent, allow_unknown_cost: false } }), source.feedback_call_id, source.scope_answer_command_id)).toBeUndefined();
    expect(futureProposalSourceMatches(source, { ...source, base_schema_id: "OTHER-rev1" })).toBe(false);
    expect(futureProposalSourceMatches(source, { ...source, base_schema_revision: 2 })).toBe(false);
  });
  it("preserves bounded model semantics and task identity, including attributes", () => {
    const proposal = { goal: "Find only cups", decision: { decision: "draft" as const, kind: "bounding_box" as const, labels: ["cup"], multi_label: false, attributes: {}, boundary_rules: ["Exclude bottles"], rationale: "TEST rationale" } };
    expect(futureProposalDefinition(base, proposal)).toEqual({ goal: "Find only cups", task: base.definition.task, boundary_rules: ["Exclude bottles"] });
    expect(futureProposalDefinition(base, { ...proposal, decision: { ...proposal.decision, multi_label: true } }).task.multi_label).toBe(true);
    const attributes = { color: { type: "enum", required: false, values: ["yellow"] } };
    expect(futureProposalDefinition(base, { ...proposal, decision: { ...proposal.decision, attributes } }).task.attributes).toEqual(attributes);
    expect(base.definition.boundary_rules).toEqual([]);
  });
});
