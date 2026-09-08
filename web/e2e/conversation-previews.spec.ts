import { resolve } from "node:path";
import { expect, test } from "./fixtures";
import { isolatedEvidencePath } from "./evidence";

test("browse previews are bounded while the selected canvas keeps original pixels", async ({ page, request }) => {
  const project = `test-preview-${Date.now()}`;
  const yaml = "version: 1\nproject:\n  name: TEST preview transport\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n";
  expect((await request.post("/api/projects", { data: { id: project, yaml } })).ok()).toBeTruthy();
  await page.goto(`/projects/${project}/work`);
  await expect(page.getByText("Saved workspace loaded", { exact: true })).toBeVisible();
  await page.getByLabel("Add images", { exact: true }).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await expect(page.getByText("Images saved on this server. This upload did not start inference.", { exact: true })).toBeVisible();
  const { images } = await (await request.get(`/api/projects/${project}/images`)).json();
  expect(images).toHaveLength(1);
  const image = images[0];
  expect(image.thumbnail_url).toMatch(/\/thumbnail$/);
  const thumbnail = page.getByRole("navigation", { name: "Select image", exact: true }).locator("img");
  await expect(thumbnail).toHaveAttribute("src", image.thumbnail_url);
  await expect.poll(() => thumbnail.evaluate(element => (element as HTMLImageElement).naturalWidth)).toBeGreaterThan(0);
  const dimensions = await thumbnail.evaluate(element => {
    const image = element as HTMLImageElement;
    return [image.naturalWidth, image.naturalHeight];
  });
  expect(Math.max(...dimensions)).toBeLessThanOrEqual(256);
  await page.getByRole("navigation", { name: "Select image", exact: true }).getByRole("button").click();
  const canvas = page.locator(".conversation-image img");
  await expect(canvas).toHaveAttribute("src", image.url);
  await expect.poll(() => canvas.evaluate(element => (element as HTMLImageElement).naturalWidth)).toBeGreaterThan(256);
  const writes: string[] = [];
  page.on("request", request => {
    if (!["GET", "HEAD"].includes(request.method())) writes.push(request.url());
  });
  await page.reload();
  await expect(canvas).toHaveAttribute("src", image.url);
  await expect(thumbnail).toHaveAttribute("src", image.thumbnail_url);
  expect(writes).toEqual([]);
  await page.screenshot({ path: isolatedEvidencePath("../docs/execution/conversational-workspace/bounded-browse-preview.png"), animations: "disabled" });
});
