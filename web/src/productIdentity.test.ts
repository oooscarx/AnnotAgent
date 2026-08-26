import { describe, expect, it } from "vitest";
import {
  NO_PROJECT_MESSAGE,
  PRIMARY_NAVIGATION,
  PRODUCT_NAME,
  PRODUCT_TAGLINE,
  activeSkills,
} from "./productIdentity";

describe("AnnotAgent product shell", () => {
  it("keeps the no-Project identity domain-neutral", () => {
    const copy = [
      PRODUCT_NAME,
      PRODUCT_TAGLINE,
      NO_PROJECT_MESSAGE,
      ...PRIMARY_NAVIGATION.map((item) => item.label),
    ].join(" ");
    expect(copy).not.toMatch(/RoboCup|football|ball|robot soccer/i);
    expect(activeSkills()).toEqual([]);
  });

  it("uses the complete product navigation", () => {
    expect(PRIMARY_NAVIGATION.map((item) => item.label)).toEqual([
      "Dashboard",
      "Projects",
      "Workflows",
      "Models",
      "Skills",
      "Runs",
      "Review",
      "Settings",
    ]);
  });
});
