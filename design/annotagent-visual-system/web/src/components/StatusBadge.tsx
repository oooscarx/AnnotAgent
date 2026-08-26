export type AnnotAgentStatus =
  | "draft"
  | "running"
  | "auto-accepted"
  | "needs-review"
  | "rejected"
  | "failed";

const DEFAULT_LABEL: Record<AnnotAgentStatus, string> = {
  draft: "Draft",
  running: "Running",
  "auto-accepted": "Auto accepted",
  "needs-review": "Needs review",
  rejected: "Rejected",
  failed: "Failed",
};

export function StatusBadge({ status, label }: { status: AnnotAgentStatus; label?: string }) {
  return <span className={`aa-status aa-status-${status}`}>{label ?? DEFAULT_LABEL[status]}</span>;
}
