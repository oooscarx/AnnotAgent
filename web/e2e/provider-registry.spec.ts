import { expect, test } from "@playwright/test";

test.describe.configure({ mode: "serial" });

const projectId = `registry-project-${Date.now()}`;

test("Provider Registry configures an offline Provider, Model Profile, and confirmed usage", async ({ page }) => {
  await page.goto("/settings");
  await expect(page.getByRole("heading", { name: "Providers", exact: true })).toBeVisible();

  const provider = page.locator(".registry-provider-card").filter({ hasText: "Mock (offline)" });
  await expect(provider).toContainText("Not required");
  await provider.getByRole("button", { name: "Check connection" }).click();
  await expect(provider.getByText("Available", { exact: true })).toBeVisible();
  await expect(provider).toContainText("without a generation request");

  await page.getByRole("button", { name: "Models", exact: true }).click();
  await page.getByRole("button", { name: "Add model" }).click();
  const modelEditor = page.locator(".registry-model-editor");
  await expect(modelEditor.getByText("Model identity", { exact: true })).toBeVisible();
  await expect(modelEditor.getByText("Pricing", { exact: true })).toBeVisible();
  const desktopIdentityLayout = await modelEditor.locator(".registry-model-identity label").evaluateAll((labels) =>
    labels.map((label) => {
      const bounds = label.getBoundingClientRect();
      return { top: Math.round(bounds.top), width: Math.round(bounds.width) };
    }),
  );
  expect(new Set(desktopIdentityLayout.map((field) => field.top)).size).toBe(1);
  expect(Math.min(...desktopIdentityLayout.map((field) => field.width))).toBeGreaterThan(180);
  expect(await modelEditor.locator(".registry-check-group .checkbox-line").evaluateAll((choices) =>
    choices.every((choice) => choice.scrollWidth <= choice.clientWidth),
  )).toBeTruthy();
  await page.getByLabel("Display name", { exact: true }).fill("Mock Pipeline Builder");
  await page.getByLabel("Remote model ID", { exact: true }).fill("mock-builder");
  await page.getByLabel("image", { exact: true }).check();
  await page.getByLabel("Tool calls", { exact: true }).check();
  await page.getByLabel("Structured output", { exact: true }).check();
  await page.getByLabel("Image classification", { exact: true }).check();
  await page.getByRole("button", { name: "Save Model Profile" }).click();
  const model = page.locator(".registry-model-card").filter({ hasText: "Mock Pipeline Builder" });
  await expect(model.getByText("Unverified", { exact: true })).toBeVisible();
  await expect(model).toContainText("revision 1");

  page.once("dialog", async (dialog) => {
    expect(dialog.message()).toContain("may incur Provider charges");
    await dialog.accept();
  });
  await model.getByRole("button", { name: "Run billable test" }).click();
  await expect(model.getByText("Available", { exact: true })).toBeVisible();

  const defaultBuilder = page.getByLabel("Default Pipeline Builder model", {
    exact: true,
  });
  await defaultBuilder.selectOption({
    label: "Mock Pipeline Builder via Mock (offline)",
  });
  const selectedDefault = await defaultBuilder.inputValue();
  await page.reload();
  await expect(
    page.getByLabel("Default Pipeline Builder model", { exact: true }),
  ).toHaveValue(selectedDefault);

  await page.getByRole("button", { name: "Usage", exact: true }).click();
  await expect(page.getByText("Mock Pipeline Builder", { exact: true })).toBeVisible();
  await expect(page.getByText("2 tokens", { exact: true })).toBeVisible();
  await expect(page.getByText("explicitly confirmed", { exact: true })).toBeVisible();
});

