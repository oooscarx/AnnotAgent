import type { AgentSession, WorkflowDraft } from "./types";

// A node is recovery-only if a required input can only come from recovery.
// An OR-merge fed by both the main and recovery paths remains on the main path.
export function recoveryNodeIds(draft: Pick<WorkflowDraft, "nodes" | "edges">): Set<string> {
  const edges = draft.edges ?? [];
  const recovery = new Set(edges.filter(edge => ["relocalize", "search_tiles"].includes(edge.route ?? "")).map(edge => edge.to_node));
  for (;;) {
    const before = recovery.size;
    for (const node of draft.nodes) {
      if ((node.inputs ?? []).filter(port => port.required).some(port => {
        const producers = edges.filter(edge => edge.to_node === node.id && edge.to_port === port.id);
        return producers.length > 0 && producers.every(edge => recovery.has(edge.from_node));
      })) recovery.add(node.id);
    }
    if (before === recovery.size) return recovery;
  }
}

export function builderStopLabel(session: Pick<AgentSession, "builder_stop_reason" | "stop_reason">): string {
  const reason = session.builder_stop_reason ?? session.stop_reason;
  if (["discovery_limit_triggered_salvage", "DiscoveryLimitTriggeredSalvage"].includes(reason ?? "")) return "Exploration stopped; saved available plan";
  if (["runnable_candidate_triggered_salvage", "RunnableCandidateTriggeredSalvage"].includes(reason ?? "")) return "Compatible plan found";
  if (["draft_ready", "DraftReady"].includes(reason ?? "")) return "Draft saved for review";
  return reason?.replaceAll("_", " ") ?? "Completed";
}

export function builderPlanSource(session: Pick<AgentSession, "plan_candidates" | "selected_candidate_id">): string {
  const source = session.plan_candidates?.find(candidate => candidate.id === session.selected_candidate_id)?.source;
  if (source === "registry_synthesis") return "System-composed from registered capabilities";
  if (source === "template_seed") return "Template-based plan";
  return source ? "Preserved candidate plan" : "Source not recorded";
}
