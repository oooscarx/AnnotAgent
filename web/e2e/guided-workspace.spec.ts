import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { copyFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";

test.describe.configure({ mode: "serial" });

const stamp = Date.now();
const projectId = `guided-e2e-${stamp}`;
const projectName = `Guided E2E ${stamp}`;
const imageSource = resolve(process.cwd(), `../workspace/e2e-guided/import-${stamp}`);
const screenshots = resolve(process.cwd(), "../docs/execution/screenshots");
let runId = "";
let reviewId = "";

const projectYaml = `version: 1
project:
  name: ${projectName}
  language: en
dataset:
  root: images
runtime:
  max_parallel_images: 1
tasks:
  - id: scene
    kind: classification
    labels: [day, night]
    required: true
review:
  auto_accept_confidence: 0.999
  force_review_below: 0.999
export:
  formats: [native]
`;

mkdirSync(imageSource, { recursive: true });
copyFileSync(
  resolve(process.cwd(), "../examples/robocup/images/synthetic-robocup.png"),
  resolve(imageSource, "synthetic-robocup.png"),
);

async function dashboard(request: APIRequestContext) {
  const response = await request.get("/api/projects");
  expect(response.ok()).toBeTruthy();
  return response.json();
}

async function findCropRun(request: APIRequestContext) {
  const response = await request.get("http://127.0.0.1:8787/api/runs");
  if (!response.ok()) return undefined;
  const { runs } = await response.json();
  for (const run of runs.filter((item: { checkpoint_present: boolean }) => item.checkpoint_present)) {
    const artifacts = await request.get(
      `http://127.0.0.1:8787/api/runs/${run.id}/pipeline-artifacts`,
    );
    if (!artifacts.ok()) continue;
    const inspection = await artifacts.json();
    const cropNode = inspection.nodes.find((node: { outputs: { kind: string }[] }) =>
      node.outputs.some((artifact) => artifact.kind === "crop_set"),
    );
    const detectionNode = inspection.nodes.find((node: { outputs: { kind: string }[] }) =>
      node.outputs.some((artifact) => artifact.kind === "detection_set"),
    );
    if (cropNode && detectionNode)
      return { run, inspection, cropNode, detectionNode };
  }
  return undefined;
}

async function openProject(page: Page) {
  await page.goto(`/projects/${projectId}`);
  await expect(page.getByRole("heading", { name: projectName })).toBeVisible();
}

test("empty workspace stays generic and contains no RoboCup product content", async ({ page }) => {
  await page.route("**/api/projects", async (route) => {
    await route.fulfill({
      json: { projects: [], runs: [], models: [], review_queue: 0 },
    });
  });
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Home" })).toBeFocused();
  await expect(page.getByText("No projects yet")).toBeVisible();
  await expect(page.locator("body")).not.toContainText("RoboCup");
  await page.screenshot({ path: `${screenshots}/01-empty-workspace.png`, fullPage: true });
});

test("create and open a generic Project", async ({ page, request }) => {
  await page.goto("/projects?new=1");
  const dialog = page.getByRole("dialog", { name: "Create Project" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel("project.yaml")).not.toHaveValue("");
  await dialog.getByLabel("Workspace ID").fill(projectId);
  await dialog.getByLabel("project.yaml").fill(projectYaml);
  await expect(dialog.getByLabel("project.yaml")).toHaveValue(projectYaml);
  await dialog.getByRole("button", { name: "Validate & create" }).click();
  await expect(dialog).toBeHidden();
  await expect.poll(async () => {
    const state = await dashboard(request);
    return state.projects.some((project: { id: string }) => project.id === projectId);
  }).toBeTruthy();
  await page.goto(`/projects/${projectId}`);
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}$`));
  await expect(page.getByRole("heading", { name: projectName })).toBeVisible();
});

test("Build navigation preserves the Project and imports real data", async ({ page }) => {
  await openProject(page);
  await page.getByRole("button", { name: "Build", exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/build/data$`));
  await page.getByLabel("Workspace-local file or directory").fill(imageSource);
  await page.getByRole("button", { name: "Add images" }).click();
  await expect(page.getByText(/1 registered/)).toBeVisible();

  for (const [name, path] of [
    ["Labels", "labels"],
    ["Pipeline", "pipeline"],
    ["Test & Publish", "test"],
  ] as const) {
    await page.getByRole("button", { name }).click();
    await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/build/${path}$`));
  }
});

test("Dry Run reports real summary metrics and publishes an immutable version", async ({ page, request }) => {
  const suggestion = await request.post("/api/workflow-drafts/suggest", {
    data: {
      project_id: projectId,
      target_task_id: "scene",
      target_label: "day",
      advisor: "mock",
      constraints: { require_review_gate: true },
    },
  });
  expect(suggestion.status()).toBe(201);

  await page.goto(`/projects/${projectId}/build/test`);
  await expect(page.getByLabel("Current Draft")).not.toHaveValue("");
  await page.getByRole("spinbutton").fill("1");
  await page.getByRole("button", { name: "Test", exact: true }).click();
  await expect(page.getByLabel("Dry Run result summary")).toContainText("Images");
  await expect(page.getByText("Ready to publish")).toBeVisible();
  await page.screenshot({ path: `${screenshots}/02-dry-run-summary.png`, fullPage: true });
  await page.getByRole("button", { name: "Publish", exact: true }).click();
  await expect(page.getByLabel("Current Draft")).toHaveValue("");

  const state = await dashboard(request);
  const project = state.projects.find((item: { id: string }) => item.id === projectId);
  expect(project.default_workflow_version.status).toBe("published");
  const started = await request.post(`/api/projects/${projectId}/runs`, {
    headers: { "idempotency-key": `e2e-${stamp}` },
    data: {
      provider: "mock",
      workflow_id: project.default_workflow_version.workflow_id,
      version: Number(project.default_workflow_version.version),
    },
  });
  expect(started.status()).toBe(202);
  runId = (await started.json()).run_id;
  await expect.poll(async () => {
    const current = await dashboard(request);
    return current.runs.find((run: { id: string }) => run.id === runId)?.status;
  }, { timeout: 30_000 }).toMatch(/completed|completed_with_review/);
  const annotationId = randomUUID();
  const createdReview = await request.post(`/api/runs/${runId}/annotations`, {
    data: {
      annotation: {
        id: annotationId,
        image_id: randomUUID(),
        task_id: "scene",
        label: "day",
        value: { kind: "classification", labels: ["day"] },
        attributes: {},
        confidence: null,
        source: "human",
        review_status: "needs_review",
        provenance: {
          run_step_id: null,
          provider: null,
          model: null,
          tool_names: [],
          parent_annotation_id: null,
          artifact_ids: [],
        },
        created_at: new Date().toISOString(),
      },
    },
  });
  expect(createdReview.status()).toBe(201);
});

test("open Run Artifact from history without entering an ID", async ({ page }) => {
  await page.goto("/runs");
  const row = page.locator(".run-row").filter({ hasText: projectName });
  await row.click();
  await expect(page).toHaveURL(new RegExp(`/runs/${runId}`));
  await expect(page.getByText("Pipeline steps")).toBeVisible();
  await expect(page.locator(".run-node-timeline button").first()).toBeVisible();
});

test("Run URL refresh restores image and node context", async ({ page }) => {
  await page.goto(`/runs/${runId}?image=0&node=core.image_input`);
  await expect(page.locator(".run-node-timeline button.active")).toContainText("core.image_input");
  const nodeInspector = page.getByRole("region", { name: "Node inspector" });
  await expect(nodeInspector).toBeVisible();
  await expect(nodeInspector.locator(".run-node-metrics article")).toHaveCount(3);
  await expect(nodeInspector.locator(".node-payload-section").filter({ has: page.getByText("Output", { exact: true }) })).toHaveAttribute("open", "");
  const imageControlGaps = await page.locator(".run-image-browser input, .run-image-browser select, .run-image-browser > div > button").evaluateAll((elements) =>
    elements.slice(1).map((element, index) =>
      element.getBoundingClientRect().top - elements[index].getBoundingClientRect().bottom,
    ),
  );
  expect(imageControlGaps.every((gap) => gap >= 7)).toBeTruthy();
  const pipelineTextFits = await page.locator(".run-node-timeline button strong, .run-node-timeline button small").evaluateAll((elements) =>
    elements.every((element) => {
      const bounds = element.getBoundingClientRect();
      const buttonBounds = element.closest("button")!.getBoundingClientRect();
      return bounds.left >= buttonBounds.left && bounds.right <= buttonBounds.right + 1
        && getComputedStyle(element).overflow === "hidden";
    }),
  );
  expect(pipelineTextFits).toBeTruthy();
  await expect(page.locator(".run-node-timeline button strong").first()).toHaveCSS("text-overflow", "ellipsis");
  const previewZoom = page.getByLabel("Preview zoom");
  await expect(previewZoom).toBeVisible();
  const zoomAlignment = await page.locator(".preview-zoom-control").evaluate((control) => {
    const range = control.querySelector("input")!.getBoundingClientRect();
    const output = control.querySelector("output")!.getBoundingClientRect();
    return Math.abs((range.top + range.height / 2) - (output.top + output.height / 2));
  });
  expect(zoomAlignment).toBeLessThanOrEqual(1);
  await page.reload();
  await expect(page).toHaveURL(new RegExp(`/runs/${runId}\\?image=0&node=core.image_input`));
  await expect(page.locator(".run-node-timeline button.active")).toContainText("core.image_input");
});

test("Review to Run to Review navigation is bidirectional", async ({ page, request }) => {
  const reviews = await request.get("/api/reviews");
  expect(reviews.ok()).toBeTruthy();
  reviewId = (await reviews.json()).reviews.find(
    (review: { run_id: string }) => review.run_id === runId,
  )?.id;
  expect(reviewId).toBeTruthy();
  await page.goto(`/review/${reviewId}`);
  await page.evaluate(() => window.localStorage.removeItem("annotagent.reviewInspectorCollapsed"));
  await page.reload();
  await expect(page.getByRole("button", { name: "Accept and commit annotation" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Save changes" })).toHaveCount(0);
  await expect(page.locator(".review-add-menu")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Box", exact: true })).toHaveCount(0);
  const widthWithInspector = await page.locator(".review-center").evaluate((element) => element.getBoundingClientRect().width);
  await page.getByRole("button", { name: "Hide details" }).click();
  const widthWithoutInspector = await page.locator(".review-center").evaluate((element) => element.getBoundingClientRect().width);
  expect(widthWithoutInspector).toBeGreaterThan(widthWithInspector);
  const canvasPresentation = await page.locator(".annotation-canvas").evaluate((element) => {
    const canvas = element as SVGSVGElement;
    const rect = canvas.getBoundingClientRect();
    return {
      renderedRatio: rect.width / rect.height,
      sourceRatio: canvas.viewBox.baseVal.width / canvas.viewBox.baseVal.height,
      shellBackground: getComputedStyle(canvas.closest(".canvas-shell")!).backgroundColor,
      usesDarkTheme: canvas.closest(".aa-dark") !== null,
    };
  });
  expect(canvasPresentation.usesDarkTheme).toBeFalsy();
  expect(canvasPresentation.shellBackground).toBe("rgb(255, 255, 255)");
  expect(Math.abs(canvasPresentation.renderedRatio - canvasPresentation.sourceRatio)).toBeLessThan(0.02);
  await page.getByRole("button", { name: "Zoom in" }).click();
  await expect(page.locator(".canvas-tools strong")).toHaveText("110%");
  await page.getByRole("button", { name: "Fit image" }).click();
  await expect(page.locator(".canvas-tools strong")).toHaveText("100%");
  await page.getByRole("button", { name: "Show details" }).click();
  await page.getByRole("button", { name: /Open run context/ }).click();
  await expect(page).toHaveURL(new RegExp(`/runs/${runId}`));
  await page.getByRole("button", { name: "Review result" }).click();
  await expect(page).toHaveURL(new RegExp(`/review/${reviewId}$`));
});

test("Review workspace has tablet and mobile layouts without horizontal overflow", async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 900 });
  await page.goto(`/review/${reviewId}`);
  await expect(page.getByRole("button", { name: "Accept and commit annotation" })).toBeVisible();
  await expect(page.locator(".review-layout")).toHaveCSS("display", "grid");
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(900);

  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator(".review-layout")).toHaveCSS("display", "flex");
  await expect(page.locator(".review-queue .queue-items")).toHaveCSS("display", "flex");
  await expect(page.locator(".review-action-bar")).toHaveCSS("position", "static");
  await expect(page.getByRole("heading", { name: "Review", level: 1 })).toHaveCSS("outline-style", "none");
  await expect(page.locator(".project-switch select")).toBeVisible();
  const mobileLayout = await page.evaluate(() => ({
    viewport: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
    queueWidth: document.querySelector<HTMLElement>(".review-queue")?.getBoundingClientRect().width ?? 0,
    centerWidth: document.querySelector<HTMLElement>(".review-center")?.getBoundingClientRect().width ?? 0,
  }));
  expect(mobileLayout.scrollWidth).toBeLessThanOrEqual(mobileLayout.viewport);
  expect(mobileLayout.queueWidth).toBeLessThanOrEqual(mobileLayout.viewport);
  expect(mobileLayout.centerWidth).toBeLessThanOrEqual(mobileLayout.viewport);
  await page.screenshot({ path: `${screenshots}/03-review-mobile.png`, fullPage: true });
});

test("an active Run restores from the server and locks duplicate Start", async ({ page, request }) => {
  const state = await dashboard(request);
  const project = state.projects.find((item: { id: string }) => item.id === projectId);
  const activeId = "00000000-0000-4000-8000-000000000001";
  project.active_run = { id: activeId, status: "running" };
  project.last_run = project.active_run;
  state.runs.unshift({
    id: activeId,
    project_name: projectName,
    workflow_name: "Guided pipeline",
    workflow_version: "1",
    skill_versions: [],
    model_bindings: [],
    provider: "mock",
    model: "mock",
    status: "running",
    controllable: true,
    input_tokens: 0,
    output_tokens: 0,
    cost: "0",
    artifact_count: 0,
    validation_issue_codes: [],
    retry_count: 0,
    fallback_nodes: [],
    model_identity: "mock",
    timed_out: false,
    checkpoint_present: false,
    review_suspended: false,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  });
  await page.route("**/api/projects", (route) => route.fulfill({ json: state }));
  await page.goto(`/projects/${projectId}`);
  await expect(page.getByRole("button", { name: "Open active run" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Start dataset batch" })).toBeDisabled();
  await page.reload();
  await expect(page.getByRole("button", { name: "Open active run" })).toBeVisible();
});

test("bbox and crop selection stay linked through parent references", async ({ page, request }) => {
  const fixture = await findCropRun(request);
  test.skip(!fixture, "A persisted B-Human DetectionSet and CropSet fixture is required.");
  const cropNodeId = fixture!.cropNode.node_id;
  await page.goto(
    `http://127.0.0.1:8787/runs/${fixture!.run.id}?image=${fixture!.inspection.image_index}&node=${encodeURIComponent(cropNodeId)}`,
  );
  const overlay = page.locator('svg[aria-label="Annotation overlay"]');
  await expect(overlay.getByRole("button").first()).toBeVisible();
  await expect(overlay.locator("g.selected")).toHaveCount(1);
  await page.getByRole("button", { name: /Crops \(1\)/ }).click();
  await expect(page.locator(".crop-preview-list button.selected")).toHaveCount(1);
  await page.getByRole("button", { name: "Image", exact: true }).click();
  await expect(overlay.locator("g.selected")).toHaveCount(1);
  await page.screenshot({ path: `${screenshots}/03-run-artifact-lineage.png`, fullPage: true });
});

test("generic Project routes contain no RoboCup-specific copy", async ({ page }) => {
  await page.goto(`/projects/${projectId}/build/pipeline`);
  await expect(page.locator("body")).not.toContainText("RoboCup");
  await expect(page.getByText("Shared Stages", { exact: true })).toBeVisible();
});
