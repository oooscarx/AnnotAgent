import { test, expect } from "@playwright/test";

test("long sidebar titles reveal their end on hover and keyboard focus, then reset", async ({ page }) => {
  await page.goto("/ui-preview?task=new");
  const title = "这是需要完整显示的很长的项目任务标题用于检查鼠标悬停之后末尾文字";
  await page.getByRole("textbox", { name: "给 AnnotAgent 的需求" }).fill(title);
  await page.getByRole("button", { name: "发送", exact: true }).click();
  const row = page.locator(".task-link.selected");
  const viewport = row.locator(".sidebar-title");
  const text = row.locator(".sidebar-title-text");
  await expect(viewport).toHaveAttribute("data-overflow", "true");
  await expect(viewport).toHaveCSS("text-overflow", "clip");
  const offset = await viewport.evaluate(el => parseFloat((el as HTMLElement).style.getPropertyValue("--title-offset")));
  expect(offset).toBeLessThan(0);
  await row.hover();
  await expect.poll(() => text.evaluate(el => new DOMMatrix(getComputedStyle(el).transform).m41), {timeout: 12000}).toBeCloseTo(offset, 0);
  await page.getByRole("textbox", { name: "给 AnnotAgent 的需求" }).hover();
  await expect(text).toHaveCSS("transform", "matrix(1, 0, 0, 1, 0, 0)");
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.keyboard.press("Tab");
  await row.focus();
  await expect(text).toHaveCSS("transition-duration", "0s");
  await expect.poll(() => text.evaluate(el => new DOMMatrix(getComputedStyle(el).transform).m41)).toBeCloseTo(offset, 0);
  await expect(row).toHaveAccessibleName(title.slice(0, 35));
});
