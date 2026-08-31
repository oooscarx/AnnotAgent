import { describe, expect, it, vi } from "vitest";
import { api } from "./api";

describe("API client", () => {
  it("reports server errors instead of silently accepting them", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
      ok: false,
      status: 400,
      statusText: "Bad Request",
      json: () => Promise.resolve({ error: "invalid project" }),
    }));
    await expect(api.health()).rejects.toThrow("invalid project");
    vi.unstubAllGlobals();
  });

  it("queries compatible Agent models without exposing credentials", async () => {
    const fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ models: [] }),
    });
    vi.stubGlobal("fetch", fetch);
    await api.compatibleModelProfiles({
      input_modalities: ["text"],
      capabilities: ["text_generation"],
      tool_calls: true,
      structured_output: true,
    });
    const [url] = fetch.mock.calls[0];
    expect(url).toContain("/api/model-profiles/compatible?");
    expect(url).toContain("input_modalities=text");
    expect(url).toContain("capabilities=text_generation");
    expect(url).toContain("tool_calls=true");
    expect(url.toLowerCase()).not.toContain("secret");
    expect(url.toLowerCase()).not.toContain("api_key");
    vi.unstubAllGlobals();
  });

  it("persists Project choices and sends the selected Agent Profile", async () => {
    const fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ bindings: [], draft: {} }),
    });
    vi.stubGlobal("fetch", fetch);
    await api.saveProjectModelBindings("demo", [
      {
        capability: "text_generation",
        role: "pipeline_builder",
        match_kind: "role",
        model_profile_id: "model-profile-id",
        locked: true,
      },
    ]);
    await api.suggestWorkflow(
      "demo",
      "llm",
      undefined,
      undefined,
      undefined,
      "model-profile-id",
    );
    const bindingBody = JSON.parse(fetch.mock.calls[0][1].body);
    const suggestionBody = JSON.parse(fetch.mock.calls[1][1].body);
    expect(bindingBody.bindings[0]).toMatchObject({
      role: "pipeline_builder",
      model_profile_id: "model-profile-id",
      locked: true,
    });
    expect(suggestionBody.agent_model_profile_id).toBe("model-profile-id");
    expect(JSON.stringify(fetch.mock.calls)).not.toContain("Authorization");
    vi.unstubAllGlobals();
  });
});
