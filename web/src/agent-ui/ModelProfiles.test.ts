import { describe, expect, it } from "vitest";
import type { RegistryModelProfile } from "../types";
import { modelMatchesScope } from "./ModelProfiles";

const base = {
  input_modalities: ["text"],
  task_capabilities: ["text_generation"],
} as RegistryModelProfile;

describe("Model Profile Settings scopes", () => {
  it("keeps planning and visual responsibilities explicit", () => {
    expect(modelMatchesScope(base, "agent")).toBe(true);
    expect(modelMatchesScope(base, "vision")).toBe(false);
    const vision = {
      ...base,
      input_modalities: ["text", "image"],
      task_capabilities: ["text_generation", "vision_language"],
    } as RegistryModelProfile;
    expect(modelMatchesScope(vision, "agent")).toBe(true);
    expect(modelMatchesScope(vision, "vision")).toBe(true);
  });
});
