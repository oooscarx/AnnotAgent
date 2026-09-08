import type { FeedbackCorrectionReason, FeedbackScopeChoice, ScopeAnswerInput, ScopeAnswerRecord } from "./conversation-feedback-api";

export type FeedbackScope = FeedbackScopeChoice["scope"];
export type FeedbackCandidateKind = "bounding_box" | "classification";
export type LocalScopeAnswer = { call_id: string; context_digest: string; scope?: FeedbackScope; reason?: FeedbackCorrectionReason; frozen?: ScopeAnswerInput };
const object = (value: unknown): value is Record<string, unknown> => Boolean(value && typeof value === "object" && !Array.isArray(value));
const nonempty = (value: unknown): value is string => typeof value === "string" && value.length > 0;
const scopes = ["current_candidate", "current_image_class", "project_future_rule"];
const reasons = ["poor_boundary", "wrong_label", "wrong_target"];

export function feedbackScopeVisible(input: { isClarification: boolean; answer: ScopeAnswerRecord | null; cancelled: boolean; local: LocalScopeAnswer }) {
  return Boolean(input.answer) || (!input.cancelled && input.isClarification) || (input.cancelled && Boolean(input.local.scope || input.local.frozen));
}

export function candidateFeedbackKind(candidate: unknown): FeedbackCandidateKind | undefined {
  if (!object(candidate) || !object(candidate.outcome) || !object(candidate.outcome.value)) return undefined;
  const kind = candidate.outcome.value.kind;
  return kind === "bounding_box" || kind === "classification" ? kind : undefined;
}

export function makeFeedbackScopeChoice(scope: FeedbackScope | undefined, reason: FeedbackCorrectionReason | undefined, kind: FeedbackCandidateKind | undefined): FeedbackScopeChoice | undefined {
  if (scope === "current_image_class" || scope === "project_future_rule") return { scope };
  if (scope !== "current_candidate" || !kind || !reason || (reason === "poor_boundary" && kind !== "bounding_box")) return undefined;
  return { scope, reason };
}

function validChoice(value: unknown): value is FeedbackScopeChoice {
  if (!object(value)) return false;
  if (value.scope === "current_image_class" || value.scope === "project_future_rule") return Object.keys(value).length === 1;
  return value.scope === "current_candidate" && typeof value.reason === "string" && reasons.includes(value.reason) && Object.keys(value).length === 2;
}
function sameChoice(a: FeedbackScopeChoice, b: FeedbackScopeChoice) {
  return a.scope === b.scope && (a.scope !== "current_candidate" || (b.scope === "current_candidate" && a.reason === b.reason));
}
export function sameScopeAnswerInput(a: ScopeAnswerInput, b: ScopeAnswerInput) {
  return a.command_id === b.command_id && a.expected_context_digest === b.expected_context_digest && sameChoice(a.choice, b.choice);
}

export function parseLocalScopeAnswer(raw: string | null, call: string): LocalScopeAnswer | undefined {
  if (!raw) return undefined;
  try {
    const value: unknown = JSON.parse(raw);
    if (!object(value) || value.call_id !== call || !nonempty(value.context_digest) || Object.keys(value).some(key => !["call_id", "context_digest", "scope", "reason", "frozen"].includes(key))) return undefined;
    if (value.scope !== undefined && (typeof value.scope !== "string" || !scopes.includes(value.scope))) return undefined;
    if (value.reason !== undefined && (typeof value.reason !== "string" || !reasons.includes(value.reason))) return undefined;
    if (value.frozen !== undefined) {
      const frozen = value.frozen;
      if (!object(frozen) || Object.keys(frozen).length !== 3 || !nonempty(frozen.command_id) || frozen.expected_context_digest !== value.context_digest || !validChoice(frozen.choice)) return undefined;
      if (value.scope !== frozen.choice.scope || (frozen.choice.scope === "current_candidate" && value.reason !== frozen.choice.reason)) return undefined;
    }
    return value as LocalScopeAnswer;
  } catch { return undefined; }
}

export function scopeAnswerConflicts(local: LocalScopeAnswer, answer: ScopeAnswerRecord) {
  if (local.call_id !== answer.call_id || local.context_digest !== answer.input.expected_context_digest) return true;
  if (local.frozen) return !sameScopeAnswerInput(local.frozen, answer.input);
  if (!local.scope) return false;
  if (local.scope !== answer.input.choice.scope) return true;
  return local.scope === "current_candidate" && answer.input.choice.scope === "current_candidate" && local.reason !== undefined && local.reason !== answer.input.choice.reason;
}

export function feedbackScopeLabel(scope?: FeedbackScope) {
  return scope === "current_candidate" ? "Only this candidate" : scope === "current_image_class" ? "This class in the current image" : scope === "project_future_rule" ? "This rule for future project work" : "No scope selected";
}
export function feedbackReasonLabel(reason?: FeedbackCorrectionReason) {
  return reason === "poor_boundary" ? "Boundary is wrong" : reason === "wrong_label" ? "Label is wrong" : reason === "wrong_target" ? "This is the wrong target" : "No correction reason selected";
}
