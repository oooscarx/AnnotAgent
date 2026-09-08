import { request } from "./api";
import type { FeedbackStatus, ScopeAnswerRecord } from "./conversation-feedback-api";
import type { AnnotationValue, ConversationCallReceipt, FinalCandidateProjection, SampleFeedbackRevision } from "./types";

export type ImageClassScope = {
  feedback_call_id: string; scope_answer: ScopeAnswerRecord; feedback_receipt: ConversationCallReceipt;
  feedback_context: FeedbackStatus["authorization"]["context"];
  sample_test_id: string; draft_id: string; draft_revision: number; draft_content_hash: string;
  image_id: string; content_hash: string; baseline_sequence: number; baseline_feedback: SampleFeedbackRevision[];
  kind: "classification" | "bounding_box"; target_label: string; members: FinalCandidateProjection[];
};
export type ImageClassPreview = { scope: ImageClassScope; scope_digest: string };
export type ImageClassCreateInput = { id: string; feedback_call_id: string; target_label?: string | null; expected_scope_digest: string };
type Subject = { outcome_id: string; source_artifact_id: string };
export type ImageClassAction = Subject & (
  | { action: "keep" | "exclude" }
  | { action: "edit"; corrected_value: AnnotationValue; corrected_label: string }
  | { action: "replace_label"; replacement: string }
);
export type ImageClassAnswerInput = { command_id: string; expected_scope_digest: string; actions: ImageClassAction[] };
export type ImageClassReview = {
  id: string; task_id: string; conversation_id: string; input: ImageClassCreateInput; scope: ImageClassScope; scope_digest: string;
  status: "pending" | "answered" | "applied" | "cancelled"; answer: ImageClassAnswerInput | null; revisions: SampleFeedbackRevision[];
  resume_checkpoint_ref: string; repair_draft_id: string | null; resume_error?: string | null; created_at: string;
};
const root = (project: string, conversation: string, task: string) => `/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}`;
const reviewRoot = (project: string, conversation: string, task: string, review: string) => `${root(project, conversation, task)}/image-class-reviews/${encodeURIComponent(review)}`;
export const imageClassApi = {
  preview: (project: string, conversation: string, task: string, call: string, token?: string, signal?: AbortSignal) => request<ImageClassPreview>(`${root(project, conversation, task)}/feedback/${encodeURIComponent(call)}/image-class-preview${token === undefined ? "" : `?${new URLSearchParams({ target_label: token })}`}`, { signal }),
  forFeedback: (project: string, conversation: string, task: string, call: string, signal?: AbortSignal) => request<ImageClassReview | null>(`${root(project, conversation, task)}/feedback/${encodeURIComponent(call)}/image-class`, { signal }),
  create: (project: string, conversation: string, task: string, call: string, input: ImageClassCreateInput) => request<ImageClassReview>(`${root(project, conversation, task)}/feedback/${encodeURIComponent(call)}/image-class`, { method: "POST", body: JSON.stringify(input) }),
  get: (project: string, conversation: string, task: string, review: string, signal?: AbortSignal) => request<ImageClassReview>(reviewRoot(project, conversation, task, review), { signal }),
  answer: (project: string, conversation: string, task: string, review: string, input: ImageClassAnswerInput) => request<ImageClassReview>(`${reviewRoot(project, conversation, task, review)}/answer`, { method: "POST", body: JSON.stringify(input) }),
  resume: (project: string, conversation: string, task: string, review: string) => request<ImageClassReview>(`${reviewRoot(project, conversation, task, review)}/resume`, { method: "POST", body: "{}" }),
  cancel: (project: string, conversation: string, task: string, review: string) => request<ImageClassReview>(`${reviewRoot(project, conversation, task, review)}/cancel`, { method: "POST", body: "{}" }),
};
