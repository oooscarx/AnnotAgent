import type { FeedbackConsent, FeedbackStatus } from "./conversation-feedback-api";
import type { HumanRequest } from "./conversation-human-api";
import type { ConversationCallCancellation, ConversationMessage } from "./types";

/** Saved outcomes outrank expiry: an expired grant does not erase an admitted call. */
export function feedbackPhase(value?: FeedbackStatus, now = Date.now()) {
  if (!value) return "unsaved";
  if (value.cancelled) return "cancelled";
  if (value.receipt?.status === "reserved") return "running";
  if (value.receipt?.status === "in_doubt") return "unknown";
  if (value.receipt?.status === "failed") return "failed";
  if (value.receipt?.status === "completed") {
    if (value.decision && "Ok" in value.decision) return value.decision.Ok.decision === "request_correction" ? "correction" : "clarify";
    return "invalid";
  }
  const expiry = Date.parse(value.authorization.consent.expires_at);
  return !Number.isFinite(expiry) || expiry <= now ? "expired" : "authorized";
}

export function feedbackNeedsPolling(value: FeedbackStatus | undefined, dispatchPending: boolean) {
  if (!value || value.cancelled) return false;
  if (value.receipt) return value.receipt.status === "reserved";
  return dispatchPending;
}

/** A slow GET may describe this same call before a newer execute/stop acknowledgement. */
export function mergeFeedbackStatus(current: FeedbackStatus | undefined, incoming: FeedbackStatus): FeedbackStatus {
  if (!current || current.authorization.consent.call_id !== incoming.authorization.consent.call_id) return incoming;
  const terminal = current.receipt && current.receipt.status !== "reserved";
  const result = (current.receipt && !incoming.receipt) || (terminal && incoming.receipt?.status === "reserved") ? current : incoming;
  return { ...result, cancelled: current.cancelled || incoming.cancelled, scope_answer: current.scope_answer ?? incoming.scope_answer ?? null };
}

/** Only a definitive rejected authorization plus an empty server lookup frees a preview. */
export function feedbackCanDiscardUnaccepted(input: { httpStatus?: number; authorizationAcknowledged: boolean; lookupConfirmedEmpty: boolean }) {
  return !input.authorizationAcknowledged && input.lookupConfirmedEmpty && input.httpStatus !== undefined && [400, 401, 403, 404, 409, 410, 422, 429].includes(input.httpStatus);
}

/** Capture before asynchronous request preparation, never when the response arrives. */
export function feedbackNavigationStillCurrent(captured: number, current: number, alive: boolean) {
  return alive && captured === current;
}

/** A cancellation tombstone is real evidence even before authorization admission. */
export function feedbackCancellationMatches(value: ConversationCallCancellation | undefined, task: string, call?: string) {
  return Boolean(value && call && value.call_id === call && value.task_id === task);
}

/** This is a navigation hint only. Server admission still enforces all blockers. */
export function feedbackWaitingRequest(message: ConversationMessage, requests: HumanRequest[]) {
  const reference = message.input.reference;
  if (!reference || reference.scope !== "sample_candidate") return undefined;
  const waiting = requests.filter(value => value.status === "pending" && value.input.task_id === reference.task_id && value.input.conversation_id === message.conversation_id);
  const exact = waiting.find(value => value.input.sample_test_id === reference.sample_test_id && value.input.image_id === message.input.image?.image_id && value.input.content_hash === message.input.image.sha256 && value.input.outcome_id === reference.candidate_id);
  return exact ? { request: exact, sameSubject: true } : waiting[0] ? { request: waiting[0], sameSubject: false } : undefined;
}

/** Local recovery never creates permission: the server validates this original envelope. */
export function parsePendingFeedback(raw: string | null, message: string): FeedbackConsent | undefined {
  if (!raw) return undefined;
  try {
    const value = JSON.parse(raw) as FeedbackConsent;
    if (!value || value.message_id !== message || value.allow_unknown_cost !== true || ![value.call_id, value.model_id, value.scope_hash, value.expires_at].every(item => typeof item === "string" && item.length > 0) || !Number.isFinite(Date.parse(value.expires_at)) || !(value.previous_grant_id === null || typeof value.previous_grant_id === "string")) return undefined;
    return value;
  } catch { return undefined; }
}
