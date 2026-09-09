import { expect, test } from "./fixtures";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

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
  expect((await request.post(`/api/projects/${id}/image-upload?name=TEST-synthetic.png`, { data: readFileSync("../examples/robocup/images/synthetic-robocup.png"), headers: { "Content-Type": "image/png" } })).ok()).toBe(true);
  const images = (await (await request.get(`/api/projects/${id}/images`)).json()).images;
  await page.goto(`/projects/${id}/work?image=${images[0].image_id}&pane=artifacts`);
  await expect(page.getByRole("img", { name: "TEST-synthetic.png", exact: true })).toBeVisible();
  await expect(page.locator(".conversation-panel")).toHaveCount(1);
  await expect(page.locator(".conversation-image-panel")).toHaveCount(1);
  const split = await page.locator(".conversation-split").boundingBox();
  const thread = await page.locator(".conversation-panel").boundingBox();
  expect(split?.x).toBe(0);
  expect(Math.abs(thread!.width / split!.width - 0.54)).toBeLessThan(0.02);
  await page.screenshot({ path: `${evidence}/data-workspace.png`, fullPage: true });
  writeFileSync(`${evidence}/metadata.json`, JSON.stringify({ sha: execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim(), dirty: true, url: page.url(), viewport: page.viewportSize(), theme: "light", data: "TEST synthetic image; no inference", service: "real isolated AnnotAgent server 8791", bundles: await page.locator("script[src]").evaluateAll(elements=>elements.map(element=>element.getAttribute("src"))) }, null, 2));
  await page.getByRole("button", { name: "Project menu", exact: true }).click();
  await page.getByRole("region", { name: "Project management", exact: true }).getByRole("button", { name: "Project management", exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${id}$`));
  await page.getByRole("button", { name: "Back to annotation workspace", exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${id}/work$`));
  await page.goto("/settings");
  await expect(page.locator(".sidebar")).toHaveCount(0);
  await expect(page.getByRole("navigation", { name: "Primary navigation", exact: true }).getByRole("link")).toHaveCount(2);
});
