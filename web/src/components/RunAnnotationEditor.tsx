import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import type { Annotation, ImageItem, ProjectSummary } from "../types";
import { AnnotationCanvas } from "./AnnotationCanvas";

export function manualAnnotationValue(kind: string, label: string): Annotation["value"] | undefined {
  if (kind === "classification") return { kind, labels: [label] };
  if (kind === "bounding_box") return { kind, rect: [0.35, 0.35, 0.3, 0.3] };
  if (kind === "keypoints") return { kind, points: [{ name: "point", point: [0.5, 0.5], visible: true }] };
  if (kind === "polyline") return { kind, points: [[0.35, 0.5], [0.65, 0.5]] };
  const rings: [number, number][][] = [[[0.35, 0.35], [0.65, 0.35], [0.5, 0.65]]];
  if (kind === "polygon") return { kind, rings };
  if (kind === "semantic_mask" || kind === "instance_mask") return { kind, mask: { encoding: "polygon", rings } };
  return undefined;
}

export function canAddRunAnnotation(project: ProjectSummary): boolean {
  return project.annotation_schema.some((task) => task.labels.length > 0 && manualAnnotationValue(task.kind, task.labels[0]));
}

/** Explicitly authored human annotation, never a fabricated detector result. */
export function RunAnnotationEditor({ project, runId, image, onSaved, onCancel, onNavigationGuardChange }: {
  project: ProjectSummary; runId: string; image: ImageItem;
  onSaved: (annotation: Annotation) => void; onCancel: () => void;
  onNavigationGuardChange: (guard?: () => boolean) => void;
}) {
  // A background Project refresh must not replace a draft's selected task or
  // erase its geometry. Keep the explicit editing scope until save or cancel.
  const [tasks] = useState(() => structuredClone(project.annotation_schema.filter((task) => task.labels.length && manualAnnotationValue(task.kind, task.labels[0]))));
  const [draft, setDraft] = useState<Annotation>(() => {
    const task = tasks[0];
    return { id: crypto.randomUUID(), image_id: image.image_id, task_id: task.id, label: task.labels[0], value: manualAnnotationValue(task.kind, task.labels[0])!, attributes: {}, confidence: undefined, source: "human", review_status: "needs_review", provenance: { tool_names: [], artifact_ids: [] }, created_at: new Date().toISOString() };
  });
  const [history, setHistory] = useState<Annotation[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const pending = useRef(false);
  const mounted = useRef(true);
  const saved = useRef(false);
  const task = tasks.find((item) => item.id === draft.task_id)!;
  useEffect(() => {
    mounted.current = true;
    const guard = () => saved.current || (!pending.current && window.confirm(t("Discard this unsaved annotation? No formal result has been accepted.")));
    onNavigationGuardChange(guard);
    const unload = (event: BeforeUnloadEvent) => { if (!saved.current) event.preventDefault(); };
    window.addEventListener("beforeunload", unload);
    return () => { mounted.current = false; onNavigationGuardChange(undefined); window.removeEventListener("beforeunload", unload); };
  }, [onNavigationGuardChange]);
  const edit = (next: Annotation) => { if (!busy) { setHistory((items) => [...items, draft]); setDraft(next); } };
  const save = async () => {
    if (pending.current || saved.current) return;
    pending.current = true; setBusy(true); setError("");
    try {
      const result = await api.createAnnotation(runId, draft);
      if (!mounted.current) return;
      saved.current = true; onNavigationGuardChange(undefined); onSaved(result.annotation);
    } catch (value) { if (mounted.current) setError((value as Error).message); }
    finally { pending.current = false; if (mounted.current) setBusy(false); }
  };
  return <section className="sample-feedback-workspace" aria-label={t("Add a missing annotation")}>
    <p className="journey-risk">{t("This is a human-created annotation, not a model prediction. Adjust it before saving. Saving adds it to Review; it does not accept this image or other results.")}</p>
    {tasks.length > 1 && <label>{t("Annotation type")}<select disabled={busy} value={draft.task_id} onChange={(event) => { const next = tasks.find((item) => item.id === event.target.value)!; edit({ ...draft, task_id: next.id, label: next.labels[0], value: manualAnnotationValue(next.kind, next.labels[0])! }); }}>{tasks.map((item) => <option key={item.id} value={item.id}>{item.display_name || item.labels.join(", ")} · {t(item.kind)}</option>)}</select></label>}
    <label>{t("Label")}<select aria-label={t("New annotation label")} disabled={busy} value={draft.label} onChange={(event) => edit({ ...draft, label: event.target.value, value: draft.value.kind === "classification" ? { kind: "classification", labels: [event.target.value] } : draft.value })}>{task.labels.map((label) => <option key={label}>{label}</option>)}</select></label>
    <AnnotationCanvas compactList imageUrl={image.url} annotations={[draft]} selectedId={draft.id} onSelect={() => undefined} onEditStart={() => setHistory((items) => [...items, draft])} onChange={(next) => { if (!busy) setDraft(next); }} readOnly={busy} />
    <div className="journey-actions"><button disabled={busy} onClick={() => { if (window.confirm(t("Discard this unsaved annotation? No formal result has been accepted."))) { saved.current = true; onNavigationGuardChange(undefined); onCancel(); } }}>{t("Cancel annotation")}</button><button disabled={busy || !history.length} onClick={() => { setDraft(history.at(-1)!); setHistory((items) => items.slice(0, -1)); }}>{t("Undo edit")}</button><button className="primary" disabled={busy} onClick={() => void save()}>{t(busy ? "Saving…" : "Save annotation to Review")}</button></div>
    {error && <p role="alert">{error} {t("Your unsaved annotation remains here. Retry saves the same annotation, not a duplicate.")}</p>}
  </section>;
}
