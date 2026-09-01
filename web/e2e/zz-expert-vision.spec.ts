import { expect, test, type APIRequestContext } from "@playwright/test";

test.describe.configure({ mode: "serial" });

const stamp = Date.now();
const projectId = `expert-vision-e2e-${stamp}`;
const workerUrl = "http://127.0.0.1:8796";
let originalSettings: Record<string, unknown>;

function worker(
  modelId: string,
  displayName: string,
  capabilities: string[],
  scoreSemantics = "relative_confidence",
) {
  return {
    id: `e2e-${modelId}`,
    display_name: displayName,
    model_id: modelId,
    base_url: workerUrl,
    authentication_reference: null,
    enabled: false,
    allow_remote: false,
    requires_checkpoint_metadata: true,
    expected_capabilities: capabilities,
    score_semantics: scoreSemantics,
    version: {
      architecture: null,
      model_version: "unconfigured",
      checkpoint_sha256: null,
      training_dataset_version: null,
      backend_protocol_version: "1",
    },
    label_space: [],
    runtime_requirements: {
      devices: ["cpu"],
      minimum_gpu_memory_mb: null,
      dependencies: [],
      supports_batch: false,
    },
    license: {
      code_license: null,
      weight_license: null,
      source_url: null,
      commercial_use: "unknown",
      redistribution: "unknown",
      usage_notes: [],
      verified_from_official_source: false,
    },
    timeout_seconds: 10,
    max_request_bytes: 20_000_000,
    max_response_bytes: 4_000_000,
    max_retries: 0,
    cost_per_request: "0",
    availability: "missing_weights",
    availability_evidence: {
      health_passed: false,
      protocol_compatible: false,
      contracts_validated: false,
      sample_conversion_passed: false,
      weights_ready: false,
      checked_at: null,
      detail: null,
    },
  };
}

async function settings(request: APIRequestContext) {
  const response = await request.get("/api/settings");
  expect(response.ok()).toBeTruthy();
  return response.json() as Promise<Record<string, any>>;
}

async function saveSettings(request: APIRequestContext, value: Record<string, unknown>) {
  const response = await request.put("/api/settings", { data: value });
  if (!response.ok()) throw new Error(`settings save failed: ${await response.text()}`);
  return response.json() as Promise<Record<string, any>>;
}

test.beforeAll(async ({ request }, testInfo) => {
  originalSettings = await settings(request);
  const created = await request.post("/api/projects", {
    data: {
      id: projectId,
      yaml: `version: 1
project:
  name: Expert Vision E2E ${stamp}
  language: en
dataset:
  root: images
runtime:
  max_parallel_images: 1
tasks:
  - id: objects
    display_name: Objects
    kind: bounding_box
    labels: [football]
    required: true
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native, coco, yolo]
`,
    },
  });
  expect(created.status()).toBe(201);
  const imported = await request.post(`/api/projects/${projectId}/import`, {
    data: { source: String(testInfo.config.metadata.e2eImport) },
  });
  expect(imported.ok()).toBeTruthy();
});

test.afterAll(async ({ request }) => {
  if (originalSettings) await saveSettings(request, originalSettings);
});

