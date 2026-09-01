import { expect, test } from "@playwright/test";

test("product workspace exposes only live Provider and Advisor paths", async ({
  page,
  request,
}) => {
  const providers = await request.get("/api/providers");
  expect(providers.ok()).toBeTruthy();
  const providerPayload = await providers.json();
  expect(
    providerPayload.providers.every(
      (provider: { adapter: string; display_name: string; preset_id?: string }) =>
        provider.adapter === "open_ai_compatible" &&
        provider.preset_id !== "mock" &&
        !provider.display_name.toLowerCase().includes("mock"),
    ),
  ).toBeTruthy();

  const presets = await request.get("/api/provider-presets");
  expect(presets.ok()).toBeTruthy();
  const presetPayload = await presets.json();
  expect(
    presetPayload.presets.every(
      (preset: { adapter: string }) => preset.adapter === "open_ai_compatible",
    ),
  ).toBeTruthy();

  const rejected = await request.post("/api/workflow-drafts/suggest", {
    data: {
      project_id: "missing-project",
      advisor: "mock",
      target_task_id: "objects",
      target_label: "ball",
    },
  });
  expect(rejected.status()).toBe(400);
  expect(await rejected.json()).toMatchObject({
    error: expect.stringContaining("choose llm"),
  });

  await page.goto("/settings/providers");
  await expect(page.getByRole("heading", { name: "Providers" })).toBeVisible();
  await expect(page.getByText("Mock", { exact: false })).toHaveCount(0);
  await page.getByRole("button", { name: "Add provider" }).click();
  await expect(page.getByLabel("Adapter")).toHaveValue("open_ai_compatible");
  await expect(page.getByLabel("Adapter").locator("option")).toHaveCount(1);
});
