import { expect, test } from "./fixtures";

// M0 acceptance contract. Real isolated Project; no prototype messages/models.
test("Agent-first empty task centers the thread and exposes one send control", async ({ page, request }) => {
  const id = `TEST-agent-first-${Date.now()}`;
  const created = await request.post("/api/projects", { data: { id, yaml:
    "version: 1\nproject:\n  name: TEST Agent-first baseline\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n",
  } });
  expect(created.ok()).toBe(true);
  await page.goto(`/projects/${id}/work`);
  await expect(page.getByRole("region", { name: "Annotation workspace", exact: true })).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Your message", exact: true })).toBeVisible();
  await page.screenshot({ path: `/tmp/annotagent-agent-first-m0.png`, fullPage: true });
  await expect.soft(page.getByRole("navigation", { name: "Conversations", exact: true })).toBeVisible();
  await expect.soft(page.getByRole("button", { name: "Send", exact: true })).toBeVisible();
  await expect.soft(page.getByRole("button", { name: /Save (goal|message)/ })).toHaveCount(0);
  await expect.soft(page.locator(".conversation-image-empty")).not.toBeVisible();
});