test("guided SAM registration requires discovery, immutable identity, and a typed Artifact conversion", async ({ page, request }) => {
  await page.goto("/settings/vision-workers");
  const originalSam = page.locator("article", { hasText: "sam2.1-hiera-tiny" });
  await expect(originalSam).toContainText("Availability · missing weights");
  await expect(originalSam.getByRole("checkbox", { name: "Enabled" })).toBeDisabled();

  await page.getByRole("button", { name: "Add expert model" }).click();
  const dialog = page.getByRole("dialog", { name: "Choose an integration" });
  await expect(dialog).toBeVisible();
  await expect(dialog.locator("label", { hasText: /^Preset/ }).locator("select")).toHaveValue("sam");
  await dialog.getByRole("button", { name: "Continue" }).click();
  await page.getByLabel("Endpoint", { exact: true }).fill(workerUrl);
  await page.getByRole("button", { name: "Save and discover" }).click();

  await expect(page.getByRole("dialog", { name: "Discover live capabilities" })).toContainText("Discovery passed");
  await expect(page.getByRole("dialog", { name: "Discover live capabilities" })).toContainText("prompted segmentation");
  await page.getByRole("button", { name: "Configure identity" }).click();
  const identityDialog = page.getByRole("dialog", { name: "Complete model identity" });
  await expect(identityDialog.getByLabel("Version", { exact: true })).toHaveValue("e2e-contract-v1");
  await expect(identityDialog.getByLabel("Checkpoint SHA-256", { exact: true })).toHaveValue("a".repeat(64));
  await expect(identityDialog.getByLabel("Checkpoint license", { exact: true })).toHaveValue("test-only deterministic fixture");
  await identityDialog.getByRole("button", { name: "Save identity and test" }).click();

  const sampleDialog = page.getByRole("dialog", { name: "Run a selected-image sample" });
  await expect(sampleDialog).toContainText(`Expert Vision E2E ${stamp}`);
  await sampleDialog.getByRole("button", { name: "Run sample test" }).click();
  await expect(sampleDialog).toContainText("Sample conversion passed");
  await expect(sampleDialog).toContainText("refined geometry");
  await sampleDialog.getByText("Converted Artifact and coordinates").click();
  await expect(sampleDialog).toContainText("mask_set");
  await expect(sampleDialog).toContainText("e2e-mask");
  await page.getByRole("button", { name: "Review registration" }).click();
  await expect(page.getByRole("dialog", { name: "Register the Expert Model" })).toContainText("Ready to register");
  await page.getByRole("button", { name: "Register Expert Model" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);

  const saved = await settings(request);
  const sam = saved.detection_workers.find((candidate: any) => candidate.model_id === "sam2.1-hiera-tiny");
  expect(sam.enabled).toBeTruthy();
  expect(sam.availability_evidence).toMatchObject({
    health_passed: true,
    protocol_compatible: true,
    contracts_validated: true,
    sample_conversion_passed: true,
    weights_ready: true,
  });
});

test("generic, YOLO, RF-DETR, and LocateAnything Workers share one discovery and sample protocol", async ({ request }) => {
  const profiles = [
    worker("e2e-generic-detector", "Generic Expert Detector", ["object_detection"]),
    worker("yolo-specialist", "YOLO Contract Fixture", ["object_detection"]),
    worker("rfdetr-specialist", "RF-DETR Contract Fixture", ["object_detection"]),
    worker("locate-anything", "LocateAnything Contract Fixture", ["open_vocabulary_detection", "phrase_grounding"], "not_provided"),
  ];
  const current = await settings(request);
  const profileIds = new Set(profiles.map((profile) => profile.model_id));
  await saveSettings(request, {
    ...current,
    detection_workers: [
      ...current.detection_workers.filter((candidate: any) => !profileIds.has(candidate.model_id)),
      ...profiles,
    ],
  });

  for (const profile of profiles) {
    const discovery = await request.post(`/api/models/${encodeURIComponent(profile.model_id)}/test`);
    if (!discovery.ok()) throw new Error(`${profile.model_id} discovery failed: ${await discovery.text()}`);
    const discovered = await discovery.json();
    expect(discovered.passed).toBeTruthy();
    expect(discovered.evidence).toMatchObject({
      health_passed: true,
      protocol_compatible: true,
      contracts_validated: true,
      weights_ready: true,
      sample_conversion_passed: false,
    });

    const sample = await request.post(`/api/models/${encodeURIComponent(profile.model_id)}/sample-test`, {
      data: { project_id: projectId, image_index: 0, query: "football" },
    });
    if (!sample.ok()) throw new Error(`${profile.model_id} sample failed: ${await sample.text()}`);
    const result = await sample.json();
    expect(result.passed).toBeTruthy();
    expect(result.converted_artifacts).toHaveLength(1);
    expect(result.coordinates).toHaveLength(1);
    expect(result.evidence.sample_conversion_passed).toBeTruthy();
  }

  const tested = await settings(request);
  for (const profile of profiles) {
    const persisted = tested.detection_workers.find((candidate: any) => candidate.model_id === profile.model_id);
    persisted.enabled = true;
  }
  await saveSettings(request, tested);
  const models = await (await request.get("/api/models")).json();
  for (const profile of profiles) {
    const registered = models.models.find((candidate: any) => candidate.id === profile.model_id);
    expect(registered.availability_group, profile.model_id).toBe("ready");
  }
});

test("Vision Workers product surface exposes status and score semantics without claiming fixture accuracy", async ({ page }) => {
  await page.goto("/settings/vision-workers");
  const locate = page.locator("article", { hasText: "LocateAnything Contract Fixture" });
  await expect(locate).toContainText("Availability · available");
  await expect(locate).toContainText("Score · not provided");
  await expect(locate.getByRole("checkbox", { name: "Enabled" })).toBeChecked();
  await expect(page.locator("body")).not.toContainText("real model accuracy");
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
});
