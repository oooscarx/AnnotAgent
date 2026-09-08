import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import { projectBatchPath, projectReviewPath, type WorkspaceRoute } from "../navigation";
import { queryKeys, workspaceQueries } from "../queryCache";
import { useRouteQuery } from "../useRouteQuery";
import type { DatasetBatchSummary, ProjectSummary } from "../types";
import { canAddRunAnnotation, RunAnnotationEditor } from "./RunAnnotationEditor";
import { AnnotationCanvas } from "./AnnotationCanvas";
import { hasUnverifiedBoundary } from "../annotationQuality";
import { BatchControls } from "./BatchControls";

/** A results presentation of the existing Batch and terminal projection, not an executor. */
export function JourneyBatch({ project, onNavigationGuardChange, batch, route, onNavigate, onReload }: {
  project?: ProjectSummary; onNavigationGuardChange: (guard?: () => boolean) => void;
  batch: DatasetBatchSummary; route: Extract<WorkspaceRoute, { kind: "projectBatch" }>;
  onNavigate: (path: string, replace?: boolean) => void; onReload: () => Promise<unknown>;
}) {
  const selectedId = route.annotationId;
  const original = route.canvasView === "original";
  const [error, setError] = useState("");
  const [addingImage, setAddingImage] = useState<string>();
  const active = ["pending", "running", "paused"].includes(batch.status);
  const reloadRef = useRef(onReload); reloadRef.current = onReload;
  useEffect(() => {
    if (!active) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try { await reloadRef.current(); } catch (failure) { if (!stopped) setError((failure as Error).message); }
      if (!stopped) timer = setTimeout(poll, 1500);
    };
    timer = setTimeout(poll, 1500);
    return () => { stopped = true; clearTimeout(timer); };
  }, [batch.id, active]);
  const status = route.status ?? "all";
  const images = batch.images.filter((image) => status === "all" || image.status === status);
  const selected = route.imageId ? images.find((image) => image.image_id === route.imageId) : images[0];
  useEffect(() => { setAddingImage(undefined); }, [selected?.image_id]);
  const index = selected ? images.indexOf(selected) : -1;
  const imageQuery = useRouteQuery(queryKeys.projectImages(batch.project_id), (signal) => api.images(batch.project_id, signal), { staleTime: 30_000 });
  const runId = selected?.child_run_id;
  const results = useRouteQuery(runId ? queryKeys.runResults(runId) : undefined, (signal) => api.runResultSummary(runId!, signal));
  const annotations = useRouteQuery(runId ? queryKeys.runAnnotations(runId) : undefined, (signal) => api.runAnnotations(runId!, signal));
  // A poll completion invalidates only this image's real result, never causes execution.
  useEffect(() => {
    if (!runId) return;
    void results.retry().catch(() => undefined);
    void annotations.retry().catch(() => undefined);
  }, [runId, selected?.status, selected?.annotation_count, selected?.review_count]);
  const result = results.data && results.data.run_id === runId && results.data.project_id === batch.project_id ? results.data : undefined;
  const inspection = annotations.data && annotations.data.run_id === runId && annotations.data.project_id === batch.project_id ? annotations.data : undefined;
  const owned = result?.image.image_id === selected?.image_id && inspection?.image_id === selected?.image_id;
  const finalIds = new Set([...(result?.projection.committed_annotation_ids ?? []), ...(result?.projection.review_candidate_ids ?? [])]);
  const final = owned ? (inspection?.annotations ?? []).filter((annotation) => finalIds.has(annotation.id)) : [];
  const image = imageQuery.data?.images.find((item) => item.image_id === selected?.image_id && item.project_id === batch.project_id);
  const select = (imageId?: string, nextStatus = status) => onNavigate(projectBatchPath(batch.project_id, batch.id, { imageId, status: nextStatus }));
  const canvasContext = (annotationId = selectedId, showOriginal = original) => onNavigate(projectBatchPath(batch.project_id, batch.id, { imageId: selected?.image_id, status, annotationId, canvasView: showOriginal ? "original" : undefined }), true);
  const reviewId = selected?.review_ids[0];
  const adding = Boolean(selected && addingImage === selected.image_id);
  return <section className="journey-scene journey-results" aria-label={t("Processing results")}>
    <div className="journey-intro"><h2>{t(active ? "Processing your images" : "Your processing results")}</h2>
      <p role="status">{t(batch.status)} · {t("{done} of {total} images settled", { done: batch.images.filter((item) => !["pending", "leased", "running"].includes(item.status)).length, total: batch.progress.total_images })}</p>
      {active && <p>{t("Returning to the project keeps this task running. Stopping cannot undo remote requests already sent.")}</p>}
      <BatchControls key={batch.id} batchId={batch.id} status={batch.status} onReload={() => reloadRef.current()} />
    </div>
    {(error || imageQuery.error || results.error || annotations.error) && <div role="alert"><p>{error || imageQuery.error?.message || results.error?.message || annotations.error?.message}</p><button onClick={() => { void onReload(); void imageQuery.retry().catch(() => undefined); void results.retry().catch(() => undefined); void annotations.retry().catch(() => undefined); }}>{t("Reload results")}</button></div>}
    <div className="journey-image-controls">
      <label>{t("Show images")}<select aria-label={t("Show images")} value={status} onChange={(event) => select(undefined, event.target.value)}>
        {[['all', 'All images'], ['ready', 'Saved results'], ['no_target', 'No target found'], ['needs_review', 'Needs review'], ['failed', 'Failed'], ['cancelled', 'Cancelled'], ['pending', 'Pending'], ['running', 'Running']].map(([value, label]) => <option key={value} value={value}>{t(label)}</option>)}
      </select></label>
      <span>{index >= 0 ? t("Image {index} of {count}", { index: index + 1, count: images.length }) : t("No images match this status")}</span>
      <button aria-pressed={original} disabled={!selected || adding} onClick={() => canvasContext(selectedId, !original)}>{t(original ? "Show results" : "Show original")}</button>
    </div>
    {selected && <div className="journey-result-image"><h3>{selected.name}</h3><p>{t(selected.status)}{selected.failure ? ` · ${selected.failure}` : ""}</p>
      {selected.status === "needs_review" && <p className="journey-risk" role="status">{t("These candidates are not approved annotations. Check the target and its boundary before accepting.")}</p>}
      {hasUnverifiedBoundary(final) && <p className="journey-risk">{t("Some boundaries have not been verified. A model confidence score is not measured boundary accuracy.")}</p>}
      {owned && selectedId && !final.some((item) => item.id === selectedId) && <p role="alert">{t("The linked annotation is not a final result of this Run. The same image remains visible; choose an available result from the annotation list.")}</p>}
      {adding && image && project && runId ? <RunAnnotationEditor key={image.image_id} project={project} runId={runId} image={image} onNavigationGuardChange={onNavigationGuardChange} onCancel={() => setAddingImage(undefined)} onSaved={(annotation) => { workspaceQueries.invalidate(queryKeys.runResults(runId)); workspaceQueries.invalidate(queryKeys.runAnnotations(runId)); onNavigate(projectReviewPath(project.id, annotation.id)); }} /> : image ? <AnnotationCanvas key={image.image_id} imageUrl={image.url} annotations={original ? [] : final} selectedId={selectedId} onSelect={(id) => canvasContext(id)} onChange={() => undefined} readOnly compactList /> : <p>{t(imageQuery.loading ? "Loading saved results…" : "The original image is unavailable. No substitute result is shown.")}</p>}
      {selected.status === "no_target" && <p>{t("No target was found. This is not evidence that the image contains none; inspect the original image.")}</p>}
      {runId && !owned && !results.loading && !annotations.loading && <p role="alert">{t("Final results are not available for this image yet.")}</p>}
    </div>}
    {!selected && <p>{t("Choose another image filter. Your current image link is not replaced with another result.")}</p>}
    {!adding && <footer className="journey-actions">
      {project && image && runId && owned && selected && !["pending", "running", "leased"].includes(selected.status) && canAddRunAnnotation(project) && <button onClick={() => setAddingImage(image.image_id)}>{t("Add a missing annotation")}</button>}
      <button disabled={index <= 0} onClick={() => select(images[index - 1]?.image_id)}>{t("Previous image")}</button>
      <button disabled={index < 0 || index >= images.length - 1} onClick={() => select(images[index + 1]?.image_id)}>{t("Next image")}</button>
      {reviewId ? <button className="primary" onClick={() => onNavigate(projectReviewPath(batch.project_id, reviewId))}>{t("Review this image")}</button>
        : !active && <button className="primary" onClick={() => onNavigate(`/projects/${encodeURIComponent(batch.project_id)}/export`)}>{t("Export confirmed results")}</button>}
    </footer>}
  </section>;
}
