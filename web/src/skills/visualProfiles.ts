import type { SkillVisualProfile } from "../annotationVisuals";
import { robocupVisualProfile } from "./robocup/visualProfile";

const PROFILES = new Map<string, SkillVisualProfile>([
  [robocupVisualProfile.skillId, robocupVisualProfile],
]);

export function visualProfilesForSkills(skillIds: string[]): SkillVisualProfile[] {
  return [...new Set(skillIds)]
    .sort()
    .map((skillId) => PROFILES.get(skillId))
    .filter((profile): profile is SkillVisualProfile => Boolean(profile));
}
