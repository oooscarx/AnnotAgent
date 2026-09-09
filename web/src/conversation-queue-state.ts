import type { SendCommand, SendReceipt } from "./conversation-send";

/** Persisted inbox state, independent of any old or native page component. */
export type QueuedMessage = {
  input: SendCommand;
  receipt: SendReceipt;
  status: "waiting_for_dispatch" | "authorized" | "running" | "completed" | "failed" | "in_doubt" | "cancelled";
  cancelled_at: string | null;
  planning_call_id: string | null;
};
export function canCancelQueuedMessage(status: QueuedMessage["status"]): boolean {
  return ["waiting_for_dispatch", "authorized", "in_doubt"].includes(status);
}
export function isPendingQueuedMessage(status: QueuedMessage["status"]): boolean {
  return status === "running" || canCancelQueuedMessage(status);
}
