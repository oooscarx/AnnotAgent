import { resolve } from "node:path";
import { expect, test } from "./fixtures";

test("conversation journal restores, freezes image references and retries without inference", async ({ page, request }) => {
  const project = `conversation-test-${Date.now()}`;
  const yaml = "version: 1\nproject:\n  name: TEST conversation workspace\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n";
  expect((await request.post("/api/projects", { data: { id: project, yaml } })).ok()).toBeTruthy();
  const root = `/api/projects/${project}/conversations`;
  const charged: string[] = [];
  page.on("request", (req) => { if (req.method() === "POST" && /suggest|dry-run|sample-operations|processing-operations|active-probe/.test(req.url())) charged.push(req.url()); });
  await page.goto(`/projects/${project}/work`);
  await expect(page.getByText("Saved workspace loaded", { exact: true })).toBeVisible();
  expect((await (await request.get(root)).json()).conversation_id).toBeNull();
  await page.reload();
  await expect(page.getByText("Saved workspace loaded", { exact: true })).toBeVisible();
  expect((await (await request.get(root)).json()).conversation_id).toBeNull();
  await page.getByLabel("Your message", { exact: true }).fill("TEST find cups, not bottles");
  await page.getByRole("button", { name: "Save message", exact: true }).click();
  await expect(page.getByRole("list", { name: "Saved messages" })).toContainText("TEST find cups, not bottles");
  const conversation = (await (await request.get(root)).json()).conversation_id;
  await page.getByLabel("Add images", { exact: true }).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await expect(page.getByText("Images saved on this server. No model has been called.", { exact: true })).toBeVisible();
  const images = (await (await request.get(`/api/projects/${project}/images`)).json()).images;
  await page.getByLabel("Your message", { exact: true }).fill("TEST inspect this image");
  await page.route(`**${root}/${conversation}/messages`, async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    const response = await route.fetch(); expect(response.ok()).toBeTruthy();
    await route.abort("failed");
  }, { times: 1 });
  await page.getByRole("button", { name: "Save message", exact: true }).click();
  await expect(page.getByRole("button", { name: "Retry saving message", exact: true })).toBeEnabled();
  await page.getByRole("button", { name: "Retry saving message", exact: true }).click();
  await expect(page.getByRole("list", { name: "Saved messages" }).getByRole("listitem")).toHaveCount(2);
  const messages = await (await request.get(`${root}/${conversation}/messages`)).json();
  expect(messages).toHaveLength(2);
  expect(messages[0].input.image).toBeNull();
  expect(messages[1].input.image).toEqual({ image_id: images[0].image_id, sha256: images[0].content_hash });
  await page.reload();
  await expect(page.getByRole("list", { name: "Saved messages" }).getByRole("listitem")).toHaveCount(2);
  await page.getByRole("button", { name: /^Referenced image/ }).click();
  await expect(page).toHaveURL(new RegExp(`conversation=${conversation}.*image=${images[0].image_id}`));
  const divider = page.getByRole("separator", { name: "Resize conversation panel" });
  await divider.focus(); await page.keyboard.press("ArrowRight");
  await expect(divider).toHaveAttribute("aria-valuenow", "34");
  for (const width of [1440, 1280, 1024, 390]) {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    if (width === 390) await page.getByRole("button", { name: "Images (1)", exact: true }).click();
    await page.screenshot({ path: `../docs/execution/conversational-workspace/journal-${width}.png`, fullPage: true, animations: "disabled" });
  }
  expect(charged).toEqual([]);
});
