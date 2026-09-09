import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FeedbackScope, taskFeedbackService } from "./agent-ui/TaskFeedback";
import type { FeedbackStatus, ScopeAnswerRecord } from "./conversation-feedback-api";

const answer: ScopeAnswerRecord = { call_id: "TEST-call", task_id: "TEST-task", conversation_id: "TEST-conversation", created_at: "2026-09-08T00:00:00Z", input: { command_id: "TEST-command", expected_context_digest: "TEST-digest", choice: { scope: "current_candidate", reason: "poor_boundary" } } };
const value: FeedbackStatus = {
  authorization: {
    consent: { call_id: "TEST-call", message_id: "TEST-message", model_id: "TEST-model", previous_grant_id: null, scope_hash: "TEST-scope", expires_at: "2030-01-01T00:00:00Z", allow_unknown_cost: true },
    context: { message: { conversation_id: "TEST-conversation", sequence: 1, input: { id: "TEST-message", text: "TEST feedback", image: null } }, candidate: { outcome: { value: { kind: "bounding_box" } } }, expected_feedback_sequence: 0, sample_content_hash: "TEST-hash", pixels_supplied: false },
    grant: { id: "TEST-call", task_id: "TEST-task", scope_hash: "TEST-scope", maximum_calls: 1, expires_at: "2030-01-01T00:00:00Z" },
    summary: { model_name: "TEST model", remote_model: "TEST", destination: "TEST local", data_scope: "TEST metadata", operation: "Interpret", maximum_output_tokens: 100 },
  },
  receipt: { id: "TEST-call", task_id: "TEST-task", status: "completed" },
  decision: { Ok: { decision: "clarify_scope", question: "TEST scope?", rationale: "TEST text" } },
  cancelled: false, error: null, scope_context_digest: "TEST-digest", scope_answer: answer,
};

describe("saved scope answer presentation", () => {
  it("does not tell the user to execute a correction after that feedback action was cancelled", () => {
    const html = renderToStaticMarkup(createElement(FeedbackScope, { project: "TEST-project", value: { ...value, cancelled: true }, conversation:"TEST-conversation",task:"TEST-task",workspace:"TEST-workspace",service:taskFeedbackService }));
    expect(html).toContain("仅保存修改范围");
    expect(html).toContain("已取消，此范围只读");
    expect(html).not.toContain("返回图片面板提供实际修正");
    expect(html).not.toContain("保存反馈范围");
  });
  it("keeps actionable guidance for an uncancelled candidate answer", () => {
    const html = renderToStaticMarkup(createElement(FeedbackScope, { project: "TEST-project", value, conversation:"TEST-conversation",task:"TEST-task",workspace:"TEST-workspace",service:taskFeedbackService }));
    expect(html).toContain("返回图片面板提供实际修正");
  });
});
