import { test, expect, type Page } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
const dir = process.env.UI_SCREENSHOT_DIR || "ui-preview/evidence";
async function scenario(page: Page, label: string) {
  await page.getByText("演示场景", { exact: true }).click();
  await page.getByRole("button", { name: label, exact: true }).click();
  await page.getByText("演示场景", { exact: true }).click();
}
async function plan(page: Page) {
  await page
    .getByRole("textbox", { name: "给 AnnotAgent 的需求" })
    .fill(
      "标出这些图片里的杯子和瓶子，用来训练检测模型。先给我方案，不确定的让我确认。",
    );
  await page.getByRole("button", { name: "发送", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "批准并试跑 3 张" }),
  ).toBeVisible();
}
test("capture actual React preview pages and evidence metadata", async ({
  page,
}) => {
  mkdirSync(dir, { recursive: true });
  const records: object[] = [];
  const sha = execFileSync("git", ["rev-parse", "HEAD"], {
    encoding: "utf8",
  }).trim();
  const shot = async (name: string) => {
    await page
      .locator("img")
      .evaluateAll((images) =>
        Promise.all(
          images.map((i) => (i as HTMLImageElement).decode().catch(() => {})),
        ),
      );
    await page.screenshot({ path: `${dir}/${name}.png` });
    records.push({
      file: `${name}.png`,
      sha,
      url: page.url(),
      viewport: page.viewportSize(),
      dpr: await page.evaluate(()=>devicePixelRatio),
      cssZoom: await page.evaluate(()=>getComputedStyle(document.body).zoom),
      theme: await page.locator("html").getAttribute("data-aa-theme"),
      status: "UI Preview / Fixture; no Live Provider",
      capturedAt: new Date().toISOString(),
    });
  };
  await page.goto("/ui-preview?task=new");
  await shot("01-new-task-light");
  await plan(page);
  await shot("02-plan-light");
  await page.getByRole("button", { name: /选择模型：/ }).click();
  await shot("06-model-picker-light");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "批准并试跑 3 张" }).click();
  await page.getByRole("button", { name: "确认模拟试跑", exact: true }).click();
  await page
    .getByRole("textbox", { name: "给 AnnotAgent 的需求" })
    .fill("请先检查杯柄边界。");
  await page.getByRole("button", { name: "排队", exact: true }).click();
  await page.getByRole("button", { name: "打开数据", exact: true }).click();
  await shot("03-running-queue-light");
  await page.getByRole("button", { name: "停止" }).click();
  await expect(page.getByRole("button", { name: "正在停止…" })).toBeVisible();
  await shot("05-stopping-light");
  await expect(
    page.getByRole("button", { name: "继续任务", exact: true }),
  ).toBeVisible();
  await page.goto("/ui-preview?task=new&settings=general");
  await page
    .getByRole("combobox", { name: "外观", exact: true })
    .selectOption("dark");
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect(page.getByRole("status")).toContainText("已保存");
  await page.getByRole("button", { name: "← 返回任务", exact: true }).click();
  await page.getByRole("button", { name: "打开数据", exact: true }).click();
  await shot("04-interrupted-dark");
  await page.goto("/ui-preview?task=new&settings=general");
  await page
    .getByRole("combobox", { name: "外观", exact: true })
    .selectOption("light");
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect(page.getByRole("status")).toContainText("已保存");
  await page.getByRole("button", { name: "← 返回任务", exact: true }).click();
  await scenario(page, "需要人工协助");
  await page.getByRole("button", { name: "打开数据", exact: true }).click();
  await shot("07-human-assistance-light");
  for (const [id, name] of [
    ["general", "08-general"],
    ["providers", "09-providers"],
    ["agent", "11-agent-models"],
    ["vision", "12-vision-plugins"],
    ["privacy", "13-data-privacy"],
    ["usage", "14-usage-budget"],
  ]) {
    await page.goto(`/ui-preview?task=new&settings=${id}`);
    await shot(name);
    if (id === "providers") {
      await page.getByRole("button", { name: "添加 Provider" }).click();
      await page.getByRole("button", { name: "保存账户" }).click();
      await shot("10-provider-validation");
      await page.getByRole("button", { name: "取消账户编辑" }).click();
    }
  }
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/ui-preview?task=new");
  await shot("15-mobile-conversation");
  await page.getByRole("button", { name: "打开数据", exact: true }).click();
  await shot("16-mobile-canvas");
  await page.goto("/ui-preview?task=new&settings=general");
  await shot("17-mobile-settings");
  if(process.env.UI_SCREENSHOT_DIR){
    await page.setViewportSize({width:1440,height:960});
    await page.goto("/ui-preview?icons=1");
    for(const theme of ["light","dark"]){
      if(theme==="dark")await page.getByRole("button",{name:"切换浅色 / 深色"}).click();
      await page.getByRole("button",{name:"plus active",exact:true}).focus();
      await page.getByRole("button",{name:"folder active",exact:true}).hover();
      await shot(`icons-${theme}`);
    }
    await page.evaluate(()=>{document.body.style.zoom="2";});
    await shot("icons-css-200-not-native");
  }
  writeFileSync(`${dir}/manifest.json`, JSON.stringify(records, null, 2));
});
for (const size of [
  { width: 1440, height: 900 },
  { width: 1280, height: 720 },
  { width: 1024, height: 768 },
  { width: 390, height: 844 },
])
  test(`responsive controls ${size.width}x${size.height}`, async ({ page }) => {
    await page.setViewportSize(size);
    await page.goto("/ui-preview?task=new");
    await scenario(page, "模拟执行中");
    for (let i = 0; i < 8; i++) {
      await page
        .getByRole("textbox", { name: "给 AnnotAgent 的需求" })
        .fill(`排队输入 ${i}`);
      await page.getByRole("button", { name: "排队", exact: true }).click();
    }
    const box = await page
      .getByRole("button", { name: "停止" })
      .boundingBox();
    expect(box!.y + box!.height).toBeLessThanOrEqual(size.height);
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth),
    ).toBeLessThanOrEqual(size.width);
    await page.getByRole("button", { name: "打开数据", exact: true }).click();
    await expect(page.locator(".artifact-pane")).toBeVisible();
    await page.getByRole("button", { name: "关闭图片", exact: true }).click();
    await expect(page.getByRole("button", { name: "停止" })).toBeVisible();
  });
