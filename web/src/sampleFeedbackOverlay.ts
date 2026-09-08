import type { Annotation, SampleFeedbackRevision } from "./types";

export type ExcludedSampleCandidate = { annotation: Annotation; revision: SampleFeedbackRevision };
/** The immutable prediction stays intact. Only explicit exclusion hides a candidate. */
export function sampleFeedbackOverlay(original: Annotation[], revisions: SampleFeedbackRevision[]): { annotations: Annotation[]; excluded: ExcludedSampleCandidate[] } {
  const values = new Map(original.map(item => [item.id, item]));
  const excluded = new Map<string, SampleFeedbackRevision>();
  for (const change of revisions) {
    if (change.addition_id && change.corrected_value && change.corrected_label) {
      const id = `human-sample:${change.addition_id}`;
      values.set(id, { id, image_id: change.image_id, task_id: "sample", label: change.corrected_label, value: change.corrected_value, attributes: {}, source: "human sample feedback", review_status: "needs_review", provenance: { addition_id: change.addition_id, sample_test_id: change.sample_test_id }, created_at: change.created_at });
      continue;
    }
    const item = change.outcome_id ? values.get(change.outcome_id) : undefined;
    if (!item) continue;
    if (change.reason === "exclude_target") { excluded.set(item.id, change); continue; }
    if (change.corrected_value || change.corrected_label) {
      const value = change.corrected_value ?? item.value;
      values.set(item.id, { ...item, value, label: value.kind === "classification" ? value.labels.join(", ") : change.corrected_label ?? item.label, confidence: undefined, source: "human sample feedback", provenance: { ...item.provenance, human_corrected: true } });
      excluded.delete(item.id);
    }
  }
  return { annotations: [...values.values()].filter(item => !excluded.has(item.id)), excluded: [...excluded].map(([id, revision]) => ({ annotation: values.get(id)!, revision })) };
}
