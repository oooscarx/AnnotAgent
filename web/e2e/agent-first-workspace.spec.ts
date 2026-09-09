import { expect, test } from "./fixtures";

test("artifact pane opens on demand and restores without losing unsent text", async ({ page, request }) => {
  const id = `TEST-agent-pane-${Date.now()}`;
  expect((await request.post("/api/projects", { data: { id, yaml:
    "version: 1\nproject:\n  name: TEST optional artifact pane\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n",
  } })).ok()).toBe(true);
  await page.goto(`/projects/${id}/work`);
  const input = page.getByRole("textbox", { name: "Your message", exact: true });
  await expect(input).toBeEnabled();
  await expect(page.getByRole("region", { name: "Project images", exact: true })).not.toBeVisible();
  await input.fill("TEST unsent target stays here");
  await page.getByRole("button", { name: "Open data and results", exact: true }).click();
  await expect(page).toHaveURL(/pane=artifacts/);
  await expect(page.getByRole("region", { name: "Project images", exact: true })).toBeVisible();
  await expect(page.getByRole("separator", { name: "Resize conversation panel" })).toHaveAttribute("aria-valuenow", "54");
  await expect(input).toHaveValue("TEST unsent target stays here");
  await page.getByRole("button", { name: "Close data and results", exact: true }).click();
  await expect(input).toHaveValue("TEST unsent target stays here");
  await page.getByRole("button", { name: "Open data and results", exact: true }).click();
  await expect(input).toHaveValue("TEST unsent target stays here");
  // Unsent composer reload persistence is a separate M3 requirement. This test
  // verifies same-object pane toggles and the persisted URL, not saved messages.
  await input.fill("");
  await page.reload();
  await expect(page.getByRole("region", { name: "Project images", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Close data and results", exact: true }).click();
  await page.reload();
  await expect(page.getByRole("region", { name: "Project images", exact: true })).not.toBeVisible();
});

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
