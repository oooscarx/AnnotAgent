import { isolatedEvidencePath } from "./evidence";
import { resolve } from "node:path";
import { readFileSync } from "node:fs";
import { expect, test } from "./fixtures";

test("image-first journey saves images and every goal label without model calls", async ({ page, request }) => {
  const inference: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && /suggest|dry-run|agent-sessions/.test(request.url())) inference.push(request.url());
  });
  // Missing-model response is a fixture; Project, image and goal writes use the actual server.
  await page.route("**/api/agent-model-bindings", (route) => route.fulfill({ json: {} }));
  await page.goto("/projects?new=1");
  const upload = page.getByRole("region", { name: "Create Project", exact: true });
  await expect(upload.getByLabel("Project name", { exact: true })).toHaveCount(0);
  await upload.getByLabel("Choose images", { exact: false }).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  const transfer = await page.evaluateHandle((base64) => {
    const data = new DataTransfer();
    const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
    data.items.add(new File([bytes], "dropped-test.png", { type: "image/png" }));
    return data;
  }, readFileSync(resolve("../examples/robocup/images/synthetic-robocup.png")).toString("base64"));
  await upload.locator(".journey-upload").dispatchEvent("drop", { dataTransfer: transfer });
  await expect(upload.getByAltText("dropped-test.png", { exact: true })).toBeVisible();
  await upload.getByRole("button", { name: "Remove selected image dropped-test.png" }).click();
  await expect(upload.getByAltText("synthetic-robocup.png", { exact: true })).toBeVisible();
  await upload.getByRole("button", { name: "Remove selected image synthetic-robocup.png" }).click();
  await expect(upload.getByRole("button", { name: "Continue", exact: true })).toBeDisabled();
  await upload.getByLabel("Choose images", { exact: false }).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await upload.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(page).toHaveURL(/\/work$/);
  const projectId = new URL(page.url()).pathname.split("/")[2];
  // Exercise the retained explicit goal editor, not the default creation route.
  await page.goto(`/projects/${projectId}/task/goal`);
  const url = page.url();
  await page.reload();
  const goal = page.getByRole("region", { name: "Annotation goal", exact: true });
  await expect(goal.getByAltText("synthetic-robocup.png", { exact: true })).toBeVisible();
  await goal.getByLabel("Describe your goal", { exact: true }).fill("Find cups and plates, not bottles.");
  await goal.getByLabel("Categories to keep", { exact: false }).fill("cup，plate");
  await goal.getByRole("button", { name: "Save goal and connect model", exact: true }).click();
  await expect(page).toHaveURL(/\/task\/model$/);
  await page.reload();
  await expect(page.getByRole("heading", { name: "One connection before we prepare your samples." })).toBeVisible();
  await page.getByRole("button", { name: "Connect a model service", exact: true }).click();
  await page.getByLabel("Service URL", { exact: true }).fill("https://not-sent.example/v1");
  await page.getByRole("button", { name: "Cancel connection", exact: true }).click();
  await page.getByRole("button", { name: "Back to saved goal", exact: true }).click();
  await expect(goal.getByText("Goal saved", { exact: true })).toBeVisible();
  expect(inference).toEqual([]);
  const saved = await (await request.get(`/api/projects/${projectId}/goal`)).json();
  expect(saved.labels).toEqual(["cup", "plate"]);
  await page.reload();
  await expect(goal.getByLabel("Describe your goal", { exact: true })).toHaveValue("Find cups and plates, not bottles.");
  await expect(goal.getByLabel("Categories to keep", { exact: false })).toHaveValue("cup, plate");
  await expect(page.locator(".sidebar, .focus-project-menu, .build-steps, .workflow-edit-details")).toHaveCount(0);
  for (const width of [1440, 1280, 1024, 390]) {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    await page.screenshot({ path: isolatedEvidencePath(`../docs/execution/guided-journey/goal-${width}.png`), fullPage: true, animations: "disabled" });
  }
  await goal.getByRole("button", { name: "Back to images", exact: true }).click();
  await expect(page).toHaveURL(/\/task\/images$/);
  await expect(page.getByText("Your images are saved on this server.", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(page).toHaveURL(url);
  expect(inference).toEqual([]);
});
