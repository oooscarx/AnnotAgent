import { describe, expect, it } from "vitest";
import { visualProfilesForSkills } from "./visualProfiles";

describe("Skill visual profile registry", () => {
  it("keeps a Generic Project without Skills domain-neutral", () => {
    expect(visualProfilesForSkills([])).toEqual([]);
    expect(visualProfilesForSkills(["unregistered-generic-skill"])).toEqual([]);
  });

  it("loads a profile only for an explicitly enabled Skill", () => {
    expect(visualProfilesForSkills(["robocup"]).map((profile) => profile.skillId)).toEqual([
      "robocup",
    ]);
  });
});
