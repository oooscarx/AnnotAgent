import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";

type FixtureManifest = {
  fixture: string;
  project: string;
  task_id: string;
  task_root: string;
};

test("task usage keeps owner scope, physical attempts and unknown cost truthful", async ({ page, request }, testInfo) => {
  const manifest = JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!, "utf8")) as FixtureManifest;
  expect(manifest.fixture).toBe("external-model-only");
  expect(manifest.project).toMatch(/^TEST-/);

  const usageResponse = await request.get(`${manifest.task_root}/model-usage?limit=50`);
  expect(usageResponse.ok(), await usageResponse.text()).toBeTruthy();
  const usage = await usageResponse.json();
  expect(usage.scope.project_id).not.toBe(manifest.project);
  expect(usage.scope.task_id).toBe(manifest.task_id);
  expect(usage.attempts.items.length).toBeGreaterThan(0);
  expect(usage.attempts.items.every((attempt: { kind: string }) => attempt.kind === "task")).toBe(true);

  const writes: string[] = [];
  page.on("request", (value) => {
    if (value.method() !== "GET") writes.push(`${value.method()} ${new URL(value.url()).pathname}`);
  });
  await page.goto(`/projects/${manifest.project}/work?task=${manifest.task_id}`);
  await page.locator("summary").filter({ hasText: "查看执行与任务详情" }).click();

  const region = page.getByRole("region", { name: "本次任务模型用量", exact: true });
  await expect(region.locator(".task-usage-view")).toBeVisible();
  await expect(region).toContainText(`${usage.summary.attempt_count} 次请求尝试`);
  await expect(region).toContainText("总费用未知（不是 0）");
  await expect(region).toContainText("未知值未按 0 处理");
  await region.locator("summary").filter({ hasText: "调用明细" }).click();
  await expect(region).toContainText("第 1 次物理请求");
  await expect(region).toContainText("价格 revision");
  await region.locator("summary").filter({ hasText: "请求与价格证据" }).first().click();
  await expect(region.locator("pre").first()).toContainText("effective_maximum_output_tokens");
  expect(writes).toEqual([]);

  await page.screenshot({ path: testInfo.outputPath("task-usage-owner-scope.png"), fullPage: true });
});
