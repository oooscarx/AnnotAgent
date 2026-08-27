import type { ProjectSummary } from "./types";

export interface ProjectRunView {
  activeRunId: string;
  activeBatchId: string;
  activeStatus?: string;
  startDisabled: boolean;
}

/** Server Project state is the only source of truth; component navigation adds no run state. */
export function deriveProjectRunView(project?: ProjectSummary): ProjectRunView {
  return {
    activeRunId: project?.active_run?.id ?? "",
    activeBatchId: project?.active_batch?.id ?? "",
    activeStatus: project?.active_run?.status ?? project?.active_batch?.status,
    startDisabled: Boolean(project?.active_run || project?.active_batch),
  };
}
