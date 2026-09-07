import { useEffect, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import type { Annotation, ImageItem, SampleFeedbackRevision, WorkflowDryRunReport } from "../types";
import { AnnotationCanvas } from "./AnnotationCanvas";

const reasons: [SampleFeedbackRevision["reason"], string][] = [
  ["correct", "Target and boundary are correct"], ["wrong_target", "Wrong target"],
  ["poor_boundary", "Boundary needs correction"], ["missing_target", "A target is missing"],
  ["cannot_judge", "Cannot judge yet"],
];

export function SampleFeedbackEditor({ sample, image, testId, onDirtyChange, onConfirmed }: {
  sample: WorkflowDryRunReport["samples"][number]; image: ImageItem; testId: string;
  onDirtyChange: (dirty: boolean) => void;
  onConfirmed?: () => void;
}) {
  const original: Annotation[] = sample.outcomes.flatMap((outcome) => outcome.value ? [{
    id: outcome.id, image_id: image.image_id, task_id: "sample", label: outcome.label,
    value: outcome.value, attributes: {}, confidence: outcome.confidence ?? undefined,
    source: "sample test", review_status: "needs_review" as const, provenance: { sample_test_id: testId }, created_at: "",
  }] : []);
  const [annotations, setAnnotations] = useState(original);
  const [selected, setSelected] = useState<string>();
  const [history, setHistory] = useState<Annotation[][]>([]);
  const [revisions, setRevisions] = useState<SampleFeedbackRevision[]>([]);
  const [reason, setReason] = useState<SampleFeedbackRevision["reason"]>("cannot_judge");
  const [note, setNote] = useState("");
  const [dirty, setDirty] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState(false);
  const [showOriginal, setShowOriginal] = useState(false);
  const [hint, setHint] = useState(true);
  const selectedAnnotation = annotations.find((item) => item.id === selected);
  useEffect(() => {
    let current = true;
    void api.sampleFeedback(testId, image.image_id).then(({ revisions: values }) => {
      if (!current) return;
      setRevisions(values);
      const restored = original.map((annotation) => {
        const revision = [...values].reverse().find((value) => value.outcome_id === annotation.id && value.corrected_value);
        return revision?.corrected_value ? { ...annotation, value: revision.corrected_value } : annotation;
      });
      setAnnotations(restored);
      const last = values.at(-1);
      if (last) { setReason(last.reason); setNote(last.note); setSelected(last.outcome_id ?? undefined); setSaved(true); }
      setLoaded(true);
    }).catch((error: Error) => { if (current) setError(error.message); });
    return () => { current = false; };
  }, [testId, image.image_id]);
  useEffect(() => { onDirtyChange(dirty); return () => onDirtyChange(false); }, [dirty, onDirtyChange]);
  useEffect(() => {
    if (!dirty) return;
    const guard = (event: BeforeUnloadEvent) => event.preventDefault();
    window.addEventListener("beforeunload", guard);
    return () => window.removeEventListener("beforeunload", guard);
  }, [dirty]);
  const edit = (annotation: Annotation) => {
    if (!loaded || busy || showOriginal || annotation.id !== selected || annotation.value.kind !== "bounding_box") return;
    setAnnotations((items) => items.map((item) => item.id === annotation.id ? annotation : item));
    setDirty(true); setSaved(false); setReason("poor_boundary");
  };
  const save = async (confirm = false) => {
    if (busy || !loaded) return;
    setBusy(true); setError("");
    const revision: SampleFeedbackRevision = {
      revision_id: crypto.randomUUID(), sample_test_id: testId, image_id: image.image_id,
      sequence: (revisions.at(-1)?.sequence ?? 0) + 1, reason: confirm ? "correct" : reason, note,
      outcome_id: reason === "missing_target" ? null : selected,
      corrected_value: reason !== "missing_target" && selectedAnnotation?.value.kind === "bounding_box" ? selectedAnnotation.value : null,
      created_at: new Date().toISOString(),
    };
    try {
      const value = await api.saveSampleFeedback(revision);
      setRevisions((items) => [...items, value.revision]); setDirty(false); setSaved(true); setHistory([]);
      if (confirm) { setReason("correct"); onDirtyChange(false); if (!selected) onConfirmed?.(); }
    } catch (error) { setError((error as Error).message); }
    finally { setBusy(false); }
  };
  return <div className="sample-feedback-workspace">
    <section className="sample-feedback-image">
      <div className="button-row"><button aria-pressed={showOriginal} onClick={() => setShowOriginal(true)}>{t("Original image")}</button><button aria-pressed={!showOriginal} onClick={() => setShowOriginal(false)}>{t("Current candidates")}</button></div>
      <AnnotationCanvas compactList imageUrl={image.url} annotations={showOriginal ? [] : annotations} selectedId={selected} readOnly={!loaded || busy || selectedAnnotation?.value.kind !== "bounding_box"} onSelect={(id) => {
        if (dirty && selected !== id) { setError(t("Save or undo this correction before selecting another result.")); return; }
        setSelected(id);
      }} onEditStart={() => setHistory((items) => [...items, annotations])} onChange={edit} />
    </section>
    <details className="sample-feedback-decision" open={dirty || undefined}>
      <summary>{t("Result needs attention")}</summary>
      <div className="sample-feedback-fields">
      <h3>{t("Your decision on this image")}</h3>
      <p>{t("Feedback is saved to this Sample Test. It does not accept formal annotations or change the Pipeline automatically.")}</p>
      <label>{t("Result to inspect")}<select aria-label={t("Result to inspect")} value={selected ?? ""} disabled={dirty || busy} onChange={(event) => setSelected(event.target.value || undefined)}><option value="">{t("Whole image")}</option>{annotations.map((annotation, index) => <option key={annotation.id} value={annotation.id}>{index + 1}. {annotation.label}</option>)}</select></label>
      {hint && selectedAnnotation?.value.kind === "bounding_box" && <div className="inline-notice"><p>{t("Drag corners to adjust the box, or edit its normalized coordinates below.")}</p><button onClick={() => setHint(false)}>{t("Dismiss hint")}</button></div>}
      {!hint && <button onClick={() => setHint(true)}>{t("Show editing hint")}</button>}
      {selectedAnnotation?.value.kind === "bounding_box" && <div className="sample-feedback-coordinates">{selectedAnnotation.value.rect.map((value, index) => <label key={index}>{["x", "y", "width", "height"][index]}<input type="number" step="0.001" min="0" max="1" value={Number(value.toFixed(6))} disabled={!loaded || busy} onChange={(event) => {
        if (selectedAnnotation.value.kind !== "bounding_box") return;
        const rect = [...selectedAnnotation.value.rect] as [number, number, number, number]; rect[index] = Number(event.target.value);
        setHistory((items) => [...items, annotations]); edit({ ...selectedAnnotation, value: { kind: "bounding_box", rect } });
      }} /></label>)}</div>}
      <label>{t("What needs attention?")}<select aria-label={t("What needs attention?")} value={reason} disabled={!loaded || busy} onChange={(event) => { setReason(event.target.value as SampleFeedbackRevision["reason"]); setDirty(true); setSaved(false); }}>{reasons.map(([value, label]) => <option key={value} value={value}>{t(label)}</option>)}</select></label>
      <label>{t("Feedback note")}<textarea aria-label={t("Feedback note")} maxLength={4000} disabled={!loaded || busy} value={note} onChange={(event) => { setNote(event.target.value); setDirty(true); setSaved(false); }} /></label>
      <div className="button-row"><button disabled={!loaded || busy} onClick={() => void save()}>{t("Save sample feedback")}</button><button disabled={!history.length || busy} onClick={() => { setAnnotations(history.at(-1)!); setHistory((items) => items.slice(0, -1)); setDirty(true); }}>{t("Undo edit")}</button></div>
      {reason !== "correct" && <p>{t("This records a quality issue, not a promised improvement. Review the existing Pipeline or correct the result manually; a new model test requires separate authorization.")}</p>}
      </div>
    </details>
    <div className="sample-confirm-action"><span>{t(selected ? "This decision applies only to the selected result." : "This decision applies to this sample image only.")}</span><button className="primary" disabled={!loaded || busy} onClick={() => void save(true)}>{t(selected ? "Confirm selected result" : onConfirmed ? "Confirm sample and next" : "Confirm this sample")}</button>{saved && <span role="status">{t("Sample feedback saved")}</span>}{error && <p role="alert">{error}</p>}</div>
  </div>;
}