test("Project reuses a compatible Model Profile and restores the locked choice", async ({ page, request }, testInfo) => {
  const modelResponse = await request.get("/api/model-profiles");
  expect(modelResponse.ok()).toBeTruthy();
  const profiles = (await modelResponse.json()).models as { id: string; display_name: string }[];
  const model = profiles.find((candidate) => candidate.display_name === "Mock Pipeline Builder");
  expect(model).toBeTruthy();
  const created = await request.post("/api/projects", {
    data: {
      id: projectId,
      yaml: `version: 1
project:
  name: Registry choice project
  language: en
dataset:
  root: images
runtime: {}
tasks:
  - id: scenes
    display_name: Scene
    kind: classification
    labels: [indoor]
    required: true
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native]
`,
    },
  });
  expect(created.status()).toBe(201);
  const imported = await request.post(`/api/projects/${projectId}/import`, {
    data: { source: String(testInfo.config.metadata.e2eImport) },
  });
  expect(imported.ok()).toBeTruthy();

  await page.goto(`/projects/${projectId}/build/pipeline`);
  await expect(
    page.getByRole("heading", { name: "How AnnotAgent will label your data" }),
  ).toBeVisible();
  await expect(page.getByText("Provider setup required", { exact: true })).toBeVisible();
  await page.getByText("Project model choices", { exact: true }).click();
  const classification = page.getByLabel("Classification", { exact: true });
  const saved = page.waitForResponse((response) =>
    response.request().method() === "PUT" &&
    response.url().includes(`/api/projects/${projectId}/model-bindings`),
  );
  await classification.selectOption(model!.id);
  expect((await saved).ok()).toBeTruthy();
  await expect(classification).toHaveValue(model!.id);

  await page.reload();
  await page.getByText("Project model choices", { exact: true }).click();
  await expect(page.getByLabel("Classification", { exact: true })).toHaveValue(model!.id);
  const bindings = await (await request.get(`/api/projects/${projectId}/model-bindings`)).json();
  expect(bindings.bindings).toContainEqual(expect.objectContaining({
    role: "classification",
    model_profile_id: model!.id,
    locked: true,
  }));
});

test("Legacy compatibility configuration imports once without moving a secret or history", async ({ page, request }) => {
  const before = await request.get("/api/registry-migrations/legacy");
  expect(before.ok()).toBeTruthy();
  const preview = (await before.json()).migration as {
    already_applied: boolean;
    moves_secret: boolean;
    modifies_historical_runs: boolean;
    project_binding_count: number;
  };
  expect(preview.moves_secret).toBe(false);
  expect(preview.modifies_historical_runs).toBe(false);
  expect(preview.project_binding_count).toBeGreaterThan(0);

  await page.goto("/settings/providers");
  if (!preview.already_applied) {
    const legacyImport = page.locator(".legacy-registry-import");
    await expect(legacyImport).not.toHaveAttribute("open", "");
    await legacyImport.locator("summary").click();
    await expect(legacyImport.getByText("Current Providers continue to work if you leave this untouched.")).toBeVisible();
    page.once("dialog", async (dialog) => {
      expect(dialog.message()).toContain("credential value and historical Runs will not be moved");
      await dialog.accept();
    });
    await page.getByRole("button", { name: "Review and import" }).click();
    await expect(page.getByText(/Imported Provider and Model Profile/)).toBeVisible();
  }
  const after = await request.get("/api/registry-migrations/legacy");
  expect(after.ok()).toBeTruthy();
  expect((await after.json()).migration.already_applied).toBe(true);
  await page.reload();
  await expect(page.getByRole("button", { name: "Review and import" })).toHaveCount(0);

  const repeated = await request.post("/api/registry-migrations/legacy", {
    data: { confirmed: true },
  });
  expect(repeated.ok()).toBeTruthy();
  const report = await repeated.json();
  expect(report.migration.already_applied).toBe(true);
  expect(report.secret_moved).toBe(false);
  expect(report.historical_runs_modified).toBe(false);
});

