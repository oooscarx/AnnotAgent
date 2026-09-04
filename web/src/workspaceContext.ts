import type { HistoryRun, ProjectSummary, ReviewItem } from "./types";

export function projectForRun(
  projects: ProjectSummary[],
  run?: HistoryRun,
): ProjectSummary | undefined {
  return run
    ? projects.find((project) => project.project_id === run.project_id)
    : undefined;
}

export function projectForReview(
  projects: ProjectSummary[],
  review?: ReviewItem,
): ProjectSummary | undefined {
  if (!review) return undefined;
  return projects.find(
    (project) => project.project_id === review.project_id,
  );
}

export function runsForContext(
  runs: HistoryRun[],
  project?: ProjectSummary,
  status = "all",
): HistoryRun[] {
  return runs.filter(
    (run) =>
      (!project || run.project_id === project.project_id) &&
      (status === "all" || run.status === status),
  );
}