test("geometry edits and image selection restore; saving failure does not advance", async ({
  page,
}) => {
  await page.goto("/ui-preview?task=new");
  await scenario(page, "需要人工协助");
  await page.getByRole("button", { name: "打开数据", exact: true }).click();
  await page.getByText("标注列表与精确编辑 · 3 个", { exact: true }).click();
  await page
    .getByRole("spinbutton", { name: "cup-1 w", exact: true })
    .fill("300");
  await page.getByRole("button", { name: "对比", exact: true }).click();
  await page.getByRole("button", { name: "关闭图片" }).click();
  await page.getByRole("button", { name: "打开数据", exact: true }).click();
  await page.getByText("标注列表与精确编辑 · 3 个", { exact: true }).click();
  await expect(
    page.getByRole("spinbutton", { name: "cup-1 w", exact: true }),
  ).toHaveValue("300");
  await page.getByRole("button", { name: "查看图片 2", exact: true }).click();
  await expect(page).toHaveURL(/image=2/);
  await page.reload();
  await expect(page.getByText("示意图片 · 2/3", { exact: true })).toBeVisible();
  await page.goBack();
  await expect(page.getByText("示意图片 · 1/3", { exact: true })).toBeVisible();
  await page.getByText("演示场景", { exact: true }).click();
  await page
    .getByRole("button", { name: "下一次保存失败", exact: true })
    .click();
  await page.getByText("演示场景", { exact: true }).click();
  await page
    .getByRole("button", { name: "提交修正并继续", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText("保存失败");
  await expect(
    page.getByRole("button", { name: "提交修正并继续", exact: true }),
  ).toBeEnabled();
  await page
    .getByRole("button", { name: "提交修正并继续", exact: true })
    .click();
  await expect(page.locator(".operation")).toContainText("演示已完成");
});
