import { describe, expect, it } from "vitest";
import { robocupVisualProfile } from "./visualProfile";

describe("RoboCup Skill visual profile", () => {
  it("loads the packaged ball mapping into a generic annotation slot", () => {
    expect(robocupVisualProfile.labelVisuals.ball).toEqual({
      slot: 4,
      pattern: "circle-label",
    });
  });
});
