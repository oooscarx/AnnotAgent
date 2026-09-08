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
  await expect(page.getByText("Images saved on this server. This upload did not start inference.", { exact: true })).toBeVisible();
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
  await divider.press("End");
  await expect(divider).toHaveAttribute("aria-valuenow", "50");
  await divider.press("Home");
  await expect(divider).toHaveAttribute("aria-valuenow", "25");
  await expect(divider).toHaveAttribute("aria-valuetext", "25% conversation panel");
  await divider.dispatchEvent("keydown", {key:"ArrowRight",isComposing:true});
  await expect(divider).toHaveAttribute("aria-valuenow", "25");
  await divider.press("Control+ArrowRight");
  await expect(divider).toHaveAttribute("aria-valuenow", "25");
  const selectedUrl=page.url();
  await page.getByLabel("Your message", {exact:true}).focus();
  await page.keyboard.press("ArrowRight");
  await expect(divider).toHaveAttribute("aria-valuenow", "25");
  expect(page.url()).toBe(selectedUrl);
  await page.emulateMedia({reducedMotion:"reduce"});
  for (const width of [1440, 1280, 1024, 390]) {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    if (width === 390) {
      await expect(divider).toBeHidden();
      const imagesTab=page.getByRole("button", {name:"Images (1)",exact:true});
      await imagesTab.focus();await page.keyboard.press("Enter");
      await expect(page.getByLabel("Your message",{exact:true})).toBeHidden();
      await expect(page.getByRole("navigation",{name:"Select image",exact:true})).toBeVisible();
      const conversationTab=page.getByRole("button", {name:"Conversation",exact:true});
      await conversationTab.focus();await page.keyboard.press("Enter");
      await expect(page.getByLabel("Your message",{exact:true})).toBeVisible();
      await imagesTab.focus();await page.keyboard.press("Enter");
    }
    expect(page.url()).toBe(selectedUrl);
    await page.screenshot({ path: resolve(process.env.ANNOTAGENT_E2E_EVIDENCE_DIR ?? "../docs/execution/conversational-workspace",`journal-${width}.png`), fullPage: true, animations: "disabled" });
  }
  expect(charged).toEqual([]);
});

test("large TEST image index renders bounded thumbnails and restores the selected page",async({page,request})=>{
  const project=`conversation-large-${Date.now()}`;
  expect((await request.post("/api/projects",{data:{id:project,yaml:"version: 1\nproject:\n  name: TEST large image index\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}})).ok()).toBeTruthy();
  // Browser-only metadata fixture: this checks DOM growth, not server import throughput.
  const images=Array.from({length:1001},(_,i)=>({image_id:`test-image-${i}`,name:`TEST image ${i}`,url:"/brand/core/pwa-192.png",content_hash:`test-hash-${i}`}));
  await page.route(`**/api/projects/${project}/images`,route=>route.fulfill({json:{images}}));
  await page.goto(`/projects/${project}/work?image=test-image-1000`);
  const thumbnails=page.getByRole("navigation",{name:"Select image",exact:true});
  await expect(thumbnails.getByRole("button")).toHaveCount(17);
  await expect(thumbnails.getByRole("button",{name:"TEST image 1000",exact:true})).toHaveAttribute("aria-current","true");
  const pages=page.getByRole("navigation",{name:"Image pages",exact:true});
  await expect(pages).toContainText("985–1001 of 1001");
  await pages.getByRole("button",{name:"Previous images",exact:true}).click();
  await expect(thumbnails.getByRole("button")).toHaveCount(24);
  expect(new URL(page.url()).searchParams.get("image")).toBe("test-image-1000");
  await thumbnails.getByRole("button",{name:"TEST image 960",exact:true}).click();
  await expect(page).toHaveURL(/image=test-image-960/);
  await page.goBack();
  await expect(pages).toContainText("985–1001 of 1001");
  await page.goForward();
  await page.reload();
  await expect(pages).toContainText("961–984 of 1001");
  await expect(thumbnails.getByRole("button",{name:"TEST image 960",exact:true})).toHaveAttribute("aria-current","true");
  await pages.scrollIntoViewIfNeeded();
  await page.screenshot({path:resolve(process.env.ANNOTAGENT_E2E_EVIDENCE_DIR ?? "../docs/execution/conversational-workspace","large-image-index-TEST.png"),fullPage:true,animations:"disabled"});
});
