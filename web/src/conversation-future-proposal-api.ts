import { request } from "./api";
import type { FeedbackConsent, FeedbackPreview, FeedbackStatus } from "./conversation-feedback-api";
import type { FutureSchemaDecision } from "./conversation-future-schema-api";
import type { ConversationCallReceipt, ConversationSchemaDraft } from "./types";

export type FutureProposalSource = { feedback_call_id: string; scope_answer_command_id: string; context_digest: string; base_schema_id: string; base_schema_revision: number };
export type FutureRuleProposal = { goal: string; decision: FutureSchemaDecision | { decision: "clarify"; question: string; rationale: string } };
export type FutureProposalPreview = FeedbackPreview & { source: FutureProposalSource; maximum_output_tokens: number };
export type FutureProposalStatus = {
  authorization: {
    consent: FeedbackConsent; grant: FeedbackStatus["authorization"]["grant"]; source: FutureProposalSource;
    context: { source: FutureProposalSource; base_schema: ConversationSchemaDraft; feedback: unknown; scope: "future_tasks_only" };
    summary: FeedbackStatus["authorization"]["summary"];
  };
  receipt: ConversationCallReceipt | null; proposal: { Ok: FutureRuleProposal } | { Err: string } | null;
  proposal_digest: string | null; cancelled: boolean; error: string | null;
};
export type PendingFutureProposal = { consent: FeedbackConsent; source: FutureProposalSource; summary: FeedbackStatus["authorization"]["summary"] };
const root = (project: string, conversation: string, task: string, feedback: string) => `/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/feedback/${encodeURIComponent(feedback)}/future-schema`;
export const futureProposalApi = {
  preview: (project: string, conversation: string, task: string, feedback: string, call: string, model?: string) => request<FutureProposalPreview>(`${root(project, conversation, task, feedback)}/proposal-preview?${new URLSearchParams({ call_id: call, ...(model ? { model_id: model } : {}) })}`),
  read: (project: string, conversation: string, task: string, feedback: string, signal?: AbortSignal) => request<FutureProposalStatus | null>(`${root(project, conversation, task, feedback)}/proposal`, { signal }),
  authorize: (project: string, conversation: string, task: string, feedback: string, consent: FeedbackConsent) => request<FutureProposalStatus>(`${root(project, conversation, task, feedback)}/proposal`, { method: "POST", body: JSON.stringify(consent) }),
  execute: (project: string, conversation: string, task: string, feedback: string, call: string) => request<FutureProposalStatus>(`${root(project, conversation, task, feedback)}/proposal/execute`, { method: "POST", body: JSON.stringify({ call_id: call }) }),
};
