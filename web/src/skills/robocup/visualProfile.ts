import labelMap from "../../../../design/annotagent-visual-system/brand/robocup/robocup-label-map.json";
import type {
  AnnotationPattern,
  AnnotationVisualSlot,
  SkillVisualProfile,
} from "../../annotationVisuals";

type LabelMapEntry = { slot: string; pattern: AnnotationPattern };

const slotNumber = (slot: string): AnnotationVisualSlot => {
  const parsed = Number(slot.replace("slot", ""));
  return (parsed >= 1 && parsed <= 8 ? parsed : 1) as AnnotationVisualSlot;
};

const labelVisuals = Object.fromEntries(
  Object.entries(labelMap.labels as Record<string, LabelMapEntry>).map(([label, visual]) => [
    label,
    { slot: slotNumber(visual.slot), pattern: visual.pattern },
  ]),
);

export const robocupVisualProfile: SkillVisualProfile = {
  skillId: "robocup",
  displayName: "RoboCup Ball",
  icon: "/brand/skills/robocup/skill-badge.svg",
  labelVisuals,
};
