import type { ProjectSummary } from "./types";

export type ProjectNextAction =
  | { kind: "build"; step: "data" | "labels" | "pipeline" | "test"; label: string }
  | { kind: "active_run"; runId: string; label: "Open active run" }
  | { kind: "active_batch"; label: "View active batch" }
  | { kind: "review"; label: "Review results" }
  | { kind: "start"; label: "Start run" };

export function deriveProjectNextAction(
  project: ProjectSummary,
): ProjectNextAction {
  if (project.active_run)
    return { kind: "active_run", runId: project.active_run.id, label: "Open active run" };
  if (project.active_batch)
    return { kind: "active_batch", label: "View active batch" };
  if (project.image_count === 0)
    return { kind: "build", step: "data", label: "Add images" };
  if (
    project.task_count === 0 ||
    project.annotation_schema.every((task) => task.labels.length === 0)
  )
    return { kind: "build", step: "labels", label: "Define labels" };
  const blockingStep = project.blocking_issues[0]?.next_step;
  if (blockingStep)
    return {
      kind: "build",
      step: blockingStep,
      label:
        blockingStep === "pipeline"
          ? project.readiness === "configuration_issue"
            ? "Fix pipeline"
            : "Choose pipeline"
          : blockingStep === "test"
            ? "Test on samples"
            : blockingStep === "labels"
              ? "Define labels"
              : "Add images",
    };
  if (project.review_count > 0)
    return { kind: "review", label: "Review results" };
  return { kind: "start", label: "Start run" };
}
