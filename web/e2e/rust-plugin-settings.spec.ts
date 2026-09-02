import { expect, test } from "@playwright/test";

test("Rust Expert Model Plugins are reachable, honest, responsive, and keyboard labeled", async ({ page }) => {
  await page.goto("/settings/plugins");
  await expect(page.getByRole("heading", { name: "Expert Model Plugins" })).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: "Expert Model Plugins" })).toHaveAttribute("aria-current", "page");
  await expect(page.getByLabel("Agent plugin permissions")).toContainText("read only");
  await expect(page.getByLabel("Installation steps")).toContainText("Verify package");
  await expect(page.getByLabel("Plugin package")).toHaveAttribute("accept", ".annotplugin");
  await expect(page.getByRole("button", { name: "Verify package" })).toBeDisabled();
  await expect(page.getByText("No Expert Model Plugins installed")).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();

  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
  await expect(page.getByRole("heading", { name: "Expert Model Plugins" })).toBeVisible();
});
