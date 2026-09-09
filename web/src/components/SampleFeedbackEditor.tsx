import { useEffect, useRef, useState, type ReactNode } from "react";
import { api, ApiRequestError } from "../api";
import { t } from "../i18n";
import type { Annotation, ImageItem, SampleFeedbackRevision, WorkflowDryRunReport } from "../types";
import { AnnotationCanvas } from "./AnnotationCanvas";
import { useSampleFreshness } from "../useSampleFreshness";
import { terminalSampleAnnotations } from "../sampleAnnotations";
import { sampleFeedbackOverlay, type ExcludedSampleCandidate } from "../sampleFeedbackOverlay";
import { SampleExcludedCandidates } from "./SampleExcludedCandidates";
import { humanReferenceReady } from "../humanReference";

const reasons: [SampleFeedbackRevision["reason"], string][] = [
  ["correct", "Target and boundary are correct"], ["wrong_target", "Wrong target"],
  ["poor_boundary", "Boundary needs correction"], ["missing_target", "A target is missing"],
  ["cannot_judge", "Cannot judge yet"],
];

export function SampleFeedbackEditor({ sample, image, testId, onDirtyChange, onConfirmed, navigation, onAdopt, projectId, draftId, onImprove, onKeepOriginal, goalOverride, humanSubmission, initialOutcomeId, onReferenceOutcome, readOnly=false }: {
  readOnly?:boolean;
  onReferenceOutcome?:(id:string)=>void;
  sample: WorkflowDryRunReport["samples"][number]; image: ImageItem; testId: string;
  onDirtyChange: (dirty: boolean) => void;
  onConfirmed?: () => void;
  onAdopt?: () => void;
  projectId: string;
  draftId: string;
  onKeepOriginal: (draftId: string, testId: string, imageId?: string) => void;
  onImprove?: (draftId: string, testId: string, imageId?: string) => void;
  navigation?: ReactNode;
  goalOverride?: {kind:string;labels:string[]};
  humanSubmission?: ({outcomeId:string;additionId?:never}|{additionId:string;outcomeId?:never}) & {continueAfterSave?:boolean;save:(revision:SampleFeedbackRevision)=>Promise<SampleFeedbackRevision>};
  initialOutcomeId?: string;
}) {
  const freshness = useSampleFreshness(projectId, draftId, testId);
  const original = terminalSampleAnnotations(sample,image.image_id,testId);
  const [annotations, setAnnotations] = useState(original);
  const [excluded, setExcluded] = useState<ExcludedSampleCandidate[]>([]);
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
  const [before, setBefore] = useState<{annotations: Annotation[]; draftId:string; testId:string}>();
  const [showBefore, setShowBefore] = useState(false);
  const [projectGoal, setGoal] = useState<Awaited<ReturnType<typeof api.projectGoal>>>();
  const goal=goalOverride ?? projectGoal;
  const mounted = useRef(true);
  const copyKey = useRef(crypto.randomUUID());
  const copying = useRef(false);
  const saving = useRef(false);
  const pendingFeedback = useRef<SampleFeedbackRevision | undefined>(undefined);
  const referenceSeed = useRef<Annotation | undefined>(undefined);
  useEffect(() => {
    let current = true; mounted.current = true;
    if(!goalOverride)void api.projectGoal(projectId).then((value) => { if (current) setGoal(value); }).catch((error: Error) => { if (current) setError(error.message); });
    void api.samplePlanEvidence(projectId, draftId).then(async (evidence) => {
      const { sample_test: baseline } = await api.workflowSampleTest(evidence.baseline_draft_id, undefined, evidence.sample_test_id);
      if (!current) return;
      if (!baseline || baseline.project_id !== projectId) throw new Error(t("The original sample is unavailable for comparison."));
      const position = baseline.inputs.findIndex((input) => input.image_id === image.image_id && input.content_hash === image.content_hash);
      const originalSample = baseline.report.samples[position];
      if (!originalSample?.projection) return;
      setBefore({ draftId: baseline.draft_id, testId: baseline.id, annotations: terminalSampleAnnotations(originalSample,image.image_id,baseline.id) });
    }).catch((error: Error) => { if (current && !(error instanceof ApiRequestError && error.status === 404)) setError(error.message); });
    return () => { current = false; mounted.current = false; };
  }, [projectId, draftId, image.image_id, image.content_hash]);
  const [hint, setHint] = useState(true);
  const [attentionOpen, setAttentionOpen] = useState(Boolean(humanSubmission));
  const selectedAnnotation = annotations.find((item) => item.id === selected);
  const referenceReady = !humanSubmission?.additionId || humanReferenceReady(selectedAnnotation,humanSubmission.additionId,referenceSeed.current);
  const selectedMaskIsRaster = selectedAnnotation && (selectedAnnotation.value.kind === "semantic_mask" || selectedAnnotation.value.kind === "instance_mask") && selectedAnnotation.value.mask.encoding !== "polygon";
  useEffect(() => {
    let current = true;
    void api.sampleFeedback(testId, image.image_id).then(({ revisions: values }) => {
      if (!current) return;
      setRevisions(values);
      const overlay = sampleFeedbackOverlay(original, values);
      const restored = overlay.annotations;
      setExcluded(overlay.excluded);
      setAnnotations(restored);
      const last = values.at(-1);
      if (last) { setReason(last.reason); setNote(last.note); setSelected(last.addition_id ? `human-sample:${last.addition_id}` : last.outcome_id ?? undefined); setSaved(true); }
      const requestedOutcome=humanSubmission?.outcomeId ?? initialOutcomeId;
      if(requestedOutcome && restored.some(value=>value.id===requestedOutcome)){
        setSelected(requestedOutcome);
        const prior=values.filter(value=>value.outcome_id===requestedOutcome || (value.addition_id && `human-sample:${value.addition_id}`===requestedOutcome)).at(-1);
        setSaved(!humanSubmission && Boolean(prior));
        setReason(prior && prior.reason!=="missing_target" ? prior.reason : "cannot_judge");
        setNote(prior?.note ?? "");
      }
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
    if (!loaded || busy || showOriginal || showBefore || annotation.id !== selected) return;
    setAnnotations((items) => items.map((item) => item.id === annotation.id ? { ...annotation, confidence: undefined, source: "human sample feedback", provenance: { ...annotation.provenance, human_corrected: true } } : item));
    setDirty(true); setSaved(false); setReason("poor_boundary"); setAttentionOpen(true);
  };
  const save = async (confirm = false) => {
    if (busy || saving.current || !loaded || showBefore || !referenceReady) return;
    saving.current = true;
    setBusy(true); setError("");
    const additionId = typeof selectedAnnotation?.provenance.addition_id === "string" ? selectedAnnotation.provenance.addition_id : undefined;
    const newAddition = additionId && !revisions.some((revision) => revision.addition_id === additionId);
    let revision: SampleFeedbackRevision = {
      revision_id: crypto.randomUUID(), sample_test_id: testId, image_id: image.image_id,
      sequence: (revisions.at(-1)?.sequence ?? 0) + 1, reason: newAddition ? "missing_target" : confirm ? "correct" : reason, note,
      addition_id: additionId,
      outcome_id: humanSubmission?.outcomeId ?? (additionId || (!confirm && reason === "missing_target") ? null : selected),
      corrected_value: (additionId || confirm || reason !== "missing_target") ? selectedAnnotation?.value : null,
      corrected_label: (additionId || confirm || reason !== "missing_target") ? selectedAnnotation?.label : null,
      created_at: new Date().toISOString(),
    };
    const payload = (value: SampleFeedbackRevision) => JSON.stringify({ ...value, revision_id: "", created_at: "" });
    if (pendingFeedback.current && payload(pendingFeedback.current) === payload(revision)) revision = pendingFeedback.current;
    pendingFeedback.current = revision;
    try {
      const value = humanSubmission ? {revision:await humanSubmission.save(revision)} : await api.saveSampleFeedback(revision);
      pendingFeedback.current = undefined;
      if (!mounted.current) return;
      setRevisions((items) => [...items, value.revision]); setReason(value.revision.reason); setDirty(false); setSaved(true); setHistory([]);
      if (confirm) { onDirtyChange(false); if (!selected) onConfirmed?.(); }
    } catch (error) { if (mounted.current) setError((error as Error).message); }
    finally { saving.current = false; if (mounted.current) setBusy(false); }
  };
  const addMissing = () => {
    if (!loaded || busy || dirty || !goal || humanSubmission?.outcomeId || (humanSubmission?.additionId && annotations.some(value=>value.provenance.addition_id===humanSubmission.additionId))) return;
    const label = humanSubmission?.additionId && goal.kind==="classification" ? "" : goal.labels?.[0] ?? "";
    const value: Annotation["value"] | undefined = goal.kind === "classification" ? { kind: "classification", labels: label ? [label] : [] }
      : goal.kind === "bounding_box" ? { kind: "bounding_box", rect: [0.4, 0.4, 0.2, 0.2] }
      : goal.kind === "semantic_mask" || goal.kind === "instance_mask" ? { kind: goal.kind, mask: { encoding: "polygon", rings: [[[0.4, 0.4], [0.6, 0.4], [0.6, 0.6], [0.4, 0.6]]] } }
      : goal.kind === "polygon" ? { kind: "polygon", rings: [[[0.4, 0.4], [0.6, 0.4], [0.6, 0.6], [0.4, 0.6]]] } : undefined;
    if (!value) return;
    const additionId = humanSubmission?.additionId ?? crypto.randomUUID();
    const added: Annotation = { id: `human-sample:${additionId}`, image_id: image.image_id, task_id: "sample", label, value, attributes: {}, source: "human sample feedback", review_status: "needs_review", provenance: { addition_id: additionId, sample_test_id: testId }, created_at: "" };
    if(humanSubmission?.additionId)referenceSeed.current=added;
    setHistory((items) => [...items, annotations]); setAnnotations((items) => [...items, added]); setSelected(added.id);
    setDirty(true); setSaved(false); setReason("missing_target"); setAttentionOpen(true); setShowBefore(false); setShowOriginal(false);
  };
  if(readOnly)return <section aria-label="Saved sample corrections"><p>Saved sample corrections · Read only</p>{error ? <p role="alert">{error}</p> : !loaded ? <p role="status">Loading saved corrections…</p> : <><AnnotationCanvas compactList imageUrl={image.url} annotations={annotations} selectedId={selected} readOnly onSelect={setSelected} onChange={()=>{}}/><SampleExcludedCandidates excluded={excluded} imageUrl={image.url}/></>}</section>;
  return <div className="sample-feedback-workspace">
    {freshness.status === "checking" && <p role="status">{t("Checking whether this sample still matches the current plan…")}</p>}
    {freshness.status === "stale" && <p role="alert" className="sample-risk-notice">{t("The plan or its inputs changed elsewhere. Your edits are kept and can still be saved as feedback on this sample. Test the current plan before adopting it.")}</p>}
    {freshness.status === "unavailable" && <div role="alert"><p>{t("Cannot verify this sample right now. Your edits are kept; adopting or adjusting the plan is paused until verification succeeds.")}</p><button onClick={freshness.retry}>{t("Check sample again")}</button></div>}
    <section className="sample-feedback-image">
      <div className="button-row"><button aria-pressed={showOriginal} onClick={() => { setShowOriginal(true); setShowBefore(false); }}>{t("Original image")}</button>{before && <button aria-pressed={showBefore} onClick={() => { setShowOriginal(false); setShowBefore(true); }}>{t("Before adjustment")}</button>}<button aria-pressed={!showOriginal && !showBefore} onClick={() => { setShowOriginal(false); setShowBefore(false); }}>{t("Current candidates")}</button></div>
      {before && <p>{t("Compare two saved tests of this same image. A proposed change is not proof of improved accuracy.")}</p>}
      <AnnotationCanvas compactList imageUrl={image.url} annotations={showOriginal ? [] : showBefore ? before?.annotations ?? [] : annotations} selectedId={showBefore ? undefined : selected} readOnly={!loaded || busy || showOriginal || showBefore || Boolean(selectedMaskIsRaster)} onSelect={(id) => {
        if (showBefore) return;
        if (dirty && selected !== id) { setError(t("Save or undo this correction before selecting another result.")); return; }
        if(!humanSubmission || id===(humanSubmission.additionId ? `human-sample:${humanSubmission.additionId}` : humanSubmission.outcomeId))setSelected(id);
      }} onEditStart={() => setHistory((items) => [...items, annotations])} onChange={edit} />
    </section>
    <SampleExcludedCandidates excluded={excluded} imageUrl={image.url}/>
    {onReferenceOutcome && <div className="conversation-candidate-reference"><button disabled={!loaded||busy||dirty||showOriginal||showBefore||!selected||!sample.outcomes.some(outcome=>outcome.id===selected)} onClick={()=>{if(selected)onReferenceOutcome(selected);}}>Reference saved candidate in message</button><small>References the original saved prediction, not later corrections or unsaved edits.</small></div>}
    {(!humanSubmission || humanSubmission.additionId) && goal && (humanSubmission ? ["classification","bounding_box"] : ["classification", "bounding_box", "polygon", "semantic_mask", "instance_mask"]).includes(goal.kind ?? "") && <button className="sample-add-missing" disabled={!loaded || busy || dirty || showBefore || Boolean(humanSubmission?.additionId && annotations.some(value=>value.provenance.addition_id===humanSubmission.additionId))} onClick={addMissing}>{humanSubmission ? "Add reference target" : t("Add missing target")}</button>}
    {typeof selectedAnnotation?.provenance.addition_id === "string" && <p className="sample-risk-notice">{t("Human sample example, not a model prediction. Adjust its label and boundary before saving. It never becomes a formal annotation automatically.")}</p>}
    {selectedAnnotation && !showOriginal && !showBefore && <label className="sample-feedback-label">{t("Correct label")}<input aria-label={t("Correct label")} value={selectedAnnotation.label} disabled={!loaded || busy} maxLength={256} onChange={(event) => {
      const label = event.target.value;
      setHistory((items) => [...items, annotations]);
      edit({ ...selectedAnnotation, label, value: selectedAnnotation.value.kind === "classification" ? { kind: "classification", labels: label.split(/[,，]/).map((item) => item.trim()).filter(Boolean) } : selectedAnnotation.value });
      setReason("wrong_target");
    }} /></label>}
    <details className="sample-feedback-decision" open={attentionOpen} onToggle={(event) => setAttentionOpen(event.currentTarget.open)}>
      <summary>{t("Result needs attention")}</summary>
      <div className="sample-feedback-fields">
      <h3>{t("Your decision on this image")}</h3>
      <p>{t("Feedback is saved to this Sample Test. It does not accept formal annotations or change the Pipeline automatically.")}</p>
      <label>{t("Result to inspect")}<select aria-label={t("Result to inspect")} value={selected ?? ""} disabled={dirty || busy || !!humanSubmission} onChange={(event) => setSelected(event.target.value || undefined)}><option value="">{t("Whole image")}</option>{annotations.map((annotation, index) => <option key={annotation.id} value={annotation.id}>{index + 1}. {annotation.label}</option>)}</select></label>
      {hint && selectedAnnotation?.value.kind === "bounding_box" && <div className="inline-notice"><p>{t("Drag corners to adjust the box, or edit its normalized coordinates below.")}</p><button onClick={() => setHint(false)}>{t("Dismiss hint")}</button></div>}
      {!hint && <button onClick={() => setHint(true)}>{t("Show editing hint")}</button>}
      {selectedAnnotation?.value.kind === "bounding_box" && <div className="sample-feedback-coordinates">{selectedAnnotation.value.rect.map((value, index) => <label key={index}>{["x", "y", "width", "height"][index]}<input type="number" step="0.001" min="0" max="1" value={Number(value.toFixed(6))} disabled={!loaded || busy} onChange={(event) => {
        if (selectedAnnotation.value.kind !== "bounding_box") return;
        const rect = [...selectedAnnotation.value.rect] as [number, number, number, number]; rect[index] = Number(event.target.value);
        setHistory((items) => [...items, annotations]); edit({ ...selectedAnnotation, value: { kind: "bounding_box", rect } });
      }} /></label>)}</div>}
      <label>{t("What needs attention?")}<select aria-label={t("What needs attention?")} value={reason} disabled={!loaded || busy || Boolean(humanSubmission?.additionId)} onChange={(event) => { setReason(event.target.value as SampleFeedbackRevision["reason"]); setDirty(true); setSaved(false); }}>{reasons.filter(([value])=>!humanSubmission || humanSubmission.additionId || value!=="missing_target").map(([value, label]) => <option key={value} value={value}>{t(label)}</option>)}</select></label>
      <label>{t("Feedback note")}<textarea aria-label={t("Feedback note")} maxLength={4000} disabled={!loaded || busy} value={note} onChange={(event) => { setNote(event.target.value); setDirty(true); setSaved(false); }} /></label>
      <div className="button-row">{!humanSubmission && <button disabled={!loaded || busy} onClick={() => void save()}>{t("Save sample feedback")}</button>}<button disabled={!history.length || busy} onClick={() => { const previous=history.at(-1)!;setAnnotations(previous); setHistory((items) => items.slice(0, -1)); setDirty(!humanSubmission?.additionId || previous.some(value=>value.provenance.addition_id===humanSubmission.additionId)); }}>{t("Undo edit")}</button></div>
      {reason !== "correct" && <p>{t("This records a quality issue, not a promised improvement. Review the existing Pipeline or correct the result manually; a new model test requires separate authorization.")}</p>}
      {onImprove && <button disabled={!loaded || busy || dirty || !revisions.length || freshness.status !== "current"} onClick={() => {
        if (copying.current) return;
        copying.current = true;
        setBusy(true); setError("");
        void api.copySamplePlan(projectId, copyKey.current, testId).then((draft) => { if (mounted.current) onImprove(draft.id, testId, image.image_id); }).catch((error: Error) => { if (mounted.current) setError(error.message); }).finally(() => { copying.current = false; if (mounted.current) setBusy(false); });
      }}>{t("Review an adjustment using saved feedback")}</button>}
      </div>
    </details>
    <div className="sample-confirm-action">
      {navigation}
      {before && <button disabled={busy || dirty} onClick={() => onKeepOriginal(before.draftId, before.testId, image.image_id)}>{t("Keep original plan")}</button>}
      <span>{t(selected ? "This decision applies only to the selected result." : "This decision applies to this sample image only.")}</span>
      {humanSubmission && <p>Submitting saves this Sandbox correction and prepares a separate revision Draft. It does not call a model or change the tested plan.</p>}
      {humanSubmission?.additionId && !referenceReady && <p role="status">Add a reference target, then adjust its box or enter its category. The initial editing placeholder cannot be submitted.</p>}
      <button className={onAdopt ? undefined : "primary"} disabled={!loaded || busy || showBefore || !sample.projection || !referenceReady} onClick={() => void save(!humanSubmission)}>{humanSubmission?.continueAfterSave ? "Submit correction and continue" : humanSubmission?.additionId ? "Submit reference target" : humanSubmission ? "Submit correction" : t(selected ? "Confirm selected result" : onConfirmed ? "Confirm sample and next" : "Confirm this sample")}</button>
      {onAdopt && <button className="primary" disabled={!loaded || busy || dirty || showBefore || !sample.projection || freshness.status !== "current"} onClick={onAdopt}>{t("Continue with this plan")}</button>}
      {saved && <span role="status">{t("Sample feedback saved")}</span>}
      {error && <p role="alert">{error}</p>}
    </div>
  </div>;
}
