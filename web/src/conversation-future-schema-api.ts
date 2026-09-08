import { request } from "./api";
import type { ConversationSchemaDraft } from "./types";

export type FutureSchemaDecision = Pick<ConversationSchemaDraft["definition"]["task"], "kind" | "labels" | "multi_label" | "attributes"> & { decision: "draft"; boundary_rules: string[]; rationale: string };
export type FutureSchemaInput = {
  command_id: string; expected_scope_answer_command_id: string; expected_context_digest: string;
  base_schema_id: string; base_schema_revision: number; goal: string; decision: FutureSchemaDecision;
  proposal_call_id?: string; proposal_digest?: string;
};
export type SavedFutureSchemaInput = {
  command_id: string; feedback_call_id: string; scope_answer_command_id: string; context_digest: string;
  base_schema_id: string; base_schema_revision: number; definition: ConversationSchemaDraft["definition"];
  proposal_call_id?: string | null; proposal_digest?: string | null;
};
export type FutureSchemaState = {
  record: { input: SavedFutureSchemaInput; schema_id: string; created_at: string } | null;
  base_schema: ConversationSchemaDraft; schema: ConversationSchemaDraft | null;
  source: { scope_answer_command_id: string; context_digest: string }; scope: "future_tasks_only";
};
const path = (project: string, conversation: string, task: string, call: string) => `/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/feedback/${encodeURIComponent(call)}/future-schema`;
export const futureSchemaApi = {
  read: (project: string, conversation: string, task: string, call: string, signal?: AbortSignal) => request<FutureSchemaState>(path(project, conversation, task, call), { signal }),
  save: (project: string, conversation: string, task: string, call: string, input: FutureSchemaInput) => request<FutureSchemaState>(path(project, conversation, task, call), { method: "POST", body: JSON.stringify(input) }),
};
