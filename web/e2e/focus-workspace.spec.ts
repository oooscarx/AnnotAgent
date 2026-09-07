import { expect, test } from "./fixtures";

test("one canonical project inventory opens page-level preparation without mutations", async ({ page }) => {
  const mutations: string[] = [];
  page.on("request", (request) => {
    if (request.url().includes("/api/") && !["GET", "HEAD"].includes(request.method())) mutations.push(request.url());
  });
  await page.goto("/");
  await expect(page).toHaveURL(/\/projects$/);
  await expect(page.getByRole("searchbox", { name: "Search projects", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "New annotation project", exact: true }).click();
  await expect(page).toHaveURL(/\/projects\?new=1$/);
  await expect(page.locator(".app-shell")).toHaveAttribute("data-layout", "focus");
  await expect(page.locator(".sidebar")).toHaveCount(0);
  await expect(page.getByRole("region", { name: "Create Project", exact: true })).toBeVisible();
  await expect(page.getByRole("dialog", { name: "Create Project", exact: true })).toHaveCount(0);
  await page.reload();
  await expect(page.getByRole("region", { name: "Create Project", exact: true })).toBeVisible();
  expect(mutations).toEqual([]);
  await page.getByRole("button", { name: "Back to projects", exact: true }).click();
  await expect(page).toHaveURL(/\/projects$/);
});
