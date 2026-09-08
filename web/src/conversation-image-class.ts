import type { Annotation, SampleFeedbackRevision, SampleTestOutcomeRecord } from "./types";
import type { ImageClassAction, ImageClassAnswerInput, ImageClassReview, ImageClassScope } from "./conversation-image-class-api";
import { sampleAnnotations } from "./sampleAnnotations";
import { sampleFeedbackOverlay } from "./sampleFeedbackOverlay";

/** Class identity comes from frozen predictions, not a rendered label string. */
export function imageClassTokens(outcome: SampleTestOutcomeRecord): string[] {
  if (outcome.value?.kind === "bounding_box") return [outcome.label];
  if (outcome.value?.kind === "classification") return [...new Set(outcome.value.labels)];
  return [];
}
export function sameImageClass(outcome: SampleTestOutcomeRecord, token: string): boolean {
  return imageClassTokens(outcome).includes(token);
}

/** A human may change this token only; all other class labels stay intact. */
export function replaceImageClassToken(labels: string[], token: string, replacement: string | null): string[] {
  if (!labels.includes(token)) throw new Error("The selected original class is no longer present. No other class was changed.");
  if (replacement !== null && (!replacement.trim() || (replacement !== token && labels.includes(replacement)))) {
    throw new Error("Choose a non-empty replacement different from the other saved classes.");
  }
  return labels.flatMap(label => label === token ? replacement === null ? [] : [replacement] : [label]);
}

