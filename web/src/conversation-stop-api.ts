import { request } from "./api";
import type { ConversationMessage } from "./types";
import type { StopMessageInput } from "./conversation-control";

export type StopTargetRef = { kind: "call" | "builder" | "journey" | "sample" | "processing" | "authorization"; id: string; task_id: string };
export type StopTarget = StopTargetRef & { state: string; parent_journey_ids: string[] };
export type StopRequestRecord = {
  message: ConversationMessage; targets: StopTarget[]; selected_target: StopTargetRef | null;
  status: "no_active_work" | "needs_selection" | "cancel_requested" | "finished"; created_at: string;
  observation?: { state: string; description: string } | null; dispatch_error?: string | null;
};
const path = (project: string, conversation: string) => `/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/stop-requests`;
export const stopApi = {
  begin: (project: string, conversation: string, input: StopMessageInput) => request<StopRequestRecord>(path(project, conversation), { method: "POST", body: JSON.stringify(input) }),
  read: (project: string, conversation: string, message: string, signal?: AbortSignal) => request<StopRequestRecord | null>(`${path(project, conversation)}/${encodeURIComponent(message)}`, { signal }),
  select: (project: string, conversation: string, message: string, target: StopTargetRef) => request<StopRequestRecord>(`${path(project, conversation)}/${encodeURIComponent(message)}/select`, { method: "POST", body: JSON.stringify({ target }) }),
};
