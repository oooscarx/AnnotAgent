import { expect, test } from "./fixtures";

test("artifact pane opens on demand and restores without losing unsent text", async ({ page, request }) => {
  const id = `TEST-agent-pane-${Date.now()}`;
  expect((await request.post("/api/projects", { data: { id, yaml:
    "version: 1\nproject:\n  name: TEST optional artifact pane\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n",
  } })).ok()).toBe(true);
  await page.goto(`/projects/${id}/work`);
  const input = page.getByRole("textbox", { name: "Your message", exact: true });
  await expect(input).toBeEnabled();
  const navigation = page.getByRole("navigation", { name: "Conversations", exact: true });
  await expect(navigation).toContainText("No saved tasks yet");
  await navigation.getByRole("button", { name: "Hide tasks", exact: true }).click();
  await expect(navigation.getByRole("searchbox")).not.toBeVisible();
  await navigation.getByRole("button", { name: "Show tasks", exact: true }).click();
  await expect(navigation.getByRole("searchbox")).toBeVisible();
  for (const theme of ["light", "dark"] as const) {
    await page.evaluate(value => document.documentElement.setAttribute("data-aa-theme", value), theme);
    await expect(page.locator("body")).toHaveCSS("background-color", theme === "light" ? "rgb(247, 247, 244)" : "rgb(24, 26, 24)");
    await expect(navigation.getByRole("button", { name: "Projects", exact: true })).toHaveCSS("background-color", theme === "light" ? "rgb(255, 255, 255)" : "rgb(32, 35, 32)");
    await page.screenshot({ path: `/tmp/annotagent-agent-first-${theme}-navigation.png`, fullPage: true, animations: "disabled" });
  }
  await page.evaluate(() => document.documentElement.setAttribute("data-aa-theme", "light"));
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

test("mobile task navigation opens on demand without covering the composer", async ({ page, request }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const id = `TEST-agent-mobile-navigation-${Date.now()}`;
  expect((await request.post("/api/projects", { data: { id, yaml:
    "version: 1\nproject:\n  name: TEST mobile task navigation\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n",
  } })).ok()).toBe(true);
  await page.goto(`/projects/${id}/work`);
  const nav = page.getByRole("navigation", { name: "Conversations", exact: true });
  await expect(nav.getByRole("searchbox")).not.toBeVisible();
  await nav.getByRole("button", { name: "Show tasks", exact: true }).click();
  await expect(nav).toContainText("No saved tasks yet");
  await nav.getByRole("button", { name: "Hide tasks", exact: true }).click();
  const input = page.getByRole("textbox", { name: "Your message", exact: true });
  await input.fill("测试输入保持可达");
  await expect(input).toHaveValue("测试输入保持可达");
  await input.press("Shift+Enter");
  await expect(input).toHaveValue("测试输入保持可达\n");
  let messageWrites = 0;
  page.on("request", request => {
    if (request.method() === "POST" && /\/send$/.test(new URL(request.url()).pathname)) messageWrites++;
  });
  await input.dispatchEvent("compositionstart");
  await input.press("Enter");
  expect(messageWrites).toBe(0);
  await input.dispatchEvent("compositionend");
  await input.dispatchEvent("keydown", { key: "Enter", keyCode: 229 });
  expect(messageWrites).toBe(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await input.press("Enter");
  await expect(page.getByRole("list", { name: "Saved messages", exact: true })).toContainText("测试输入保持可达");
  expect(messageWrites).toBe(1);
  await expect(input).toHaveValue("");
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
