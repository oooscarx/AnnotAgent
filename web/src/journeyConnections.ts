import type { ExpertPluginInstallation, InstalledModelBundle, InstalledModelInstance, ModelCapability, ModelInstanceProfile, ProviderProfile, RegistryModelProfile } from "./types";

export type JourneyConnectionPurpose = "planning" | "vision";

export function journeyVisionCapabilities(kind: string): ModelCapability[] {
  if (kind === "classification") return ["image_classification", "vision_language"];
  if (kind === "semantic_mask") return ["semantic_segmentation", "instance_segmentation"];
  return ["object_detection", "open_vocabulary_detection", "phrase_grounding", "vision_language"];
}

export function journeyModelMatches(model: RegistryModelProfile, purpose: JourneyConnectionPurpose, kind: string): boolean {
  if (!model.enabled) return false;
  if (purpose === "planning") return model.input_modalities.includes("text") && model.task_capabilities.includes("text_generation") && model.protocol_features.tool_calls && model.protocol_features.structured_output;
  return model.input_modalities.includes("image") && journeyVisionCapabilities(kind).some((capability) => model.task_capabilities.includes(capability));
}

export function journeyConnectionReady(model: RegistryModelProfile, providers: ProviderProfile[]): boolean {
  const provider = providers.find((item) => item.id === model.provider_id);
  return model.status === "available" && !!provider?.enabled && provider.adapter !== "mock" && provider.credential_configured && ["available", "configured"].includes(provider.health.status);
}

export function journeyReadyLocalModels(profiles: ModelInstanceProfile[], instances: InstalledModelInstance[], plugins: ExpertPluginInstallation[], bundles: InstalledModelBundle[]): ModelInstanceProfile[] {
  return profiles.filter((profile) => {
    if (!profile.selectable || profile.availability !== "available") return false;
    const instance = instances.find((item) => item.id === profile.model_instance_id && item.status === "ready" && item.model_profile_revision === profile.model_profile_revision);
    if (!instance?.contract_inspection.valid || instance.smoke_test_result?.status !== "passed") return false;
    return plugins.some((plugin) => plugin.enabled && plugin.manifest.id === instance.plugin_id && plugin.manifest.version === instance.plugin_version && plugin.package_sha256 === instance.plugin_package_sha256)
      && bundles.some((bundle) => bundle.enabled && bundle.manifest.publishable && !bundle.manifest.fixture && bundle.manifest.id === instance.model_bundle_id && bundle.manifest.version === instance.model_bundle_version && bundle.bundle_sha256 === instance.model_bundle_sha256);
  });
}

export function journeyLocalModelMatches(model: ModelInstanceProfile, kind: string): boolean {
  return model.selectable && model.availability === "available" && journeyVisionCapabilities(kind).some((capability) => model.capabilities.includes(capability));
}
