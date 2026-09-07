import { test, expect } from "@playwright/test";

test("Chinese selection persists across refresh and synchronizes without losing an unsaved form", async ({ page, context }) => {
  await page.goto("/");
  await page.getByLabel("Language / 语言", { exact: true }).selectOption("zh-CN");
  await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
  await expect(page.getByRole("heading", { name: "首页", exact: true })).toBeVisible();
  await page.reload();
  await expect(page.getByLabel("Language / 语言", { exact: true })).toHaveValue("zh-CN");
  await page.getByRole("button", { name: "选择图片开始", exact: true }).click();
  await page.getByLabel("项目名称", { exact: true }).fill("Unsaved 中文 project");
  const formUrl = page.url();
  const secondTab = await context.newPage();
  await secondTab.goto("/");
  await expect(secondTab.getByLabel("Language / 语言", { exact: true })).toHaveValue("zh-CN");
  await secondTab.getByLabel("Language / 语言", { exact: true }).selectOption("en");
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.getByLabel("Project name", { exact: true })).toHaveValue("Unsaved 中文 project");
  await expect(page).toHaveURL(formUrl);
  await secondTab.close();
});

test("browser language selects Chinese and translated settings remain usable on mobile", async ({ browser, baseURL }) => {
  const context = await browser.newContext({ locale: "zh-CN", viewport: { width: 390, height: 844 } });
  const page = await context.newPage();
  await page.goto(`${baseURL}/settings/providers`);
  await expect(page.getByLabel("Language / 语言", { exact: true })).toHaveValue("zh-CN");
  await expect(page.getByRole("heading", { name: "设置", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "新增服务商", exact: true })).toBeVisible();
  await expect(page.getByLabel("Language / 语言", { exact: true })).toBeInViewport();
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
  await page.getByRole("button", { name: "模型", exact: true }).click();
  await expect(page).toHaveURL(/\/settings\/models$/);
  await expect(page.getByRole("heading", { name: "设置", exact: true })).toBeVisible();
  await context.close();
});
