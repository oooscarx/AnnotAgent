import type { Annotation } from "./types";

export function reviewLabelText(annotation: Annotation): string {
  return annotation.value.kind === "classification" ? annotation.value.labels.join(", ") : annotation.label ?? "";
}

export function withReviewLabel(annotation: Annotation, text: string): Annotation {
  if (annotation.value.kind !== "classification") return { ...annotation, label: text };
  // Keep an unfinished trailing category while typing; Core validates on save.
  const labels = text === "" ? [] : [...new Set(text.split(/[,，\n]/).map(label => label.trim()))];
  return { ...annotation, label: labels[0], value: { kind: "classification", labels } };
}
