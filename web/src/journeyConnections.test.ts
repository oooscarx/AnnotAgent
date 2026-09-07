import { describe, expect, it } from "vitest";
import { journeyModelMatches, journeyVisionCapabilities } from "./journeyConnections";
import type { RegistryModelProfile } from "./types";

describe("task-specific model preparation", () => {
  const profile = { enabled: true, input_modalities: ["text"], task_capabilities: ["text_generation"], protocol_features: { tool_calls: true, structured_output: true } } as RegistryModelProfile;
  it("does not treat a text planner as a vision model", () => {
    expect(journeyModelMatches(profile, "planning", "bounding_box")).toBe(true);
    expect(journeyModelMatches(profile, "vision", "bounding_box")).toBe(false);
  });
  it("does not treat a prompted refiner as a whole-image detector or segmenter", () => {
    const refiner = { ...profile, input_modalities: ["image"], task_capabilities: ["prompted_segmentation"] } as RegistryModelProfile;
    expect(journeyModelMatches(refiner, "vision", "bounding_box")).toBe(false);
    expect(journeyModelMatches(refiner, "vision", "semantic_mask")).toBe(false);
  });
  it("accepts supported VLM tasks without pretending they provide pixel segmentation", () => {
    const vlm = { ...profile, input_modalities: ["image"], task_capabilities: ["vision_language"] } as RegistryModelProfile;
    expect(journeyModelMatches(vlm, "vision", "classification")).toBe(true);
    expect(journeyModelMatches(vlm, "vision", "bounding_box")).toBe(true);
    expect(journeyModelMatches(vlm, "vision", "semantic_mask")).toBe(false);
    expect(journeyVisionCapabilities("semantic_mask")).not.toContain("vision_language");
  });
});
