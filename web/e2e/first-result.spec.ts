import { expect, test } from "./fixtures";
import { resolve } from "node:path";

test("first-result entry and offline example do not perform server mutations", async ({ page }) => {
  const mutations: string[] = [];
  page.on("request", request => {
    if (request.url().includes("/api/") && !["GET", "HEAD"].includes(request.method())) mutations.push(request.url());
  });
  await page.goto("/");
  await page.screenshot({ path: resolve("../docs/execution/first-result/entry.png"), fullPage: true });
  await expect(page.getByRole("button", { name: "Start with images", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Explore an example", exact: true }).click();
  const example = page.getByRole("dialog", { name: "Offline annotation example" });
  await expect(example.getByText("Offline demo · no model inference", { exact: true })).toBeVisible();
  await example.getByRole("button", { name: "See a sample", exact: true }).click();
  await expect(example.getByText("Offline demo · no model inference", { exact: true })).toBeVisible();
  await expect(example.getByRole("button", { name: "Use my images", exact: true })).toBeVisible();
  await example.getByRole("button", { name: "Close", exact: true }).click();
  expect(mutations).toEqual([]);
});

for (const viewport of [{ width: 1440, height: 900 }, { width: 1024, height: 768 }, { width: 390, height: 844 }]) {
  test(`offline entry and editor fit ${viewport.width}×${viewport.height}`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.goto("/");
    await page.getByRole("button", { name: "Explore an example", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Offline annotation example", exact: true });
    await dialog.getByRole("button", { name: "See a sample", exact: true }).click();
    await expect(dialog.getByRole("button", { name: "Select example box", exact: true })).toBeVisible();
    expect(await dialog.evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBeTruthy();
    await page.keyboard.press("Escape");
    await expect(dialog).not.toBeVisible();
    await expect(page.getByRole("button", { name: "Explore an example", exact: true })).toBeFocused();
  });
}
