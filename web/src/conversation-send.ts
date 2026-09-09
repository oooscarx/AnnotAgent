import type { ConversationMessage, ConversationMessageInput } from "./types";
export type SendModel = {revision:number;model_profile_id:string|null};
export type SendMode = "plan" | "execute";
export type SendCommand = { message: ConversationMessageInput; task_id: string | null; schema_revision: string; agent_model?:SendModel; mode?:SendMode };
export type SendReceipt = { message: ConversationMessage; task_id: string; disposition: "new_task" | "task_message" | "candidate_feedback"; agent_model?:SendModel; mode?:SendMode };
export type PendingSend = { conversation: string; input: SendCommand };
const uuid = (value: unknown): value is string => typeof value === "string" && /^[a-f\d]{8}(-[a-f\d]{4}){3}-[a-f\d]{12}$/i.test(value);
export function sameSendCommand(a: SendCommand, b: SendCommand): boolean {
  const stable = (value: unknown): string => value && typeof value === "object" ? `{${Object.entries(value).filter(([,v])=>v!==undefined).sort(([a],[b])=>a.localeCompare(b)).map(([k,v])=>`${JSON.stringify(k)}:${stable(v)}`).join(",")}}` : JSON.stringify(value);
  const normalize = (value: SendCommand) => ({...value,message:{...value.message,reference:value.message.reference ?? null}});
  return stable(normalize(a)) === stable(normalize(b));
}
export function parsePendingSend(raw: string | null): PendingSend | undefined {
  if (!raw || raw.length > 100_000) return;
  try {
    const value = JSON.parse(raw);
    const input = value?.input, message = input?.message;
    if (!uuid(value?.conversation) || !uuid(message?.id) || typeof message.text !== "string" || !message.text.trim() || message.text.length > 65_536 || (input.task_id !== null && !uuid(input.task_id)) || typeof input.schema_revision !== "string" || !/^[a-f\d]{64}$/i.test(input.schema_revision)) return;
    if (message.image && (typeof message.image.image_id !== "string" || typeof message.image.sha256 !== "string")) return;
    if (message.reference && (message.reference.scope !== "sample_candidate" || message.reference.task_id !== input.task_id || message.reference.project_schema_revision !== input.schema_revision)) return;
    if (input.agent_model !== undefined && (!input.agent_model || !Number.isSafeInteger(input.agent_model.revision) || input.agent_model.revision < 0 || (input.agent_model.model_profile_id !== null && !uuid(input.agent_model.model_profile_id)))) return;
    if (input.mode !== undefined && input.mode !== "plan" && input.mode !== "execute") return;
    return value;
  } catch { return; }
}
