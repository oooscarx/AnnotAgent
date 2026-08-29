import type { HistoryRun, ProjectSummary, ReviewItem } from "./types";

export function projectForRun(
  projects: ProjectSummary[],
  run?: HistoryRun,
): ProjectSummary | undefined {
  return run
    ? projects.find((project) => project.name === run.project_name)
    : undefined;
}

export function projectForReview(
  projects: ProjectSummary[],
  review?: ReviewItem,
): ProjectSummary | undefined {
  if (!review) return undefined;
  return projects.find(
    (project) =>
      project.id === review.project_id ||
      (!review.project_id && project.name === review.project_name),
  );
}

export function runsForContext(
  runs: HistoryRun[],
  project?: ProjectSummary,
  status = "all",
): HistoryRun[] {
  return runs.filter(
    (run) =>
      (!project || run.project_name === project.name) &&
      (status === "all" || run.status === status),
  );
}
