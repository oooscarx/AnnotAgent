import { t } from "../i18n";
import { projectReviewPath } from "../navigation";
import type { Annotation, HistoryRun, ImageItem, ProjectSummary, RunResultSummary } from "../types";
import { useState } from "react";
import { canAddRunAnnotation, RunAnnotationEditor } from "./RunAnnotationEditor";
import { queryKeys, workspaceQueries } from "../queryCache";
import { AnnotationCanvas } from "./AnnotationCanvas";
import { hasUnverifiedBoundary } from "../annotationQuality";

/** Results of one existing immutable Run. Debug and history remain management views. */
export function JourneyRun({ project, onNavigationGuardChange, run, image, summary, annotations, annotationsReady, selectedId, original, onSelect, onOriginal, busy, onControl, onNavigate }: {
  project: ProjectSummary; onNavigationGuardChange: (guard?: () => boolean) => void;
  run: HistoryRun; image?: ImageItem; summary?: RunResultSummary; annotations: Annotation[];
  annotationsReady: boolean; busy: boolean; onControl: (action: "pause" | "resume" | "cancel") => void;
  selectedId?: string; original: boolean; onSelect: (id: string) => void; onOriginal: (value: boolean) => void;
  onNavigate: (path: string) => void;
}) {
  const [adding, setAdding] = useState(false);
  const reviewId = summary?.projection.review_candidate_ids[0];
  const projectId = summary?.project_id ?? image?.project_id;
  return <section className="journey-scene journey-results" aria-label={t("Run results")}>
    <div className="journey-intro"><h2>{t(["pending", "running"].includes(run.status) ? "Processing this image" : "Check this image’s results")}</h2><p role="status">{t(run.status)}{summary ? ` · ${t("{count} final results", { count: summary.result_count })}` : ` · ${t("Loading saved results…")}`}</p></div>
    {run.controllable && <div className="button-row" aria-label={t("Processing controls")}>
      {run.status === "running" && <button disabled={busy} onClick={() => onControl("pause")}>{t("Pause")}</button>}
      {run.status === "paused" && <button disabled={busy} onClick={() => onControl("resume")}>{t("Resume")}</button>}
      <button className="danger-button" disabled={busy} onClick={() => onControl("cancel")}>{t("Cancel")}</button>
      <p>{t("Returning to the project keeps this task running. Stopping cannot undo remote requests already sent.")}</p>
    </div>}
    {run.terminal_reason && !["completed", "completed_with_review"].includes(run.status) && <p role="status" className="journey-risk">{run.terminal_reason}</p>}
    {annotationsReady && selectedId && !annotations.some((item) => item.id === selectedId) && <p role="alert">{t("The linked annotation is not a final result of this Run. The same image remains visible; choose an available result from the annotation list.")}</p>}
    {summary?.needs_review_count ? <p className="journey-risk">{t("These candidates are not approved annotations. Check the target and its boundary before accepting.")}</p> : null}
    {hasUnverifiedBoundary(annotations) && <p className="journey-risk">{t("Some boundaries have not been verified. A model confidence score is not measured boundary accuracy.")}</p>}
    {summary?.no_target_count ? <p className="journey-risk">{t("No target was found. This is not evidence that the image contains none; inspect the original image.")}</p> : null}
    <div className="journey-image-controls"><span>{image?.name ?? t("Original image")}</span><button disabled={!image || adding} aria-pressed={original} onClick={() => onOriginal(!original)}>{t(original ? "Show results" : "Show original")}</button></div>
    {adding && image ? <RunAnnotationEditor project={project} runId={run.id} image={image} onNavigationGuardChange={onNavigationGuardChange} onCancel={() => setAdding(false)} onSaved={(annotation) => { workspaceQueries.invalidate(queryKeys.runResults(run.id)); workspaceQueries.invalidate(queryKeys.runAnnotations(run.id)); onNavigate(projectReviewPath(project.id, annotation.id)); }} /> : <div className="journey-result-image">
      {image ? <AnnotationCanvas compactList imageUrl={image.url} annotations={original ? [] : annotations} selectedId={selectedId} onSelect={onSelect} onChange={() => undefined} readOnly /> : <p>{t("The original image is unavailable. No substitute result is shown.")}</p>}
    </div>}
    {!annotationsReady && <p role="status">{t("Loading saved results…")}</p>}
    {annotationsReady && summary && !annotations.length && !summary.no_target_count && <p>{t("No final annotation was produced. Inspect the image and failure details.")}</p>}
    {!adding && <footer className="journey-actions">
      {image && annotationsReady && !run.controllable && !["pending", "running", "paused"].includes(run.status) && canAddRunAnnotation(project) && <button onClick={() => setAdding(true)}>{t("Add a missing annotation")}</button>}
      {projectId && reviewId ? <button className="primary" onClick={() => onNavigate(projectReviewPath(projectId, reviewId))}>{t("Review result")}</button>
        : projectId && !run.controllable && <button className="primary" onClick={() => onNavigate(`/projects/${encodeURIComponent(projectId)}/export`)}>{t("Export confirmed results")}</button>}
    </footer>}
  </section>;
}
