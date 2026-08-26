import type { ProjectSummary } from "./types";

export interface ProjectRunView {
  activeRunId: string;
  activeStatus?: string;
  startDisabled: boolean;
}

/** Server Project state is the only source of truth; component navigation adds no run state. */
export function deriveProjectRunView(project?: ProjectSummary): ProjectRunView {
  return {
    activeRunId: project?.active_run?.id ?? "",
    activeStatus: project?.active_run?.status,
    startDisabled: Boolean(project?.active_run),
  };
}
