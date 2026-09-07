import { expect, test } from "./fixtures";

test("first-result entry and offline example do not perform server mutations", async ({ page }) => {
  const mutations: string[] = [];
  page.on("request", request => {
    if (request.url().includes("/api/") && !["GET", "HEAD"].includes(request.method())) mutations.push(request.url());
  });
  await page.goto("/");
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
