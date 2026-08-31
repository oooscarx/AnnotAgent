import { expect, test } from "@playwright/test";

test.describe.configure({ mode: "serial" });

const projectId = `registry-project-${Date.now()}`;

test("Provider Registry configures an offline Provider, Model Profile, and confirmed usage", async ({ page }) => {
  await page.goto("/settings");
  await expect(page.getByRole("heading", { name: "Providers", exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Add provider" }).click();
  await page.getByRole("button", { name: "Save Provider" }).click();
  const provider = page.locator(".registry-provider-card").filter({ hasText: "Mock (offline)" });
  await expect(provider).toContainText("Not required");
  await provider.getByRole("button", { name: "Check connection" }).click();
  await expect(provider.getByText("Available", { exact: true })).toBeVisible();
  await expect(provider).toContainText("without a generation request");

  await page.getByRole("button", { name: "Models", exact: true }).click();
  await page.getByRole("button", { name: "Add model" }).click();
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

test("Settings registry remains reachable without horizontal page overflow on a phone", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 768 });
  await page.goto("/settings");
  expect(await page.evaluate(() => document.body.scrollWidth)).toBeLessThanOrEqual(1024);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/settings/usage");
  for (const section of ["Providers", "Models", "Vision Workers", "Storage", "Usage"]) {
    await expect(page.getByRole("button", { name: section, exact: true })).toBeVisible();
  }
  const widths = await page.evaluate(() => ({
    body: document.body.scrollWidth,
    viewport: window.innerWidth,
  }));
  expect(widths.body).toBeLessThanOrEqual(widths.viewport);
});
