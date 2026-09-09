import { expect, test } from "./fixtures";
import { mkdirSync } from "node:fs";

const evidence = process.env.ANNOTAGENT_CONVERGENCE_EVIDENCE ?? "/tmp/annotagent-ui-convergence";

test("default entry and workspace mount no permanent sidebar", async ({ page, request }) => {
  mkdirSync(evidence, { recursive: true });
  await page.setViewportSize({ width: 1440, height: 900 });
  const id = `TEST-ui-convergence-${Date.now()}`;
  expect((await request.post("/api/projects", { data: { id, yaml:
    "version: 1\nproject:\n  name: TEST UI convergence\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n",
  } })).ok()).toBe(true);
  await page.goto("/projects");
  await expect(page.getByRole("heading", { name: "My projects", exact: true })).toBeVisible();
  await page.screenshot({ path: `${evidence}/projects.png`, fullPage: true });
  await expect.soft(page.locator(".sidebar")).toHaveCount(0);
  const navigation = page.getByRole("navigation", { name: "Primary navigation", exact: true });
  await expect.soft(navigation).toContainText("Projects");
  await expect.soft(navigation).toContainText("Settings");
  await expect.soft(navigation.getByRole("link", { name: /^(Runs|Review|Workflows)$/ })).toHaveCount(0);
  await page.goto(`/projects/${id}/work`);
  await expect(page.getByRole("textbox", { name: "Your message", exact: true })).toBeEnabled();
  await page.screenshot({ path: `${evidence}/empty-workspace.png`, fullPage: true });
  await expect.soft(page.locator(".sidebar")).toHaveCount(0);
  await expect.soft(page.locator(".agent-conversation-navigation")).toHaveCount(0);
  await expect.soft(page.getByRole("navigation", { name: "Conversations", exact: true })).toHaveCount(0);
  await expect.soft(page.locator(".focus-header")).toHaveCount(0);
  await expect.soft(page.locator(".conversation-surface-controls")).toHaveCount(0);
  const header = await page.locator(".agent-project-header").boundingBox();
  expect.soft(header?.height).toBeGreaterThanOrEqual(48);
  expect.soft(header?.height).toBeLessThanOrEqual(56);
  const composer = await page.locator(".conversation-composer").boundingBox();
  expect.soft(composer?.width).toBeLessThanOrEqual(736);
  expect.soft(Math.abs((composer?.x ?? -1000) + (composer?.width ?? 0) / 2 - 720)).toBeLessThanOrEqual(24);
});
