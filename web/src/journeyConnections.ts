import type { ModelCapability, ProviderProfile, RegistryModelProfile } from "./types";

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
