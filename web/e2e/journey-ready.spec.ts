import { resolve } from "node:path";
import { expect, test } from "./fixtures";

test("ready fixture journey plans without image calls then authorizes a bounded sandbox test", async ({ page, request }) => {
  test.setTimeout(120_000);
  const provider = await (await request.post("/api/providers", { data: { display_name: "Journey TEST fixture", adapter: "open_ai_compatible", base_url: "http://127.0.0.1:8796/openai/v1" } })).json();
  expect((await request.post(`/api/providers/${provider.id}/credential`, { data: { source: "workspace_file", secret: "guided-e2e-protocol-fixture" } })).ok()).toBeTruthy();
  const model = await (await request.post("/api/model-profiles", { data: {
    provider_id: provider.id, display_name: "Journey TEST model", remote_model_id: "e2e-pipeline-builder",
    input_modalities: ["text", "image"], task_capabilities: ["text_generation", "vision_language", "image_classification"], protocol_features: { tool_calls: true, structured_output: true },
  } })).json();
  expect((await request.post(`/api/providers/${provider.id}/active-probe`, { data: { model_profile_id: model.id, confirmed_billable: true } })).ok()).toBeTruthy();
  const defaults = await (await request.get("/api/agent-model-bindings")).json();
  expect((await request.put("/api/agent-model-bindings", { data: { ...defaults, pipeline_builder: model.id } })).ok()).toBeTruthy();
  const sampleRequests: unknown[] = [];
  page.on("request", (req) => { if (req.method() === "POST" && req.url().endsWith("/dry-run")) sampleRequests.push(req.postDataJSON()); });
  await page.goto("/projects?new=1");
  await page.getByLabel("Choose images", { exact: true }).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(page).toHaveURL(/\/task\/goal$/);
  const projectId = new URL(page.url()).pathname.split("/")[2];
  expect((await request.put(`/api/projects/${projectId}/model-bindings`, { data: { bindings: [{ capability: "image_classification", role: "classification", match_kind: "capability", model_profile_id: model.id, locked: false }] } })).ok()).toBeTruthy();
  await page.getByLabel("Describe your goal", { exact: true }).fill("Classify the scene as day.");
  await page.getByRole("radio", { name: /Image categories/ }).check();
  await page.getByLabel("Categories to keep", { exact: true }).fill("day");
  await page.getByRole("button", { name: "Prepare sample results", exact: true }).click();
  await expect(page.getByRole("region", { name: "Planning authorization" })).toBeVisible();
  expect(sampleRequests).toEqual([]);
  await page.getByRole("button", { name: "Authorize planning", exact: true }).click();
  await expect(page).toHaveURL(/\/task\/samples\?draft=/, { timeout: 60_000 });
  await expect(page.getByRole("region", { name: "Sample authorization" })).toBeVisible();
  const draftId = new URL(page.url()).searchParams.get("draft")!;
  const sessions = await (await request.get(`/api/projects/${projectId}/agent-sessions`)).json();
  expect(sessions.sessions[0].builder_constraints.maximum_dry_runs).toBe(0);
  expect(sessions.sessions[0].steps.filter((step: { tool_name: string; success: boolean }) => step.tool_name === "dry_run_pipeline" && step.success)).toHaveLength(0);
  const preview = await (await request.get(`/api/workflow-drafts/${draftId}/sample-preview`)).json();
  expect(preview.supported, JSON.stringify(preview)).toBe(true);
  expect(preview.request_limit).toBe(12);
  const stale = await request.post(`/api/workflow-drafts/${draftId}/dry-run`, { data: { image_indices: [0], expected_revision: preview.revision, authorization_fingerprint: "stale-scope" } });
  expect(stale.status()).toBe(400);
  await page.getByRole("checkbox", { name: "I reviewed the sample scope" }).check();
  await page.getByRole("button", { name: "Test samples", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Does this result match what you need?" })).toBeVisible();
  expect(sampleRequests).toHaveLength(1);
  await expect(page.locator(".sample-feedback-image svg image")).toBeVisible();
  await expect(page.locator(".sample-technical-details, .sample-diagnostics, .workflow-edit-details, .build-steps, .sidebar, .focus-project-menu")).toHaveCount(0);
  const resultUrl = page.url();
  await page.reload();
  await expect(page.locator(".sample-feedback-image svg image")).toBeVisible();
  await expect(page).toHaveURL(resultUrl);
  expect(sampleRequests).toHaveLength(1);
  for (const [width, height] of [[1440, 900], [1280, 720], [1024, 768], [390, 844]]) {
    await page.setViewportSize({ width, height });
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    await page.screenshot({ path: `../docs/execution/guided-journey/sample-${width}.png`, fullPage: true });
  }
  const annotations = await (await request.get(`/api/projects/${projectId}/export-readiness`)).json();
  expect(JSON.stringify(annotations)).not.toContain("human_accepted");
});
