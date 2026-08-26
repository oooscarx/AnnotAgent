import labelMap from "../../design/annotagent-visual-system/brand/robocup/robocup-label-map.json";
import type { Annotation } from "./types";

export type AnnotationPattern =
  | "solid-box"
  | "dashed-box"
  | "solid-line"
  | "diagonal-fill"
  | "circle-label"
  | "crosshair";

export type AnnotationVisual = {
  slot: number;
  pattern: AnnotationPattern;
};

type LabelMapEntry = { slot: string; pattern: AnnotationPattern };

const ROBOCUP_LABELS = labelMap.labels as Record<string, LabelMapEntry>;

const slotNumber = (slot: string): number => {
  const parsed = Number(slot.replace("slot", ""));
  return parsed >= 1 && parsed <= 8 ? parsed : 1;
};

const stableSlot = (label: string): number => {
  let value = 0;
  for (const character of label) value = (value * 31 + character.charCodeAt(0)) >>> 0;
  return (value % 8) + 1;
};

const defaultPattern = (annotation: Annotation): AnnotationPattern => {
  switch (annotation.value.kind) {
    case "polyline":
      return "solid-line";
    case "polygon":
    case "instance_mask":
      return "diagonal-fill";
    case "keypoints":
      return "crosshair";
    default:
      return "solid-box";
  }
};

export function annotationVisual(annotation: Annotation, skillId?: string): AnnotationVisual {
  const label = annotation.label ?? annotation.task_id;
  const mapped = skillId === "robocup" ? ROBOCUP_LABELS[label] : undefined;
  return {
    slot: mapped ? slotNumber(mapped.slot) : stableSlot(label),
    pattern: mapped?.pattern ?? defaultPattern(annotation),
  };
}

export const annotationColor = (slot: number): string => `var(--aa-annotation-${slot})`;
