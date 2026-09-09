import { expect, test } from "@playwright/test";

test("native disclosure has one soft chevron and keyboard operation without nested buttons", async ({ page }) => {
  await page.goto("/ui-preview?task=new");
  await page.getByRole("textbox", { name: "给 AnnotAgent 的需求" }).fill("标出杯子，先给我方案");
  await page.getByRole("button", { name: "发送", exact: true }).click();
  const details = page.locator(".plan-block .ui-disclosure");
  const summary = details.locator(":scope > summary");
  await expect(summary.locator("svg")).toHaveCount(1);
  await expect(summary.locator("button")).toHaveCount(0);
  await expect(summary).toHaveCSS("list-style-type", "none");
  await summary.focus();
  await page.keyboard.press("Enter");
  await expect(details).toHaveAttribute("open", "");
  await page.keyboard.press("Space");
  await expect(details).not.toHaveAttribute("open", "");
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect(summary.locator(".disclosure-chevron")).toHaveCSS("transition-duration", "0s");
});
