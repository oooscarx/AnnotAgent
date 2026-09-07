import { describe, expect, it } from "vitest";
import { journeyLocalModelMatches, journeyModelMatches, journeyReadyLocalModels, journeyVisionCapabilities } from "./journeyConnections";
import type { ExpertPluginInstallation, InstalledModelBundle, InstalledModelInstance, ModelInstanceProfile, RegistryModelProfile } from "./types";

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
  it("requires the same enabled plugin and publishable bundle as the tested local instance", () => {
    const profile = { selectable: true, availability: "available", model_instance_id: "instance", model_profile_revision: 2, capabilities: ["object_detection"] } as ModelInstanceProfile;
    const instance = { id: "instance", status: "ready", model_profile_revision: 2, plugin_id: "plugin", plugin_version: "1", plugin_package_sha256: "plugin-digest", model_bundle_id: "bundle", model_bundle_version: "1", model_bundle_sha256: "bundle-digest", contract_inspection: { valid: true }, smoke_test_result: { status: "passed" } } as InstalledModelInstance;
    const plugin = { enabled: true, manifest: { id: "plugin", version: "1" }, package_sha256: "plugin-digest" } as ExpertPluginInstallation;
    const bundle = { enabled: true, manifest: { id: "bundle", version: "1", publishable: true, fixture: false }, bundle_sha256: "bundle-digest" } as InstalledModelBundle;
    expect(journeyReadyLocalModels([profile], [instance], [plugin], [bundle])).toEqual([profile]);
    expect(journeyLocalModelMatches(profile, "bounding_box")).toBe(true);
    expect(journeyLocalModelMatches({ ...profile, capabilities: ["prompted_segmentation"] }, "bounding_box")).toBe(false);
    for (const unavailable of [{ ...profile, selectable: false }, { ...profile, availability: "unknown" }, { ...profile, model_profile_revision: 3 }]) expect(journeyReadyLocalModels([unavailable], [instance], [plugin], [bundle])).toEqual([]);
    expect(journeyReadyLocalModels([profile], [instance], [{ ...plugin, enabled: false }], [bundle])).toEqual([]);
    expect(journeyReadyLocalModels([profile], [instance], [{ ...plugin, package_sha256: "other" }], [bundle])).toEqual([]);
    expect(journeyReadyLocalModels([profile], [{ ...instance, status: "failed" }], [plugin], [bundle])).toEqual([]);
    for (const unavailable of [{ ...bundle, enabled: false }, { ...bundle, bundle_sha256: "other" }, { ...bundle, manifest: { ...bundle.manifest, fixture: true } }, { ...bundle, manifest: { ...bundle.manifest, publishable: false } }]) expect(journeyReadyLocalModels([profile], [instance], [plugin], [unavailable])).toEqual([]);
  });
});
