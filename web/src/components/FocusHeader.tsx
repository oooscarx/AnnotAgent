import type { RefObject } from "react";
import { t } from "../i18n";
import { projectBuildPath, projectReviewPath, projectRunsPath, projectTrashPath, type WorkspaceRoute } from "../navigation";
import type { ProjectSummary } from "../types";

export function usesFocusLayout(route: WorkspaceRoute): boolean {
  return (route.kind === "projects" && Boolean(route.create))
    || route.kind === "build" || route.kind === "projectRun" || route.kind === "projectBatch"
    || (route.kind === "projectReview" && Boolean(route.reviewItemId)) || route.kind === "export";
}

export function FocusHeader({ project, route, titleRef, loaded, connection, onNavigate }: {
  project?: ProjectSummary; route: WorkspaceRoute; titleRef: RefObject<HTMLHeadingElement | null>;
  loaded: boolean; connection: string; onNavigate: (path: string) => void;
}) {
  const projectId = "projectId" in route ? route.projectId : undefined;
  const parent = projectId ? `/projects/${encodeURIComponent(projectId)}` : "/projects";
  const title = route.kind === "projects" ? "Images and your goal" : route.kind === "build"
    ? ({ data: "Project images", labels: "Annotation goal", pipeline: "Annotation plan", test: "Inspect samples" })[route.step]
    : route.kind === "projectReview" ? "Review annotations" : route.kind === "export" ? "Export dataset" : "Processing and results";
  const items = projectId ? [
    ["Project overview", parent], ["Data management", projectBuildPath(projectId, "data")],
    ["Automation", projectBuildPath(projectId, "pipeline")], ["Processing history", projectRunsPath(projectId)],
    ["Review list", projectReviewPath(projectId)], ["Export dataset", `${parent}/export`],
    ["Project labels and settings", projectBuildPath(projectId, "labels")], ["Recycle bin", projectTrashPath(projectId)],
  ] : [["My projects", "/projects"]];
  return <header className="focus-header" aria-label={t("Task context")}>
    <button onClick={() => onNavigate(parent)}>{t(projectId ? "Back to project" : "Back to projects")}</button>
    <div className="focus-heading"><span>{project?.name ?? t(projectId ? "Loading Project…" : "New annotation project")}</span><h1 ref={titleRef} tabIndex={-1}>{t(title)}</h1></div>
    <span className="focus-connection" role="status">{t(loaded ? "Workspace loaded" : "Loading workspace state…")} · SSE {t(connection)}</span>
    <details className="focus-project-menu"><summary>{t("Project menu")}</summary><nav aria-label={t("Project management")}>
      {items.map(([label, path]) => <a key={path} href={path} onClick={(event) => { event.preventDefault(); event.currentTarget.closest("details")?.removeAttribute("open"); onNavigate(path); }}>{t(label)}</a>)}
    </nav></details>
  </header>;
}
