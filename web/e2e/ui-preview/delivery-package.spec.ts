import { expect, test } from "@playwright/test";
import { resolve } from "node:path";

test("package authorization uses server readiness and never scans images or starts a package in React", async ({ page }) => {
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async (path) => {
    const { React, createRoot, DeliveryPackage } = await import(path);
    const host = document.createElement("main");
    document.body.replaceChildren(host);
    const state = { imageReads: 0, starts: 0, authorizations: [] as unknown[], armed: false };
    Object.assign(window, { packageAuthorizationTest: state });
    const service = {
      history: async () => ({ items: [], next_cursor: null }),
      pendingPackage: () => undefined,
      image: async () => { state.imageReads += 1; throw new Error("must not scan"); },
      startPackage: async () => { state.starts += 1; throw new Error("must not start"); },
      packageReadiness: async () => ({
        intent_revision: 2, intent_sha256: "intent", ready: false,
        counts: { total: 2, complete: 1, positive: 1, negative: 0, excluded: 0, unresolved: 1, failed: 0 },
        review_revisions: { one: 3 },
        blockers: [{ code: "review_incomplete", message: "还有 1 张图片待确认", image_ids: ["two"] }],
        consent: state.armed ? { input: { id: "authorization-one", intent_revision: 2, intent_sha256: "intent", confirmed: true }, state: "armed" } : null,
        package: null,
      }),
      authorizePackage: async (_p: string, _t: string, input: unknown) => {
        state.authorizations.push(input); state.armed = true;
        return { input, state: "armed" };
      },
      packageStatus: async () => { throw new Error("no package selected"); },
      cancelPackage: async () => { throw new Error("unused"); },
      downloadUrl: () => "/unused",
    };
    createRoot(host).render(React.createElement(DeliveryPackage, {
      service, project: "TEST", task: "TASK",
      scope: { revision: 2, content_sha256: "intent", image_ids: ["one", "two"] },
      onInspect: () => {},
    }));
  }, `/@fs/${resolve("e2e/ui-preview/delivery-review-harness.tsx")}`);

  await expect(page.getByText("已完成 1/2")).toBeVisible();
  await expect(page.getByText("还有 1 张图片待确认")).toBeVisible();
  let state = await page.evaluate(() => (window as unknown as { packageAuthorizationTest: { imageReads: number; starts: number; authorizations: unknown[] } }).packageAuthorizationTest);
  expect(state.imageReads).toBe(0);
  expect(state.starts).toBe(0);
  expect(state.authorizations).toEqual([]);

  await page.getByRole("button", { name: "允许审核齐全后自动打包", exact: true }).click();
  await expect(page.getByText("自动打包授权已保存", { exact: false })).toBeVisible();
  state = await page.evaluate(() => (window as unknown as { packageAuthorizationTest: { imageReads: number; starts: number; authorizations: unknown[] } }).packageAuthorizationTest);
  expect(state.imageReads).toBe(0);
  expect(state.starts).toBe(0);
  expect(state.authorizations).toHaveLength(1);
  expect(state.authorizations[0]).toMatchObject({ intent_revision: 2, intent_sha256: "intent", confirmed: true });
});

test("package history and download render only persisted server receipts", async ({ page }) => {
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async (path) => {
    const { React, createRoot, DeliveryPackage } = await import(path);
    const host = document.createElement("main");
    document.body.replaceChildren(host);
    const state = { writes: 0 };
    Object.assign(window, { packageReceiptTest: state });
    const receipt = {
      job: {
        id: "saved-package", phase: "ready", intent_revision: 2, snapshot_sha256: "snapshot",
        result: {
          images: 11, objects: 20, negatives: 1, excluded: 1, bytes: 12345,
          sha256: "TEST frozen hash",
          summary: { labels: ["冻结类别"], splits: { train: 9, val: 2 }, warnings: ["TEST report warning"], exclusions: { excluded: "TEST reason" } },
        },
        error: null,
      },
      active: false, interrupted: false,
    };
    const service = {
      history: async () => ({ items: [{ id: "saved-package", created_at: "TEST saved" }], next_cursor: null }),
      pendingPackage: () => undefined,
      startPackage: async () => { state.writes += 1; throw new Error("must not start"); },
      packageReadiness: async () => ({
        intent_revision: 2, intent_sha256: "intent", ready: true,
        counts: { total: 11, complete: 11, positive: 9, negative: 1, excluded: 1, unresolved: 0, failed: 0 },
        review_revisions: {}, blockers: [],
        consent: { input: { id: "consent", intent_revision: 2, intent_sha256: "intent", confirmed: true }, state: "consumed" },
        package: receipt,
      }),
      packageStatus: async () => receipt,
      cancelPackage: async () => { throw new Error("unused"); },
      downloadUrl: () => "/TEST-only-no-download",
    };
    createRoot(host).render(React.createElement(DeliveryPackage, {
      service, project: "TEST", task: "TASK",
      scope: { revision: 2, content_sha256: "intent", image_ids: ["one"] },
      onInspect: () => {},
    }));
  }, `/@fs/${resolve("e2e/ui-preview/delivery-review-harness.tsx")}`);

  await expect(page.getByText("数据集已打包", { exact: true })).toBeVisible();
  await expect(page.getByText("类别：冻结类别", { exact: true })).toBeVisible();
  await expect(page.getByText("训练图片 9 · 验证图片 2", { exact: false })).toBeVisible();
  await expect(page.getByRole("link", { name: "下载数据集 ZIP" })).toHaveAttribute("href", "/TEST-only-no-download");
  await page.getByRole("button", { name: "刷新审核与打包状态", exact: true }).click();
  expect(await page.evaluate(() => (window as unknown as { packageReceiptTest: { writes: number } }).packageReceiptTest.writes)).toBe(0);
});