test("Provider lifecycle is reference-safe and persistent credentials stay write-only", async ({ request, page }) => {
  const listed = await request.get("/api/providers");
  expect(listed.ok()).toBeTruthy();
  const providers = (await listed.json()).providers as {
    id: string;
    display_name: string;
    enabled: boolean;
  }[];
  const mock = providers.find((provider) => provider.display_name === "Mock (offline)");
  let remote = providers.find((provider) => provider.display_name === "Remote lifecycle fixture");
  if (!remote) {
    const created = await request.post("/api/providers", {
      data: {
        display_name: "Remote lifecycle fixture",
        preset_id: "custom",
        adapter: "open_ai_compatible",
        base_url: "https://provider.invalid/v1",
      },
    });
    expect(created.ok()).toBeTruthy();
    remote = await created.json();
  }
  expect(mock).toBeTruthy();
  expect(remote).toBeTruthy();

  const blockedDelete = await request.delete(`/api/providers/${mock!.id}`);
  expect(blockedDelete.status()).toBe(409);
  expect((await blockedDelete.json()).code).toBe("provider_in_use");

  const disabled = await request.patch(`/api/providers/${mock!.id}`, {
    data: { enabled: false },
  });
  expect(disabled.ok()).toBeTruthy();
  expect((await disabled.json()).health.status).toBe("disabled");
  const incompatible = await request.get(
    "/api/model-profiles/compatible?input_modalities=image&capabilities=image_classification&allow_unverified=true",
  );
  expect(incompatible.ok()).toBeTruthy();
  expect((await incompatible.json()).models).not.toContainEqual(
    expect.objectContaining({ provider_id: mock!.id }),
  );

  const enabled = await request.patch(`/api/providers/${mock!.id}`, {
    data: { enabled: true },
  });
  expect(enabled.ok()).toBeTruthy();
  const checked = await request.post(`/api/providers/${mock!.id}/check`);
  expect(checked.ok()).toBeTruthy();
  const compatible = await request.get(
    "/api/model-profiles/compatible?input_modalities=image&capabilities=image_classification&allow_unverified=true",
  );
  expect(compatible.ok()).toBeTruthy();
  expect((await compatible.json()).models).toContainEqual(
    expect.objectContaining({ provider_id: mock!.id }),
  );

  const rotatedSecret = "e2e-persistent-workspace-credential";
  const rotated = await request.post(`/api/providers/${remote!.id}/credential`, {
    data: { source: "workspace_file", secret: rotatedSecret },
  });
  expect(rotated.ok()).toBeTruthy();
  const rotatedBody = await rotated.json();
  expect(rotatedBody.credential_source).toBe("workspace_file");
  expect(JSON.stringify(rotatedBody)).not.toContain(rotatedSecret);
  const fetched = await request.get(`/api/providers/${remote!.id}`);
  expect(fetched.ok()).toBeTruthy();
  expect(JSON.stringify(await fetched.json())).not.toContain(rotatedSecret);

  await page.goto("/settings");
  const remoteCard = page.locator(".registry-provider-card").filter({ hasText: "Remote lifecycle fixture" });
  await remoteCard.getByText("Rotate or remove credential", { exact: true }).click();
  await expect(remoteCard.getByLabel("Storage", { exact: true })).toHaveValue("workspace_file");
  await expect(remoteCard.getByLabel("API key", { exact: true })).toBeVisible();
  await expect(remoteCard.getByText("For security, an existing key is never shown here.")).toBeVisible();
  const credentialLayout = await remoteCard.locator(".credential-editor").evaluate((element) => ({
    overflows: element.scrollWidth > element.clientWidth,
    columns: getComputedStyle(element).gridTemplateColumns.split(" ").length,
  }));
  expect(credentialLayout).toEqual({ overflows: false, columns: 1 });
});

test("Settings registry remains reachable without horizontal page overflow on a phone", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 768 });
  await page.goto("/settings");
  expect(await page.evaluate(() => document.body.scrollWidth)).toBeLessThanOrEqual(1024);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/settings");
  for (const section of ["Providers", "Models", "Vision Workers", "Storage", "Usage"]) {
    await expect(page.getByRole("button", { name: section, exact: true })).toBeVisible();
  }
  const remoteCard = page.locator(".registry-provider-card").filter({ hasText: "Remote lifecycle fixture" });
  await remoteCard.getByText("Rotate or remove credential", { exact: true }).click();
  await expect(remoteCard.locator(".credential-actions")).toHaveCSS("grid-template-columns", /\d+px/);
  await page.getByRole("button", { name: "Models", exact: true }).click();
  await page.getByRole("button", { name: "Add model" }).click();
  await expect(page.locator(".registry-model-identity")).toHaveCSS("grid-template-columns", /\d+px/);
  expect(await page.locator(".registry-model-editor").evaluate((editor) => editor.scrollWidth <= editor.clientWidth)).toBeTruthy();
  expect(await page.locator(".registry-check-group .checkbox-line").evaluateAll((choices) =>
    choices.every((choice) => choice.scrollWidth <= choice.clientWidth),
  )).toBeTruthy();
  const widths = await page.evaluate(() => ({
    body: document.body.scrollWidth,
    viewport: window.innerWidth,
  }));
  expect(widths.body).toBeLessThanOrEqual(widths.viewport);
});
