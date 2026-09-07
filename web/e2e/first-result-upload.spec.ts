import { resolve } from "node:path";
import { expect, test } from "./fixtures";

test("browser images and a goal are saved without any model request", async ({ page }) => {
  const inference: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && /suggest|dry-run|agent-sessions/.test(request.url())) inference.push(request.url());
  });
  await page.goto("/projects?new=1");
  const wizard = page.getByRole("region", { name: "Create Project", exact: true });
  await wizard.getByLabel("Choose images", { exact: true }).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await expect(wizard.getByAltText("synthetic-robocup.png", { exact: true })).toBeVisible();
  await wizard.getByLabel("Project name", { exact: true }).fill(`First result ${Date.now()}`);
  await wizard.getByLabel("Describe your goal", { exact: true }).fill("Find the ball, not the players.");
  await wizard.getByLabel("Object name", { exact: true }).fill("Ball");
  await wizard.getByRole("button", { name: "Save goal and images", exact: true }).click();
  await expect(wizard).not.toBeVisible();
  expect(inference).toEqual([]);
  await page.reload();
  await expect(page.getByText("Find the ball, not the players.", { exact: true }).first()).toBeVisible();
  await expect(page.locator(".guidance-actions .primary")).toHaveText("Choose automation");
  await page.locator(".guidance-actions .primary").click();
  await expect(page).toHaveURL(/\/build\/pipeline\?draft=/);
  const taskUrl = page.url();
  await page.getByRole("button", { name: "Prepare models in this task", exact: true }).click();
  const setup = page.getByRole("region", { name: "Task model preparation", exact: true });
  await expect(setup).toBeVisible();
  await setup.getByRole("button", { name: "Provider connections", exact: true }).click();
  await expect(setup.getByRole("button", { name: "Provider connections", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(setup.getByRole("heading", { name: "Providers", exact: true })).toBeVisible();
  expect(page.url()).toBe(taskUrl);
  await page.screenshot({ path: "../docs/execution/first-result/no-model-preparation.png", animations: "disabled" });
  await setup.getByRole("button", { name: "Return to this Draft", exact: true }).click();
  await expect(setup).not.toBeVisible();
  expect(page.url()).toBe(taskUrl);
  expect(inference).toEqual([]);
});
