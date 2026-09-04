import { afterEach, describe, expect, it, vi } from "vitest";
import { api, resetLocalApiSessionForTests } from "./api";

afterEach(() => {
  resetLocalApiSessionForTests();
  vi.unstubAllGlobals();
});

function mutationFetch(result: unknown) {
  return vi.fn().mockImplementation((url: string) => {
    if (url === "/api/session") {
      return Promise.resolve({
        ok: true,
        status: 200,
        json: () => Promise.resolve({ csrf_token: "csrf-token" }),
      });
    }
    if (url === "/api/session/privileged-confirmation") {
      return Promise.resolve({
        ok: true,
        status: 200,
        json: () => Promise.resolve({ confirmation_token: "one-time-token" }),
      });
    }
    return Promise.resolve({
      ok: true,
      status: 200,
      json: () => Promise.resolve(result),
    });
  });
}

function apiCalls(fetch: ReturnType<typeof vi.fn>) {
  return fetch.mock.calls.filter(([url]) => !String(url).startsWith("/api/session"));
}

describe("API client", () => {
  it("reports server errors instead of silently accepting them", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
      ok: false,
      status: 400,
      statusText: "Bad Request",
      json: () => Promise.resolve({ error: "invalid project" }),
    }));
    await expect(api.health()).rejects.toThrow("invalid project");
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
  });

  it("persists Project choices and sends the selected Agent Profile", async () => {
    const fetch = mutationFetch({ bindings: [], draft: {} });
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
    const calls = apiCalls(fetch);
    const bindingBody = JSON.parse(calls[0][1].body);
    const suggestionBody = JSON.parse(calls[1][1].body);
    expect(bindingBody.bindings[0]).toMatchObject({
      role: "pipeline_builder",
      model_profile_id: "model-profile-id",
      locked: true,
    });
    expect(suggestionBody.agent_model_profile_id).toBe("model-profile-id");
    expect(JSON.stringify(fetch.mock.calls)).not.toContain("Authorization");
  });

  it("starts formal execution with only an exact Published Workflow Version", async () => {
    const fetch = mutationFetch({ run_id: "run", batch: { id: "batch" } });
    vi.stubGlobal("fetch", fetch);
    const workflow = { workflow_id: "workflow", version: 3 };
    await api.startRun("demo", workflow, "idempotent-run");
    await api.startBatch("demo", 5, workflow);
    const calls = apiCalls(fetch);
    const runBody = JSON.parse(calls[0][1].body);
    const batchBody = JSON.parse(calls[1][1].body);
    expect(runBody).toEqual(workflow);
    expect(batchBody).toEqual({ limit: 5, ...workflow });
    expect(JSON.stringify([runBody, batchBody])).not.toContain("provider");
  });

  it("restores the persisted Sample Test for a Draft", async () => {
    const fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ sample_test: null, current: false }),
    });
    vi.stubGlobal("fetch", fetch);

    await api.workflowSampleTest("draft id");

    expect(fetch).toHaveBeenCalledWith(
      "/api/workflow-drafts/draft%20id/sample-test",
      expect.objectContaining({
        headers: expect.objectContaining({ "content-type": "application/json" }),
      }),
    );
  });

  it("creates an evidence-bounded improvement without publish authority", async () => {
    const fetch = mutationFetch({ id: "improvement", status: "draft_created" });
    vi.stubGlobal("fetch", fetch);
    await api.createPipelineImprovement("generic", {
      workflow_id: "baseline",
      workflow_version: 2,
      target_task_id: "objects",
      target_label: "vehicle",
      evidence_run_ids: ["diagnosis-run"],
      evaluation_run_ids: ["holdout-run"],
    });
    await api.comparePipelineImprovement("improvement");
    await api.applyPipelineImprovement("improvement", ["add:review"]);
    const calls = apiCalls(fetch).map(([url, init]) => ({
      url,
      body: init?.body ? JSON.parse(init.body) : undefined,
    }));
    expect(calls[0]).toEqual({
      url: "/api/projects/generic/pipeline-improvements",
      body: expect.objectContaining({
        evidence_run_ids: ["diagnosis-run"],
        evaluation_run_ids: ["holdout-run"],
      }),
    });
    expect(calls[2].body).toEqual({ selected_change_ids: ["add:review"] });
    expect(JSON.stringify(calls)).not.toContain("publish");
    expect(JSON.stringify(calls)).not.toContain("api_key");
  });

  it("binds privileged requests to a one-time server confirmation", async () => {
    const fetch = mutationFetch({ provider_id: "provider", credential_configured: true });
    vi.stubGlobal("fetch", fetch);

    await api.saveProviderCredential("provider", {
      source: "workspace_file",
      secret: "write-only-test-value",
    });

    const confirmation = fetch.mock.calls.find(([url]) => url === "/api/session/privileged-confirmation");
    expect(JSON.parse(confirmation?.[1]?.body)).toEqual({
      action: "POST /api/providers/provider/credential",
      confirmed: true,
    });
    const requestCall = apiCalls(fetch)[0];
    const headers = requestCall[1].headers as Headers;
    expect(headers.get("x-annotagent-csrf")).toBe("csrf-token");
    expect(headers.get("x-annotagent-privileged-confirmation")).toBe("one-time-token");
    expect(JSON.stringify(fetch.mock.calls)).not.toContain("Authorization");
  });
});
