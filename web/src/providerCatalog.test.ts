import { describe, expect, it } from "vitest";
import {
  applyProviderPreset,
  getProviderPreset,
  inferProviderPreset,
  isEnvironmentVariableName,
  isCatalogModel,
} from "./providerCatalog";

describe("provider catalog", () => {
  it("recognizes a persisted provider from its endpoint", () => {
    const preset = inferProviderPreset({
      default_provider: "openai_compatible",
      provider: { endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/" },
    });
    expect(preset.id).toBe("dashscope");
  });

  it("applies provider defaults without discarding advanced settings", () => {
    const settings = applyProviderPreset({
      default_provider: "mock",
      provider: { endpoint: "https://old.invalid/v1", max_output_tokens: 8192 },
      budget: { max_requests: 10 },
    }, "gemini");
    expect(settings.default_provider).toBe("openai_compatible");
    expect(settings.provider?.endpoint).toBe("https://generativelanguage.googleapis.com/v1beta/openai");
    expect(settings.provider?.model).toBe("gemini-3.7-flash");
    expect(settings.provider?.max_output_tokens).toBe(8192);
    expect(settings.budget).toEqual({ max_requests: 10 });
  });

  it("keeps custom gateways editable and supports unlisted models", () => {
    const custom = applyProviderPreset({
      default_provider: "mock",
      provider: { endpoint: "https://gateway.example/v1", model: "private-vision" },
    }, "custom");
    expect(custom.provider?.endpoint).toBe("https://gateway.example/v1");
    expect(custom.default_provider).toBe("openai_compatible");
    expect(isCatalogModel(getProviderPreset("dashscope"), "private-vision")).toBe(false);
  });

  it("distinguishes an environment variable name from a pasted API key", () => {
    expect(isEnvironmentVariableName("DASHSCOPE_API_KEY")).toBe(true);
    expect(isEnvironmentVariableName("_PRIVATE_GATEWAY_TOKEN_2")).toBe(true);
    expect(isEnvironmentVariableName("not-a-variable-name")).toBe(false);
    expect(isEnvironmentVariableName("123_API_KEY")).toBe(false);
  });
});