export type ImageClassLocal = { review_id: string; scope_digest: string; selected_id: string; actions: ImageClassAction[]; frozen?: ImageClassAnswerInput };
const canonical = (value: unknown) => JSON.stringify(value, (_key, item) => {
  if (!item || typeof item !== "object" || Array.isArray(item)) return item;
  // Rust stores this one typed field as NormalizedRect<f32>. Its shortest JSON
  // decimals differ from a browser's double without changing the saved command.
  // IDs, labels, budgets and all other numeric fields keep exact comparison.
  const value = item.action === "edit" && item.corrected_value?.kind === "bounding_box" && Array.isArray(item.corrected_value.rect) && item.corrected_value.rect.length === 4 && item.corrected_value.rect.every((coordinate: unknown) => typeof coordinate === "number")
    ? { ...item, corrected_value: { ...item.corrected_value, rect: item.corrected_value.rect.map(Math.fround) } } : item;
  return Object.fromEntries(Object.entries(value).sort(([a], [b]) => a.localeCompare(b)));
});
export const sameImageClassInput = (a: unknown, b: unknown) => canonical(a) === canonical(b);
export function initialImageClassLocal(review: ImageClassReview): ImageClassLocal {
  return { review_id: review.id, scope_digest: review.scope_digest, selected_id: review.scope.members[0]?.outcome.id ?? "", actions: review.answer?.actions ?? review.scope.members.map(member => ({ action: "keep", outcome_id: member.outcome.id, source_artifact_id: member.source_artifact_id })) };
}
export function imageClassAnswerConflicts(local: ImageClassLocal, review: ImageClassReview): boolean {
  return Boolean(review.answer && (local.frozen ? !sameImageClassInput(review.answer, local.frozen) : local.actions.some(item => item.action !== "keep") && !sameImageClassInput(review.answer.actions, local.actions)));
}
export function imageClassLocalNeedsGuard(local: ImageClassLocal, review: ImageClassReview): boolean {
  return imageClassAnswerConflicts(local, review) || (!review.answer && (Boolean(local.frozen) || !sameImageClassInput(local.actions, initialImageClassLocal(review).actions)));
}
export function imageClassDisplayOrigin(local: ImageClassLocal, review: ImageClassReview, showSaved: boolean): "local" | "saved" {
  return review.answer && (!imageClassAnswerConflicts(local, review) || showSaved) ? "saved" : "local";
}
export function imageClassEditValue(baseline: Annotation | undefined, action: ImageClassAction | undefined): Annotation | undefined {
  return baseline && action?.action === "edit" && action.outcome_id === baseline.id ? { ...baseline, value: action.corrected_value, label: action.corrected_label } : baseline;
}
export function validateImageClassActions(scope: ImageClassScope, actions: ImageClassAction[]): void {
  if (!Array.isArray(actions) || actions.length !== scope.members.length || new Set(actions.map(item => item.outcome_id)).size !== actions.length) throw new Error("Every frozen candidate needs exactly one decision.");
  const baseline = sampleFeedbackOverlay(sampleAnnotations(scope.members.map(member => member.outcome), scope.image_id, scope.sample_test_id), scope.baseline_feedback).annotations;
  for (const action of actions) {
    const member = scope.members.find(item => item.outcome.id === action.outcome_id && item.source_artifact_id === action.source_artifact_id);
    const original = baseline.find(item => item.id === action.outcome_id);
    if (!member || !original) throw new Error("This decision does not match a frozen candidate and Artifact.");
    if (action.action === "keep" || action.action === "exclude") continue;
    if (action.action === "replace_label" && original.value.kind === "classification") { replaceImageClassToken(original.value.labels, scope.target_label, action.replacement); continue; }
    if (action.action === "edit" && original.value.kind === "bounding_box" && action.corrected_value?.kind === "bounding_box" && typeof action.corrected_label === "string" && action.corrected_label.trim()) {
      const rect = action.corrected_value.rect.map(Math.fround);
      // Match NormalizedRect::new after serde converts each component to f32,
      // including the f32 addition used for its inside-image check.
      if (rect.length === 4 && rect.every(Number.isFinite) && rect[0] >= 0 && rect[0] <= 1 && rect[1] >= 0 && rect[1] <= 1 && rect[2] > 0 && rect[2] <= 1 && rect[3] > 0 && rect[3] <= 1 && Math.fround(rect[0] + rect[2]) <= 1 + 2 ** -23 && Math.fround(rect[1] + rect[3]) <= 1 + 2 ** -23) continue;
    }
    throw new Error("This correction is not valid for the frozen candidate type.");
  }
}
export function makeImageClassAnswer(review: ImageClassReview, local: ImageClassLocal, command: string): ImageClassAnswerInput {
  if (local.review_id !== review.id || local.scope_digest !== review.scope_digest) throw new Error("The review context changed; the original edits are retained but cannot be applied elsewhere.");
  validateImageClassActions(review.scope, local.actions);
  if (local.frozen) {
    if (local.frozen.expected_scope_digest !== review.scope_digest || !sameImageClassInput(local.actions, local.frozen.actions)) throw new Error("The original pending answer cannot be replaced.");
    return local.frozen;
  }
  return { command_id: command, expected_scope_digest: review.scope_digest, actions: local.actions };
}
export function parseImageClassLocal(raw: string | null, review: ImageClassReview): ImageClassLocal | undefined {
  try {
    if (!raw) return undefined;
    const value = JSON.parse(raw) as ImageClassLocal;
    if (!value || typeof value.selected_id !== "string" || !review.scope.members.some(item => item.outcome.id === value.selected_id)) return undefined;
    if (value.review_id !== review.id || value.scope_digest !== review.scope_digest || !Array.isArray(value.actions) || value.actions.length !== review.scope.members.length || new Set(value.actions.map(item => item.outcome_id)).size !== value.actions.length) return undefined;
    for (const action of value.actions) {
      if (!review.scope.members.some(item => item.outcome.id === action.outcome_id && item.source_artifact_id === action.source_artifact_id)) return undefined;
      if (action.action === "keep" || action.action === "exclude") continue;
      if (action.action === "replace_label" && review.scope.kind === "classification" && typeof action.replacement === "string") continue;
      if (action.action === "edit" && review.scope.kind === "bounding_box" && typeof action.corrected_label === "string" && action.corrected_value?.kind === "bounding_box" && Array.isArray(action.corrected_value.rect) && action.corrected_value.rect.length === 4 && action.corrected_value.rect.every(Number.isFinite)) continue;
      return undefined;
    }
    if (value.frozen && (typeof value.frozen.command_id !== "string" || !value.frozen.command_id)) return undefined;
    if (value.frozen) makeImageClassAnswer(review, value, "validation-only");
    return value;
  } catch { return undefined; }
}
export function mergeImageClassReview(previous: ImageClassReview | undefined, incoming: ImageClassReview): ImageClassReview {
  if (!previous || previous.id !== incoming.id) return incoming;
  if (previous.scope_digest !== incoming.scope_digest) return previous;
  if ((previous.status === "cancelled" && incoming.status === "pending") || (previous.answer && !incoming.answer) || (previous.repair_draft_id && !incoming.repair_draft_id)) return previous;
  return incoming;
}
export function imageClassOverlay(scope: ImageClassScope, actions: ImageClassAction[], allOutcomes: SampleTestOutcomeRecord[]) {
  validateImageClassActions(scope, actions);
  const original = sampleAnnotations(allOutcomes, scope.image_id, scope.sample_test_id);
  const baseline = sampleFeedbackOverlay(original, scope.baseline_feedback);
  const revisions: SampleFeedbackRevision[] = [];
  for (const action of actions) {
    if (action.action === "keep") continue;
    const item = baseline.annotations.find(value => value.id === action.outcome_id)!;
    const base: SampleFeedbackRevision = { revision_id: `local:${action.outcome_id}`, sample_test_id: scope.sample_test_id, image_id: scope.image_id, sequence: scope.baseline_sequence + revisions.length + 1, reason: "wrong_target", outcome_id: action.outcome_id, note: "Unsaved local review preview", created_at: "" };
    if (action.action === "edit") revisions.push({ ...base, reason: "poor_boundary", corrected_value: action.corrected_value, corrected_label: action.corrected_label });
    else if (item.value.kind === "classification") {
      const labels = replaceImageClassToken(item.value.labels, scope.target_label, action.action === "exclude" ? null : action.action === "replace_label" ? action.replacement : scope.target_label);
      revisions.push(labels.length ? { ...base, corrected_value: { kind: "classification", labels }, corrected_label: labels.join(", ") } : { ...base, reason: "exclude_target" });
    } else revisions.push({ ...base, reason: "exclude_target" });
  }
  return sampleFeedbackOverlay(original, [...scope.baseline_feedback, ...revisions]);
}
