import { expect, test } from "@playwright/test";
import { randomUUID } from "node:crypto";
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

test("model runtime settings expose the exact next-request mapping without probing", async ({ page, request }, testInfo) => {
  const health = await request.get("/api/health");
  expect(health.headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const session = await (await request.get("/api/session")).json();
  const providers = await (await request.get("/api/providers")).json();
  const provider = providers.providers.find((value: { adapter: string }) => value.adapter === "open_ai_compatible");
  expect(provider).toBeTruthy();

  const name = `TEST R3 runtime ${randomUUID()}`;
  const createdResponse = await request.post("/api/model-profiles", {
    headers: { "x-annotagent-csrf": session.csrf_token },
    data: {
      provider_id: provider.id,
      display_name: name,
      remote_model_id: "test-r3-effective-request-no-call",
      input_modalities: ["text", "image"],
      task_capabilities: ["text_generation", "vision_language"],
      protocol_features: { tool_calls: true, structured_output: true, reasoning_controls: true },
      limits: { context_tokens: 32768, maximum_output_tokens: 2048 },
      generation_defaults: {
        maximum_output_tokens: 1024,
        temperature: 0.1,
        top_p: 0.9,
        reasoning_mode: "enabled",
        reasoning_wire_parameter: "enable_thinking",
        supported_reasoning_modes: ["disabled", "enabled"],
      },
      pricing: {
        currency: "USD",
        input_per_million_tokens: "2",
        output_per_million_tokens: "8",
        source: "user_configured",
      },
    },
  });
  expect(createdResponse.ok(), await createdResponse.text()).toBeTruthy();
  const created = await createdResponse.json();

  const writes: string[] = [];
  page.on("request", (value) => {
    if (value.method() !== "GET") writes.push(`${value.method()} ${new URL(value.url()).pathname}`);
  });
  await page.goto("/settings/vision-models");
  const region = page.getByRole("region", { name: "视觉模型配置", exact: true });
  await region.locator('input[placeholder="名称、模型 ID 或能力"]').fill(name);
  await region.getByRole("button", { name: "编辑配置", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "编辑模型配置", exact: true });
  const evidence = dialog.getByRole("region", { name: "有效模型请求", exact: true });
  await expect(evidence).toContainText("32768 tokens");
  await expect(evidence).toContainText("31744 tokens");
  await expect(evidence).toContainText("1024 tokens");
  await expect(evidence).toContainText("enable_thinking=true");
  await expect(evidence).toContainText("USD 2");
  await expect(evidence).toContainText("USD 8");
  await expect(dialog.locator("label").filter({ hasText: "思考模式" }).locator("select")).toHaveValue("enabled");
  await dialog.locator("label").filter({ hasText: "默认最大输出（tokens）" }).locator("input").fill("1536");
  await dialog.locator("label").filter({ hasText: "输出 / 百万 tokens" }).locator("input").fill("9");
  await dialog.getByRole("button", { name: "保存配置", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  const saved = await (await request.get(`/api/model-profiles/${created.id}`)).json();
  expect(saved.model.generation_defaults.maximum_output_tokens).toBe(1536);
  expect(saved.model.pricing.output_per_million_tokens).toBe("9");
  expect(writes).toEqual([`PATCH /api/model-profiles/${created.id}`]);
  await page.screenshot({ path: testInfo.outputPath("model-runtime-settings.png"), fullPage: true });
});
