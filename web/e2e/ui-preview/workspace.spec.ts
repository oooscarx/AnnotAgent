import { test, expect } from "@playwright/test";
test.beforeEach(async ({ page }) => {
  await page.goto("/ui-preview?task=new");
});
test("one project sidebar, centered composer and no production navigation", async ({
  page,
}) => {
  await expect(page.locator(".project-sidebar")).toHaveCount(1);
  await expect(
    page.locator(".sidebar,.agent-conversation-navigation"),
  ).toHaveCount(0);
  await expect(
    page.getByRole("link", { name: /Runs|Review|Pipelines/ }),
  ).toHaveCount(0);
  const sidebar = await page.locator(".project-sidebar").boundingBox();
  const c = await page.locator(".composer").boundingBox();
  expect(sidebar!.width).toBe(216);
  expect(c!.width).toBeLessThanOrEqual(720);
  expect(Math.abs(c!.x + c!.width / 2 - (216 + (1440 - 216) / 2))).toBeLessThan(
    24,
  );
  expect((await page.locator(".workspace-header").boundingBox())!.height).toBe(
    52,
  );
  await expect(page.locator(".artifact-pane")).toHaveCount(0);
});
test("each project creates a task from the plus beside its name", async ({ page }) => {
  await expect(page.locator(".project-sidebar > .new-task")).toHaveCount(0);
  const project = page.locator(".project-tree-header").filter({
    hasText: "商品图片标注",
  });
  await expect(project.getByRole("button", { name: "在商品图片标注中新建任务" })).toBeVisible();
  const previousTasks = await page.locator(".task-tree-row").count();
  await project.getByRole("button", { name: "在商品图片标注中新建任务" }).click();
  await expect(page.locator(".task-tree-row")).toHaveCount(previousTasks + 1);
  await expect(page).toHaveURL(/task=[0-9a-f-]{36}/);
});
test("IME, Shift Enter, single send, approval, queue, stop and continue", async ({
  page,
}) => {
  const input = page.getByRole("textbox", { name: "给 AnnotAgent 的需求" });
  await input.fill("框出杯子");
  await input.dispatchEvent("compositionstart");
  await input.press("Enter");
  await expect(page.locator(".user-message")).toHaveCount(0);
  await input.dispatchEvent("compositionend");
  // Synthetic composition does not implement the OS IME's default text editing.
  await input.fill("框出杯子");
  await input.press("Shift+Enter");
  await expect(input).toHaveValue("框出杯子\n");
  await input.press("Enter");
  await expect(page.locator(".user-message")).toHaveCount(1);
  await page.getByRole("button", { name: "批准并试跑 3 张" }).click();
  await page.getByRole("button", { name: "确认模拟试跑", exact: true }).click();
  await input.fill("先检查边界");
  await page.getByRole("button", { name: "排队", exact: true }).click();
  await expect(page.locator(".queue")).toContainText("1 条");
  await page.getByRole("button", { name: "停止" }).click();
  await expect(page.getByRole("button", { name: "正在停止…" })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "继续任务", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "继续任务", exact: true }).click();
  await expect(page.locator(".operation")).toContainText("模拟执行中");
});
test("task draft, pane and model selection survive relevant navigation", async ({
  page,
}) => {
  const input = page.getByRole("textbox", { name: "给 AnnotAgent 的需求" });
  await input.fill("保留中文输入");
  await page.getByRole("button", { name: "打开数据", exact: true }).click();
  await expect(input).toHaveValue("保留中文输入");
  await page.reload();
  await expect(page.locator(".artifact-pane")).toBeVisible();
  await expect(input).toHaveValue("保留中文输入");
  await page.getByRole("button", { name: /选择模型：/ }).click();
  await page.getByRole("button", { name: /Qwen · 演示/ }).click();
  await expect(
    page.getByRole("button", { name: /选择模型：Qwen · 演示/ }),
  ).toBeVisible();
  await page.getByRole("button", { name: /选择模型：/ }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: /选择模型：/ })).toBeFocused();
});
