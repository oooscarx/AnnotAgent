import { request } from "./api";
import type { HumanRequest } from "./conversation-human-api";
import type { ConversationCallReceipt, ConversationMessage, ProjectCallLimitSnapshot } from "./types";

export type FeedbackConsent = {
  call_id: string; message_id: string; model_id: string; previous_grant_id: string | null;
  scope_hash: string; expires_at: string; allow_unknown_cost: boolean;
};
export type FeedbackDecision =
  | { decision: "request_correction"; reason: "poor_boundary" | "wrong_label" | "wrong_target"; question: string; rationale: string }
  | { decision: "clarify_scope"; question: string; rationale: string };
export type FeedbackCorrectionReason = "poor_boundary" | "wrong_label" | "wrong_target";
export type FeedbackScopeChoice =
  | { scope: "current_candidate"; reason: FeedbackCorrectionReason }
  | { scope: "current_image_class" }
  | { scope: "project_future_rule" };
export type ScopeAnswerInput = { command_id: string; expected_context_digest: string; choice: FeedbackScopeChoice };
export type ScopeAnswerRecord = { call_id: string; task_id: string; conversation_id: string; input: ScopeAnswerInput; created_at: string };
export type FeedbackStatus = {
  authorization: {
    consent: FeedbackConsent;
    context: { message: ConversationMessage; candidate: unknown; expected_feedback_sequence: number; sample_content_hash: string; pixels_supplied: false };
    grant: { id: string; task_id: string; scope_hash: string; maximum_calls: number; expires_at: string };
    summary: { model_name: string; remote_model: string; destination: string; data_scope: string; operation: string; maximum_output_tokens: number };
  };
  receipt: ConversationCallReceipt | null;
  decision: { Ok: FeedbackDecision } | { Err: string } | null;
  cancelled: boolean;
  error: string | null;
  scope_context_digest: string;
  scope_answer: ScopeAnswerRecord | null;
};
export type FeedbackPreview = {
  consent: FeedbackConsent; model_name: string; remote_model: string; destination: string;
  maximum_calls: 1; cumulative_maximum_calls: number; used_calls: number; image_count: 0;
  estimated_cost: null; data_scope: string; operation: string; project_call_limit?: ProjectCallLimitSnapshot;
};

const root = (project: string, conversation: string, task: string) =>
  `/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}`;
export const feedbackApi = {
  preview: (project: string, conversation: string, task: string, message: string, call: string, model?: string) => {
    const query = new URLSearchParams({ message_id: message, call_id: call });
    if (model) query.set("model_id", model);
    return request<FeedbackPreview>(`${root(project, conversation, task)}/feedback-preview?${query}`);
  },
  forMessage: (project: string, conversation: string, task: string, message: string, signal?: AbortSignal) =>
    request<FeedbackStatus | null>(`${root(project, conversation, task)}/feedback?${new URLSearchParams({ message_id: message })}`, { signal }),
  status: (project: string, conversation: string, task: string, call: string, signal?: AbortSignal) =>
    request<FeedbackStatus>(`${root(project, conversation, task)}/feedback/${encodeURIComponent(call)}`, { signal }),
  authorize: (project: string, conversation: string, task: string, consent: FeedbackConsent) =>
    request<FeedbackStatus>(`${root(project, conversation, task)}/feedback-authorizations`, { method: "POST", body: JSON.stringify(consent) }),
  execute: (project: string, conversation: string, task: string, call: string) =>
    request<FeedbackStatus>(`${root(project, conversation, task)}/feedback/${encodeURIComponent(call)}/execute`, { method: "POST", body: "{}" }),
  prepareCorrection: (project: string, conversation: string, task: string, call: string) =>
    request<HumanRequest | null>(`${root(project, conversation, task)}/feedback/${encodeURIComponent(call)}/human-request`, { method: "POST", body: "{}" }),
  answerScope: (project: string, conversation: string, task: string, call: string, input: ScopeAnswerInput) =>
    request<ScopeAnswerRecord>(`${root(project, conversation, task)}/feedback/${encodeURIComponent(call)}/scope-answer`, { method: "POST", body: JSON.stringify(input) }),
};
