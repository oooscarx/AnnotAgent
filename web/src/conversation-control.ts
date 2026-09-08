import type { ConversationMessage, ConversationMessageInput } from "./types";
import type { StopRequestRecord, StopTargetRef } from "./conversation-stop-api";

export type StopMessageInput = { id: string; text: string; image: null; reference: { scope: "stop_request"; task_id: string | null } };
export type PendingStop = { conversation_id: string | null; input: StopMessageInput };
export type LocalStopSelection = { message_id: string; target: StopTargetRef; pending: boolean };
const object = (value: unknown): value is Record<string, unknown> => Boolean(value && typeof value === "object" && !Array.isArray(value));
const nonempty = (value: unknown): value is string => typeof value === "string" && value.length > 0;

/** Deliberately not natural-language intent inference: sentences remain messages. */
export function isStopCommand(text: string) {
  const command = text.trim();
  return command === "停止" || command.toLowerCase() === "stop";
}
export function composerIntent(text: string, prepareGoal: boolean): { kind: "stop" | "message"; prepareGoal: boolean } {
  return isStopCommand(text) ? { kind: "stop", prepareGoal: false } : { kind: "message", prepareGoal };
}
export function makeStopMessage(id: string, text: string, selectedTask: string | null): StopMessageInput {
  if (!isStopCommand(text)) throw new Error("A stop request must be a standalone stop or 停止 command");
  return { id, text, image: null, reference: { scope: "stop_request", task_id: selectedTask } };
}
export function isStopMessage(input: ConversationMessageInput): input is StopMessageInput { return input.reference?.scope === "stop_request" && input.image === null && isStopCommand(input.text); }
export function isAnnotationGoalMessage(message: ConversationMessage) { return !message.input.reference; }
export function parsePendingStop(raw: string | null): PendingStop | undefined {
  if (!raw) return undefined;
  try {
    const value: unknown = JSON.parse(raw);
    if (!object(value) || !(value.conversation_id === null || nonempty(value.conversation_id)) || !object(value.input)) return undefined;
    const input = value.input;
    if (!nonempty(input.id) || typeof input.text !== "string" || !isStopCommand(input.text) || input.image !== null || !object(input.reference) || input.reference.scope !== "stop_request" || !(input.reference.task_id === null || nonempty(input.reference.task_id))) return undefined;
    if (Object.keys(input).some(key => !["id", "text", "image", "reference"].includes(key)) || Object.keys(input.reference).some(key => !["scope", "task_id"].includes(key))) return undefined;
    return value as PendingStop;
  } catch { return undefined; }
}
export function stopTargetMatches(a: StopTargetRef, b: StopTargetRef) { return a.kind === b.kind && a.id === b.id && a.task_id === b.task_id; }
/** Display-only shortening. Selection and accessible details retain exact IDs. */
export function compactStopTarget(target: StopTargetRef, name?: string) {
  const title = (name ?? "").replace(/\s+/g, " ").trim();
  const characters = Array.from(title);
  return { title: characters.length > 72 ? `${characters.slice(0, 72).join("")}…` : title, operationId: target.id.slice(0, 8), taskId: target.task_id.slice(0, 8) };
}
export function parseStopSelection(raw: string | null, message: string): LocalStopSelection | undefined {
  if (!raw) return undefined;
  try {
    const value: unknown = JSON.parse(raw);
    if (!object(value) || value.message_id !== message || typeof value.pending !== "boolean" || !object(value.target)) return undefined;
    if (!nonempty(value.target.id) || !nonempty(value.target.task_id) || !["call", "builder", "journey", "sample", "processing", "authorization"].includes(String(value.target.kind)) || Object.keys(value.target).length !== 3) return undefined;
    return value as LocalStopSelection;
  } catch { return undefined; }
}
export function stopSelectionConflicts(local: LocalStopSelection | undefined, record: StopRequestRecord | undefined) { return Boolean(local && record?.selected_target && !stopTargetMatches(local.target, record.selected_target)); }
export function mergeStopRecord(previous: StopRequestRecord | undefined, incoming: StopRequestRecord): StopRequestRecord {
  if (previous?.selected_target && (!incoming.selected_target || !stopTargetMatches(previous.selected_target, incoming.selected_target))) return previous;
  if (previous && ["no_active_work", "finished"].includes(previous.status) && incoming.status === "needs_selection") return previous;
  if (previous?.observation && ["not_active", "cancelled", "finished", "unknown"].includes(previous.observation.state) && incoming.observation && ["running", "cancel_pending"].includes(incoming.observation.state)) return { ...incoming, observation: previous.observation };
  return incoming;
}

/** Journal messages are append-only. An older GET must not erase an acknowledged send. */
export function mergeConversationMessages(previous: ConversationMessage[], incoming: ConversationMessage[], conversation: string) {
  const messages = new Map<string, ConversationMessage>();
  for (const message of [...incoming, ...previous]) if (message.conversation_id === conversation) messages.set(message.input.id, message);
  return [...messages.values()].sort((a, b) => a.sequence - b.sequence || a.input.id.localeCompare(b.input.id));
}
