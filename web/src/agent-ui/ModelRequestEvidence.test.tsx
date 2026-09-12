import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { RegistryModelProfile } from "../types";
import {
  EffectiveModelRequestView,
  validateEffectiveModelRequest,
  type EffectiveModelRequest,
} from "./ModelRequestEvidence";

const model = { id: "model", revision: 7, provider_id: "provider" } as RegistryModelProfile;

const effective = (overrides: Partial<EffectiveModelRequest> = {}): EffectiveModelRequest => ({
  model_profile_id: "model",
  model_profile_revision: 7,
  provider_id: "provider",
  provider_adapter: "open_ai_compatible",
  endpoint_summary: "models.example.test",
  remote_model_id: "vision-pro",
  context_tokens: 32_768,
  requested_maximum_output_tokens: 1_024,
  effective_maximum_output_tokens: 1_024,
  maximum_input_context_tokens: 31_744,
  temperature: 0.1,
  top_p: "0.9",
  structured_output_mode: "tool",
  image_detail: "high",
  system_prompt_version: "demo-v1",
  reasoning: {
    requested_mode: "medium",
    supported_modes: ["low", "medium", "high"],
    support_known: true,
    wire_parameter: "reasoning_effort",
    wire_value: "medium",
  },
  pricing_snapshot: {
    model_profile_id: "model",
    model_profile_revision: 7,
    pricing: {
      currency: "USD",
      input_per_million_tokens: "2",
      output_per_million_tokens: "8",
      cached_input_per_million_tokens: null,
      per_image: null,
      per_request: null,
      source: "user_configured",
      updated_at: "2026-09-12T00:00:00Z",
    },
    captured_at: "2026-09-12T00:00:01Z",
  },
  snapshot_sha256: "a".repeat(64),
  ...overrides,
});

describe("ModelRequestEvidence", () => {
  it("renders context, effective output, mapped reasoning and immutable pricing revision", () => {
    const html = renderToStaticMarkup(<EffectiveModelRequestView value={effective()} />);
    for (const text of ["32768", "31744", "1024", "reasoning_effort=medium", "low、medium、high", "USD 2", "USD 8", "Model Profile r7"])
      expect(html).toContain(text);
    expect(html).toContain("不会发送模型请求");
    expect(html).not.toContain("API key");
  });

  it("keeps unknown prices and modes explicit instead of displaying zero", () => {
    const value = effective({
      reasoning: { requested_mode: null, supported_modes: [], support_known: false, wire_parameter: null, wire_value: null },
      pricing_snapshot: {
        ...effective().pricing_snapshot,
        pricing: { ...effective().pricing_snapshot.pricing, input_per_million_tokens: null, output_per_million_tokens: null },
      },
    });
    const html = renderToStaticMarkup(<EffectiveModelRequestView value={value} />);
    expect(html).toContain("实际可选模式未知");
    expect(html.match(/未知（不是 0）/g)).toHaveLength(2);
  });

  it("rejects a stale or foreign effective-request projection", () => {
    expect(() => validateEffectiveModelRequest(model, effective({ model_profile_revision: 6 }))).toThrow("revision");
    expect(() => validateEffectiveModelRequest(model, effective({ provider_id: "other" }))).toThrow("revision");
    expect(validateEffectiveModelRequest(model, effective())).toMatchObject({
      status: "declared",
      supported_reasoning_modes: ["low", "medium", "high"],
      maximum_output_tokens: 1_024,
    });
  });
});
