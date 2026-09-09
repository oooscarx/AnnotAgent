import { test, expect } from "@playwright/test";
const sections = [
  "general",
  "providers",
  "agent",
  "vision",
  "privacy",
  "usage",
];
for (const section of sections)
  test(`${section}: loading, empty, error and recovery are explicit`, async ({
    page,
  }) => {
    await page.goto(`/ui-preview?task=new&settings=${section}`);
    await page.getByText("预览状态控制", { exact: true }).click();
    const select = page.getByRole("combobox", { name: "设置页面状态" });
    await select.selectOption("loading");
    await expect(page.getByRole("status")).toContainText("正在加载");
    await page.getByRole("button", { name: "完成模拟加载" }).click();
    await select.selectOption("empty");
    await expect(page.getByRole("button", { name: "开始配置" })).toBeVisible();
    await page.getByRole("button", { name: "开始配置" }).click();
    await select.selectOption("error");
    await expect(page.getByRole("alert")).toContainText("没有回退成成功数据");
    await page.getByRole("button", { name: "重试模拟读取" }).click();
    await expect(page.getByRole("button", { name: "保存设置" })).toBeVisible();
  });
test("general: immediate theme preview, cancel, failure, save and refresh", async ({
  page,
}) => {
  await page.goto("/ui-preview?settings=general");
  await page
    .getByRole("combobox", { name: "外观", exact: true })
    .selectOption("dark");
  await expect(page.locator("html")).toHaveAttribute("data-aa-theme", "dark");
  await page.getByRole("button", { name: "取消", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-aa-theme", "light");
  await page
    .getByRole("combobox", { name: "外观", exact: true })
    .selectOption("dark");
  await page.getByText("预览状态控制", { exact: true }).click();
  await page.getByRole("button", { name: "模拟下一次保存失败" }).click();
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect(page.getByRole("alert")).toContainText("编辑仍保留");
  await expect(
    page.getByRole("combobox", { name: "外观", exact: true }),
  ).toHaveValue("dark");
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect(page.getByRole("status")).toContainText("已保存");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-aa-theme", "dark");
});
test("providers: validate, save, simulated probe failure/unknown, deletion impact", async ({
  page,
}) => {
  await page.goto("/ui-preview?settings=providers");
  await page.getByRole("button", { name: "＋ 添加 Provider" }).click();
  await page.getByRole("button", { name: "保存账户" }).click();
  await expect(page.getByRole("alert")).toContainText("显示名称");
  await page.getByLabel("显示名称").fill("测试账户");
  await page
    .getByLabel("Endpoint")
    .fill("https://example.invalid/v1?key=forbidden");
  await page.getByRole("button", { name: "保存账户" }).click();
  await expect(page.getByRole("alert")).toContainText("不含凭证");
  await page.getByLabel("Endpoint").fill("https://example.invalid/v1");
  await page.getByRole("button", { name: "保存账户" }).click();
  await expect(page.getByText("测试账户", { exact: true })).toBeVisible();
  await page
    .getByRole("combobox", { name: "模拟连接结果" })
    .selectOption("unknown");
  await page
    .getByRole("button", { name: "测试连接", exact: true })
    .last()
    .click();
  await expect(page.getByText(/模拟结果未知/)).toBeVisible();
  await page.getByRole("button", { name: "删除", exact: true }).last().click();
  await expect(page.getByRole("dialog")).toContainText("不触碰真实凭证");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "删除", exact: true }).last(),
  ).toBeFocused();
  await expect(page.locator("input[type=password]")).toHaveCount(0);
});
test("dirty settings cannot silently leave through global project sidebar", async ({
  page,
}) => {
  await page.goto("/ui-preview?settings=usage");
  await page.getByRole("textbox", { name: "预算上限" }).fill("7.50");
  page.once("dialog", (d) => d.dismiss());
  await page.getByRole("button", { name: "· 新任务", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "预算上限" })).toHaveValue(
    "7.50",
  );
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect(page.getByRole("status")).toContainText("已保存");
  await page.getByRole("button", { name: "· 新任务", exact: true }).click();
  await expect(page.locator(".composer")).toBeVisible();
});
test("visual install confirm, failure and retry are simulated", async ({
  page,
}) => {
  await page.goto("/ui-preview?settings=vision");
  await page.getByRole("checkbox", { name: "模拟安装后校验失败" }).check();
  await page.getByRole("button", { name: "安装兼容模型" }).first().click();
  await page.getByRole("button", { name: "确认模拟操作" }).click();
  await expect(page.getByText(/模拟校验失败/)).toBeVisible();
  await page.getByRole("checkbox", { name: "模拟安装后校验失败" }).uncheck();
  await page.getByRole("button", { name: "安装兼容模型" }).first().click();
  await page.getByRole("button", { name: "确认模拟操作" }).click();
  await expect(
    page.getByText("1.0 · ONNX · Ready", { exact: true }),
  ).toBeVisible();
});
test("privacy cleanup preserves protected reference amount; usage unknown is not zero", async ({
  page,
}) => {
  await page.goto("/ui-preview?settings=privacy");
  await page.getByRole("button", { name: "查看清理范围" }).click();
  await expect(page.getByRole("dialog")).toContainText(
    "原图和已确认标注始终保留",
  );
  await page.getByRole("button", { name: "确认模拟操作" }).click();
  await expect(page.getByText("24 MB · 模拟", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "用量与预算", exact: true }).click();
  await expect(
    page.getByRole("cell", { name: "未知", exact: true }),
  ).toBeVisible();
  await page.getByRole("textbox", { name: "预算上限" }).fill("0.10");
  await expect(
    page.getByText("演示用量超过预算", { exact: true }),
  ).toBeVisible();
});
test("preview pages never request business or external APIs", async ({
  page, baseURL,
}) => {
  const requests: string[] = [];
  page.on("request", (r) => {
    if (
      r.url().includes("/api/") ||
      new URL(r.url()).origin !== new URL(baseURL!).origin
    )
      requests.push(r.url());
  });
  for (const section of sections) {
    await page.goto(`/ui-preview?settings=${section}`);
    await expect(page.locator("h1")).toBeVisible();
  }
  expect(requests).toEqual([]);
  const keys = await page.evaluate(() => Object.keys(localStorage));
  expect(keys.every((k) => k.startsWith("annotagent.ui-preview."))).toBe(true);
});
