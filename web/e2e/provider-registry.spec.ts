import { expect, test } from "@playwright/test";

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
  await page.getByLabel("Tool calls", { exact: true }).check();
  await page.getByLabel("Structured output", { exact: true }).check();
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

  await page.getByRole("button", { name: "Usage", exact: true }).click();
  await expect(page.getByText("Mock Pipeline Builder", { exact: true })).toBeVisible();
  await expect(page.getByText("2 tokens", { exact: true })).toBeVisible();
  await expect(page.getByText("explicitly confirmed", { exact: true })).toBeVisible();
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
