import type { Annotation } from "./types";

export type AnnotationVisualSlot = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8;

export type AnnotationPattern =
  | "solid-box"
  | "dashed-box"
  | "solid-line"
  | "diagonal-fill"
  | "circle-label"
  | "crosshair";

export interface LabelVisualMapping {
  slot: AnnotationVisualSlot;
  pattern?: AnnotationPattern;
}

export interface SkillVisualProfile {
  skillId: string;
  displayName: string;
  icon?: string;
  labelVisuals: Record<string, LabelVisualMapping>;
}

export interface AnnotationVisualContext {
  projectOverrides?: Record<string, LabelVisualMapping>;
  skillProfiles?: SkillVisualProfile[];
  schemaVisuals?: Record<string, LabelVisualMapping>;
}

export interface AnnotationVisual {
  slot: AnnotationVisualSlot;
  pattern: AnnotationPattern;
  source: "project" | "skill" | "schema" | "fallback";
  sourceId?: string;
}

const stableSlot = (label: string): AnnotationVisualSlot => {
  let value = 0;
  for (const character of label) value = (value * 31 + character.charCodeAt(0)) >>> 0;
  return ((value % 8) + 1) as AnnotationVisualSlot;
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

export function annotationVisual(
  annotation: Annotation,
  context: AnnotationVisualContext = {},
): AnnotationVisual {
  const label = annotation.label ?? annotation.task_id;
  const project = context.projectOverrides?.[label];
  if (project) {
    return {
      ...project,
      pattern: project.pattern ?? defaultPattern(annotation),
      source: "project",
    };
  }

  // Skill conflicts are resolved by stable Skill id, never registration or array order.
  const skillMatch = [...(context.skillProfiles ?? [])]
    .sort((left, right) => left.skillId.localeCompare(right.skillId))
    .map((profile) => ({ profile, visual: profile.labelVisuals[label] }))
    .find((candidate) => candidate.visual);
  if (skillMatch?.visual) {
    return {
      ...skillMatch.visual,
      pattern: skillMatch.visual.pattern ?? defaultPattern(annotation),
      source: "skill",
      sourceId: skillMatch.profile.skillId,
    };
  }

  const schema = context.schemaVisuals?.[label];
  if (schema) {
    return {
      ...schema,
      pattern: schema.pattern ?? defaultPattern(annotation),
      source: "schema",
    };
  }

  return {
    slot: stableSlot(label),
    pattern: defaultPattern(annotation),
    source: "fallback",
  };
}

export const annotationColor = (slot: AnnotationVisualSlot): string => `var(--aa-annotation-${slot})`;
