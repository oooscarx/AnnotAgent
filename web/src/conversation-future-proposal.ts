import type { FutureProposalSource, FutureProposalStatus, FutureRuleProposal, PendingFutureProposal } from "./conversation-future-proposal-api";
import { parsePendingFeedback } from "./conversation-feedback";
import type { ConversationSchemaDraft } from "./types";

export function futureProposalPhase(value?: FutureProposalStatus, now = Date.now()) {
  if (!value) return "unsaved";
  if (value.cancelled) return "cancelled";
  if (value.receipt?.status === "reserved") return "running";
  if (value.receipt?.status === "in_doubt") return "unknown";
  if (value.receipt?.status === "failed") return "failed";
  if (value.receipt?.status === "completed") {
    if (value.proposal && "Ok" in value.proposal && value.proposal_digest) return value.proposal.Ok.decision.decision === "draft" ? "draft" : "clarify";
    return "invalid";
  }
  const expiry = Date.parse(value.authorization.consent.expires_at);
  return Number.isFinite(expiry) && expiry > now ? "authorized" : "expired";
}
export function futureProposalNeedsPolling(value: FutureProposalStatus | undefined, dispatchPending: boolean) {
  return Boolean(value && !value.cancelled && (value.receipt ? value.receipt.status === "reserved" : dispatchPending));
}
export function mergeFutureProposal(current: FutureProposalStatus | undefined, incoming: FutureProposalStatus): FutureProposalStatus {
  if (!current) return incoming;
  if (current.authorization.consent.call_id !== incoming.authorization.consent.call_id) return current;
  const terminal = current.receipt && current.receipt.status !== "reserved";
  const retained = (current.receipt && !incoming.receipt) || (terminal && incoming.receipt?.status === "reserved") || (current.proposal && !incoming.proposal) ? current : incoming;
  return { ...retained, cancelled: current.cancelled || incoming.cancelled };
}
export function futureProposalSourceMatches(a: FutureProposalSource, b: FutureProposalSource) {
  return a.feedback_call_id === b.feedback_call_id && a.scope_answer_command_id === b.scope_answer_command_id && a.context_digest === b.context_digest && a.base_schema_id === b.base_schema_id && a.base_schema_revision === b.base_schema_revision;
}
export function parseFutureProposalPending(raw: string | null, feedback: string, answer: string): PendingFutureProposal | undefined {
  if (!raw) return undefined;
  try {
    const value = JSON.parse(raw) as PendingFutureProposal;
    const source = value.source, summary = value.summary;
    if (!source || source.feedback_call_id !== feedback || source.scope_answer_command_id !== answer || typeof source.context_digest !== "string" || !source.context_digest || typeof source.base_schema_id !== "string" || !source.base_schema_id || !Number.isInteger(source.base_schema_revision) || source.base_schema_revision < 1) return undefined;
    if (!summary || ![summary.model_name, summary.remote_model, summary.destination, summary.data_scope, summary.operation].every(item => typeof item === "string") || !Number.isInteger(summary.maximum_output_tokens)) return undefined;
    if (!value.consent || !parsePendingFeedback(JSON.stringify(value.consent), value.consent.message_id)) return undefined;
    return value;
  } catch { return undefined; }
}
export function futureProposalDefinition(base: ConversationSchemaDraft, proposal: FutureRuleProposal): ConversationSchemaDraft["definition"] {
  const decision = proposal.decision;
  if (decision.decision !== "draft" || !proposal.goal.trim() || !decision.labels.length) throw new Error("The proposal is incomplete. Review its saved evidence and define the future rules manually.");
  return { ...base.definition, goal: proposal.goal, task: { ...base.definition.task, kind: decision.kind, labels: [...decision.labels], multi_label: decision.multi_label, attributes: structuredClone(decision.attributes) }, boundary_rules: [...decision.boundary_rules] };
}
