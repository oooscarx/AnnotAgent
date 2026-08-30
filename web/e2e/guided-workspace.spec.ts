import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { resolve } from "node:path";

test.describe.configure({ mode: "serial" });

const stamp = Date.now();
const projectId = `guided-e2e-${stamp}`;
const projectName = `Guided E2E ${stamp}`;
const emptyProjectId = `guided-empty-${stamp}`;
const agentDiffProjectId = `guided-agent-diff-${stamp}`;
const robocupAgentProjectId = `guided-robocup-agent-${stamp}`;
const cropProjectId = `guided-crop-${stamp}`;
const mixedProjectId = `guided-mixed-${stamp}`;
const mixedRunId = "00000000-0000-4000-8000-000000000091";
const mixedReviewId = "00000000-0000-4000-8000-000000000092";
const screenshots = resolve(process.cwd(), "../docs/execution/screenshots");
let runId = "";
let reviewId = "";
let reviewImageId = "";
let cropRunId = "";

async function dashboard(request: APIRequestContext) {
  const response = await request.get("/api/projects");
  expect(response.ok()).toBeTruthy();
  return response.json();
}

async function findCropRun(request: APIRequestContext, targetProjectId: string) {
  const response = await request.get("/api/runs");
  if (!response.ok()) return undefined;
  const { runs } = await response.json();
  for (const run of runs.filter((item: { checkpoint_present: boolean }) => item.checkpoint_present)) {
    const artifacts = await request.get(`/api/runs/${run.id}/pipeline-artifacts`);
    if (!artifacts.ok()) continue;
    const inspection = await artifacts.json();
    if (inspection.project_id !== targetProjectId) continue;
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

async function createCropRun(request: APIRequestContext, page: Page, imageSource: string) {
  const created = await request.post("/api/projects", {
    data: {
      id: cropProjectId,
      yaml: `version: 1
project:
  name: Guided Crop Fixture ${stamp}
  language: en
dataset:
  root: images
runtime:
  max_parallel_images: 1
tasks:
  - id: ball-objects
    display_name: Ball
    kind: bounding_box
    labels: [ball]
    required: true
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native, coco, yolo]
`,
    },
  });
  expect(created.status()).toBe(201);
  const imported = await request.post(`/api/projects/${cropProjectId}/import`, {
    data: { source: imageSource },
  });
  expect(imported.ok()).toBeTruthy();

  const suggested = await request.post("/api/workflow-drafts/suggest", {
    data: {
      project_id: cropProjectId,
      target_task_id: "ball-objects",
      target_label: "ball",
      advisor: "mock",
      constraints: { require_review_gate: true },
    },
  });
  expect(suggested.status()).toBe(201);
  const suggestion = await suggested.json();
  await page.goto(`/projects/${cropProjectId}/build/pipeline`);
  await expect(page.getByText("Shared Stages", { exact: true })).toBeVisible();
  await page.getByText("Edit automation", { exact: true }).click();
  const autosaved = page.waitForResponse((response) =>
    response.request().method() === "PATCH" && response.url().includes(`/api/workflow-drafts/${suggestion.draft.id}`),
  );
  await page.getByRole("button", { name: "Add detection + crop" }).click();
  const saved = await autosaved;
  expect(saved.ok()).toBeTruthy();
  const draft = await saved.json();
  const steps = draft.label_pipeline.label_pipelines[0].steps;
  expect(steps.some((step: { node_type: string }) => step.node_type === "core.crop")).toBeTruthy();
  expect(steps.some((step: { node_type: string }) => step.node_type === "core.artifact_cache")).toBeTruthy();
  const dryRun = await request.post(`/api/workflow-drafts/${draft.id}/dry-run`, {
    data: { image_indices: [0] },
  });
  expect(dryRun.ok()).toBeTruthy();
  expect((await dryRun.json()).validation.valid).toBeTruthy();
  const published = await request.post(`/api/workflow-drafts/${draft.id}/publish`);
  expect(published.ok()).toBeTruthy();
  const version = await published.json();
  const started = await request.post(`/api/projects/${cropProjectId}/runs`, {
    headers: { "idempotency-key": `crop-e2e-${stamp}` },
    data: {
      provider: "mock",
      workflow_id: version.workflow_id,
      version: version.version,
    },
  });
  expect(started.status()).toBe(202);
  ({ run_id: cropRunId } = await started.json());
  await expect.poll(async () => {
    const state = await dashboard(request);
    return state.runs.find((run: { id: string }) => run.id === cropRunId)?.status;
  }, { timeout: 30_000 }).toMatch(/completed|completed_with_review/);
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

test("create and open a generic Project", async ({ page, request }, testInfo) => {
  const imageSource = String(testInfo.config.metadata.e2eImport);
  await page.goto("/projects?new=1");
  const dialog = page.getByRole("dialog", { name: "Create Project" });
  await expect(dialog).toBeVisible();
  await dialog.getByText("Classify images", { exact: true }).click();
  await dialog.getByLabel("Project name").fill(projectName);
  await dialog.getByLabel("Class name").fill("Day");
  await dialog.getByRole("button", { name: "Continue" }).click();
  await dialog.getByLabel("Image file or folder").fill(imageSource);
  await dialog.getByRole("button", { name: "Continue" }).click();
  await dialog.getByText("Balanced", { exact: true }).click();
  await dialog.getByRole("button", { name: "Continue" }).click();
  await dialog.getByLabel("Provider").selectOption("mock");
  await page.setViewportSize({ width: 1024, height: 900 });
  await page.screenshot({ path: `${screenshots}/02-guided-project-wizard.png` });
  await page.setViewportSize({ width: 720, height: 450 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
  await dialog.getByRole("button", { name: "Use recommendation" }).click();
  await expect(dialog).toBeHidden();
  await expect.poll(async () => {
    const state = await dashboard(request);
    return state.projects.some((project: { id: string }) => project.id === projectId);
  }).toBeTruthy();
  await page.goto(`/projects/${projectId}`);
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}$`));
  await expect(page.getByRole("heading", { name: projectName })).toBeVisible();
  await expect(page.locator(".guidance-hero h2")).toBeVisible();
  await expect(page.locator(".guidance-actions .primary")).toHaveCount(1);
  await expect(page.locator(".guidance-actions .primary")).toHaveText(/Test on samples|Activate automation/);
  const restoredAction = await page.locator(".guidance-actions .primary").textContent();
  await expect(page.locator(".journey-timeline li")).toHaveCount(8);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.screenshot({ path: `${screenshots}/03-project-guidance.png`, fullPage: true });
  await page.reload();
  await expect(page.locator(".guidance-actions .primary")).toHaveText(restoredAction ?? "");
});

test("Project blocker exposes one server-owned repair action", async ({ page, request }) => {
  const created = await request.post("/api/projects", {
    data: {
      id: emptyProjectId,
      yaml: `version: 1
project:
  name: Empty Guidance ${stamp}
  language: en
dataset:
  root: images
runtime:
  max_parallel_images: 1
tasks:
  - id: subject-class
    display_name: Subject
    kind: classification
    labels: [subject]
    required: true
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native]
`,
    },
  });
  expect(created.status()).toBe(201);
  await page.goto(`/projects/${emptyProjectId}`);
  await expect(page.locator(".guidance-actions .primary")).toHaveText("Add images");
  await expect(page.getByLabel("Project blockers")).toContainText("No images yet");
  await expect(page.locator(".journey-timeline li.current")).toContainText("Data");
});

test("Build blocks direct navigation past an incomplete prerequisite", async ({ page }) => {
  await page.goto(`/projects/${emptyProjectId}/build/test`);
  await expect(page.getByLabel("Build step blocked")).toContainText("Add images to start this Project");
  await expect(page.getByRole("button", { name: "Test & Activate" })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Add images" })).toBeVisible();
});

test("Build navigation preserves the Project and imports real data", async ({ page }) => {
  await openProject(page);
  await page.getByRole("button", { name: "Build", exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/build/data$`));
  await expect(page.getByText(/1 registered/)).toBeVisible();
  await expect(page.getByRole("heading", { name: "Add images to your Project" })).toBeVisible();
  await expect(page.locator(".build-image-list article")).toHaveCount(1);
  await page.screenshot({ path: `${screenshots}/04-build-data.png`, fullPage: true });
  await page.reload();
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/build/data$`));

  for (const [name, path] of [
    ["Labels", "labels"],
    ["Automation", "pipeline"],
    ["Test & Activate", "test"],
  ] as const) {
    await page.getByLabel("Build steps").getByRole("button").filter({ hasText: name }).click();
    await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/build/${path}$`));
    if (path === "labels") {
      await expect(page.getByRole("heading", { name: "What do you want to annotate?" })).toBeVisible();
      await page.screenshot({ path: `${screenshots}/05-build-labels.png`, fullPage: true });
    }
    if (path === "pipeline")
      await expect(page.getByRole("heading", { name: "How AnnotAgent will label your data" })).toBeVisible();
  }
});

test("Automation Recipe previews Advisor changes and autosaves Drawer edits", async ({ page }) => {
  await page.goto(`/projects/${projectId}/build/pipeline`);
  await expect(page.getByRole("heading", { name: "How AnnotAgent will label your data" })).toBeVisible();
  await expect(page.getByText("Shared Stages", { exact: true })).toBeVisible();
  await expect(page.getByText(/Runs once per image/).first()).toBeVisible();
  await expect(page.locator(".pipeline-step-card > code")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Publish", exact: true })).toHaveCount(0);

  await page.getByRole("button", { name: "Ask AnnotAgent" }).click();
  await expect(page.getByRole("heading", { name: "Proposed Changes" })).toBeVisible();
  await expect(page.getByLabel("Draft Diff")).toBeVisible();
  await expect(page.getByText("Why", { exact: true })).toBeVisible();
  const agentTrace = page.getByLabel("pipeline_builder Agent trace");
  await expect(agentTrace).toContainText("Ready for your review");
  await agentTrace.getByText(/Tool actions/).click();
  await expect(agentTrace).toContainText("validate_pipeline");
  await expect(agentTrace).toContainText("dry_run_pipeline");
  await page.getByRole("button", { name: "Reject proposal" }).click();
  await expect(page.getByRole("heading", { name: "Proposed Changes" })).toBeHidden();

  const configure = page.getByRole("button", { name: "Configure node" }).first();
  await expect(configure).toBeVisible();
  await configure.click();
  const drawer = page.getByRole("dialog");
  await drawer.getByText("Expert details", { exact: true }).click();
  const parameters = drawer.getByLabel("Parameters and class mapping");
  const current = JSON.parse(await parameters.inputValue());
  const autosaved = page.waitForResponse((response) =>
    response.request().method() === "PATCH" && response.url().includes("/api/workflow-drafts/"),
  );
  await parameters.fill(JSON.stringify({ ...current, guided_e2e: true }, null, 2));
  expect((await autosaved).ok()).toBeTruthy();
  await drawer.getByRole("button", { name: "Close node configuration" }).click();

  await page.getByText("View technical graph", { exact: true }).click();
  await expect(page.getByLabel("Technical graph JSON")).toBeVisible();
  await page.screenshot({ path: `${screenshots}/06-automation-recipe.png`, fullPage: true });
});

test("Pipeline Builder applies a structured Draft Diff and restores it with Undo", async ({ page, request }, testInfo) => {
  const created = await request.post("/api/projects", {
    data: {
      id: agentDiffProjectId,
      yaml: `version: 1
project:
  name: Agent Diff ${stamp}
  language: en
dataset:
  root: images
runtime:
  max_parallel_images: 1
tasks:
  - id: components
    display_name: Components
    kind: bounding_box
    labels: [component]
    required: true
review:
  auto_accept_confidence: 0.99
  force_review_below: 0.5
export:
  formats: [native, coco]
`,
    },
  });
  expect(created.status()).toBe(201);
  const imported = await request.post(`/api/projects/${agentDiffProjectId}/import`, {
    data: { source: String(testInfo.config.metadata.e2eImport) },
  });
  expect(imported.ok()).toBeTruthy();
  const baseResponse = await request.post("/api/workflow-drafts/suggest", {
    data: {
      project_id: agentDiffProjectId,
      target_task_id: "components",
      target_label: "component",
      advisor: "mock",
      constraints: { require_review_gate: true },
      builder_constraints: {
        priority: "balanced",
        max_model_calls_per_image: 4,
        target_review_rate: 1,
        allow_external_models: false,
        allow_human_review: true,
        maximum_agent_turns: 16,
        maximum_tool_calls: 48,
        maximum_dry_runs: 3,
        maximum_agent_cost: "1",
      },
    },
  });
  expect(baseResponse.status()).toBe(201);
  const base = (await baseResponse.json()).draft;
  expect(base.nodes.some((node: { node_type: string }) => node.node_type === "core.crop")).toBeFalsy();

  await page.goto(`/projects/${agentDiffProjectId}/build/pipeline`);
  const proposalResponse = page.waitForResponse((response) =>
    response.request().method() === "POST" && response.url().endsWith("/api/workflow-drafts/suggest"),
  );
  await page.getByRole("button", { name: "Ask AnnotAgent" }).click();
  const proposal = await (await proposalResponse).json();
  const toolNames = proposal.agent_session.steps.map((step: { tool_name: string }) => step.tool_name);
  expect(toolNames.filter((name: string) => name === "validate_pipeline")).toHaveLength(3);
  expect(toolNames.filter((name: string) => name === "dry_run_pipeline")).toHaveLength(2);
  expect(toolNames.indexOf("disconnect_pipeline_nodes")).toBeLessThan(toolNames.indexOf("connect_pipeline_nodes"));
  expect(toolNames).toContain("add_pipeline_node");
  expect(proposal.agent_session.status).toBe("waiting_for_human");
  const diff = page.getByLabel("Draft Diff");
  await expect(diff).toBeVisible({ timeout: 30_000 });
  await expect(diff.getByRole("checkbox").first()).toBeVisible();
  await page.getByRole("button", { name: "Apply selected" }).click();
  await expect(page.getByRole("button", { name: "Undo Agent changes" })).toBeVisible();

  await expect.poll(async () => {
    const response = await request.get(`/api/workflow-drafts?project_id=${agentDiffProjectId}`);
    const current = (await response.json()).drafts.find((draft: { id: string }) => draft.id === base.id);
    return current.nodes.some((node: { node_type: string }) => node.node_type === "core.crop");
  }).toBeTruthy();

  await page.getByRole("button", { name: "Undo Agent changes" }).click();
  await expect.poll(async () => {
    const response = await request.get(`/api/workflow-drafts?project_id=${agentDiffProjectId}`);
    const current = (await response.json()).drafts.find((draft: { id: string }) => draft.id === base.id);
    return current.nodes.some((node: { node_type: string }) => node.node_type === "core.crop");
  }).toBeFalsy();
});

test("RoboCup Agent loads Domain advice, avoids unavailable Labs, and restores bounded stops", async ({ page, request }, testInfo) => {
  const created = await request.post("/api/projects", {
    data: {
      id: robocupAgentProjectId,
      yaml: `version: 1
project:
  name: Lean RoboCup Agent ${stamp}
  skill: robocup
  skill_version: "1"
  language: en
dataset:
  root: images
runtime:
  max_parallel_images: 1
tasks:
  - id: objects
    display_name: Football
    kind: bounding_box
    labels: [ball]
    required: true
    validators: [ball_hard_negative, robocup_ball_field_relation]
review:
  auto_accept_confidence: 0.92
  force_review_below: 0.72
export:
  formats: [native, coco, yolo]
`,
    },
  });
  expect(created.status()).toBe(201);
  const imported = await request.post(`/api/projects/${robocupAgentProjectId}/import`, {
    data: { source: String(testInfo.config.metadata.e2eImport) },
  });
  expect(imported.ok()).toBeTruthy();

  const advised = await request.post("/api/workflow-drafts/suggest", {
    data: {
      project_id: robocupAgentProjectId,
      target_task_id: "objects",
      target_label: "ball",
      advisor: "mock",
      constraints: { require_review_gate: true },
      builder_constraints: {
        priority: "balanced",
        target_review_rate: 1,
        allow_external_models: false,
        allow_human_review: true,
        maximum_agent_turns: 16,
        maximum_tool_calls: 48,
        maximum_dry_runs: 3,
        maximum_agent_cost: "1",
      },
    },
  });
  expect(advised.status()).toBe(201);
  const suggestion = await advised.json();
  const tools = suggestion.agent_session.steps.map((step: { tool_name: string }) => step.tool_name);
  expect(tools).toContain("load_skill_resource");
  expect(tools.indexOf("load_skill_resource")).toBeLessThan(tools.indexOf("create_draft_from_template"));
  expect(suggestion.agent_session.status).toBe("waiting_for_human");
  expect(suggestion.draft.nodes.filter((node: { model_binding?: string }) => node.model_binding)).toHaveLength(1);
  expect(suggestion.draft.nodes.some((node: { node_type: string }) => /sam|recovery|crop/i.test(node.node_type))).toBeFalsy();

  const registry = await (await request.get("/api/models")).json();
  const unavailableModelIds = new Set(
    registry.models
      .filter((model: { availability_group: string }) => ["configured_unavailable", "labs", "disabled"].includes(model.availability_group))
      .map((model: { id: string }) => model.id),
  );
  expect(unavailableModelIds.size).toBeGreaterThan(0);
  expect(suggestion.draft.nodes.some((node: { model_binding?: string }) =>
    node.model_binding ? unavailableModelIds.has(node.model_binding) : false,
  )).toBeFalsy();

  const cancelled = await request.post(`/api/agent-sessions/${suggestion.agent_session.id}/cancel`);
  expect(cancelled.ok()).toBeTruthy();
  expect((await cancelled.json()).session.status).toBe("cancelled");

  const budgetLimited = await request.post("/api/workflow-drafts/suggest", {
    data: {
      project_id: robocupAgentProjectId,
      target_task_id: "objects",
      target_label: "ball",
      advisor: "mock",
      constraints: { require_review_gate: true },
      builder_constraints: {
        priority: "balanced",
        target_review_rate: 1,
        allow_external_models: false,
        allow_human_review: true,
        maximum_agent_turns: 16,
        maximum_tool_calls: 1,
        maximum_dry_runs: 3,
        maximum_agent_cost: "1",
      },
    },
  });
  expect(budgetLimited.status()).toBe(400);
  const sessions = await (await request.get(`/api/projects/${robocupAgentProjectId}/agent-sessions`)).json();
  expect(sessions.sessions[0].status).toBe("budget_exceeded");
  expect(sessions.sessions.some((session: { status: string }) => session.status === "cancelled")).toBeTruthy();

  await page.goto(`/projects/${robocupAgentProjectId}`);
  await expect(page.getByRole("heading", { name: `Lean RoboCup Agent ${stamp}` })).toBeVisible();
  await expect(page.getByLabel("pipeline_builder Agent trace").first()).toContainText("Stopped at budget");
  await page.reload();
  await expect(page.getByLabel("pipeline_builder Agent trace").first()).toContainText("Stopped at budget");
});

test("Dry Run reports real summary metrics and publishes an immutable version", async ({ page, request }) => {
  await page.goto(`/projects/${projectId}/build/test`);
  await expect(page.getByLabel("Current Draft")).not.toHaveValue("");
  await page.getByRole("spinbutton").fill("1");
  await page.getByRole("button", { name: "Test samples", exact: true }).click();
  await expect(page.getByLabel("Dry Run result summary")).toContainText("Images");
  await expect(page.getByRole("heading", { name: "Sample test complete" })).toBeVisible();
  await expect(page.getByText("Ready to activate")).toBeVisible();
  await expect(page.getByLabel("Full Run Estimate")).toContainText("Review workload");
  await expect(page.getByRole("heading", { name: "What the automation found" })).toBeVisible();
  await expect(page.locator(".sample-result-card").first()).toContainText("day");
  await expect(page.getByRole("heading", { name: "What needs a human decision" })).toBeVisible();
  await expect(page.getByText("No uncertain results in this sample")).toBeVisible();
  await expect(page.locator(".sample-diagnostics details[open]")).toHaveCount(0);
  await expect(page.locator(".sample-outcome-metrics > div")).toHaveCount(3);
  await page.screenshot({ path: `${screenshots}/02-dry-run-summary.png`, fullPage: true });
  await page.setViewportSize({ width: 720, height: 450 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBeTruthy();
  await expect(page.getByRole("button", { name: "Activate automation", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Activate automation", exact: true }).click();
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
  reviewImageId = randomUUID();
  const createdReview = await request.post(`/api/runs/${runId}/annotations`, {
    data: {
      annotation: {
        id: annotationId,
        image_id: reviewImageId,
        task_id: "day-class",
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
  await expect(page.getByRole("button", { name: "Results", exact: true })).toHaveAttribute("aria-current", "page");
  await expect(page.getByLabel("Run result summary")).toContainText("Accepted");
  await expect(page.getByLabel("Run result summary")).toContainText("Needs review");
  await expect(page.getByText("Result Preview", { exact: true })).toBeVisible();
  await expect(page.getByText("Pipeline Steps", { exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Original", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Result", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Compare", exact: true })).toBeVisible();
  await page.screenshot({ path: `${screenshots}/07-run-results.png`, fullPage: true });
  await page.setViewportSize({ width: 720, height: 450 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBeTruthy();
  await page.getByRole("button", { name: "Debug", exact: true }).click();
  await expect(page).toHaveURL(/view=debug/);
  await expect(page.getByText("Pipeline Steps", { exact: true })).toBeVisible();
  await expect(page.locator(".run-node-timeline button").first()).toBeVisible();
});

test("global Runs and Review ignore hidden active Project state", async ({ page }) => {
  await page.goto("/");
  await page.evaluate((id) => window.localStorage.setItem("annotagent.activeProjectId", id), projectId);
  await page.goto("/runs");
  await expect(page.getByLabel("Project filter")).toHaveValue("");
  await expect(page).toHaveURL(/\/runs$/);

  await page.goto("/review");
  await expect(page.getByLabel("Project filter")).toHaveValue("");
  await expect(page).toHaveURL(/\/review(?:\/[^?]+)?$/);

  await page.goto(`/review?project_id=${projectId}`);
  await expect(page.getByLabel("Project filter")).toHaveValue(projectId);
  await expect(page).toHaveURL(new RegExp(`/review(?:/[^?]+)?\\?project_id=${projectId}$`));
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
  await page.screenshot({ path: `${screenshots}/08-run-debug.png`, fullPage: true });
  await page.reload();
  await expect(page).toHaveURL(new RegExp(`/runs/${runId}\\?view=debug&image=0&node=core.image_input`));
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
  await expect(page.getByRole("button", { name: "Accept and next" })).toBeVisible();
  await expect(page.getByLabel("Review progress")).toContainText("0 of 1 results reviewed");
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
  await page.getByRole("button", { name: /Review \d+ result/ }).click();
  await expect(page).toHaveURL(new RegExp(`/review/${reviewId}$`));
});

test("Review workspace has tablet and mobile layouts without horizontal overflow", async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 900 });
  await page.goto(`/review/${reviewId}`);
  await expect(page.getByRole("button", { name: "Accept and next" })).toBeVisible();
  await expect(page.locator(".review-layout")).toHaveCSS("display", "grid");
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(900);

  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator(".review-layout")).toHaveCSS("display", "flex");
  await expect(page.locator(".review-queue .queue-items")).toHaveCSS("display", "flex");
  await expect(page.locator(".review-action-bar")).toHaveCSS("position", "static");
  await expect(page.getByRole("heading", { name: "Review", level: 1 })).toHaveCSS("outline-style", "none");
  await expect(page.getByLabel("Project filter")).toBeVisible();
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
  const summaryResponse = await request.get(`/api/projects/${projectId}/summary`);
  expect(summaryResponse.ok()).toBeTruthy();
  const summary = await summaryResponse.json();
  const activeId = "00000000-0000-4000-8000-000000000001";
  project.active_run = { id: activeId, status: "running" };
  project.last_run = project.active_run;
  summary.project.active_run = project.active_run;
  summary.guidance.stage = "running";
  summary.guidance.headline = "Your dataset run is in progress.";
  summary.guidance.explanation = "Open the active Run to follow server-owned progress.";
  summary.guidance.primary_action = {
    kind: "open_active_run",
    label: "Open active run",
    destination: `/runs/${activeId}`,
    enabled: true,
    disabled_reason: null,
  };
  summary.guidance.journey = summary.guidance.journey.map((step: { id: string; state: string; detail: string }) =>
    step.id === "full_run" ? { ...step, state: "current", detail: "In progress" } : step,
  );
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
  await page.route(`**/api/projects/${projectId}/summary`, (route) => route.fulfill({ json: summary }));
  await page.goto(`/projects/${projectId}`);
  await expect(page.getByRole("button", { name: "Open active run" })).toBeVisible();
  await expect(page.locator(".guidance-actions .primary")).toHaveCount(1);
  await expect(page.getByRole("button", { name: "Run dataset" })).toHaveCount(0);
  await page.reload();
  await expect(page.getByRole("button", { name: "Open active run" })).toBeVisible();
});

test("bbox and crop selection stay linked through parent references", async ({ page, request }, testInfo) => {
  await createCropRun(request, page, String(testInfo.config.metadata.e2eImport));
  const fixture = await findCropRun(request, cropProjectId);
  expect(fixture).toBeTruthy();
  const cropNodeId = fixture!.cropNode.node_id;
  await page.goto(
    `/runs/${fixture!.run.id}?image=${fixture!.inspection.image_index}&node=${encodeURIComponent(cropNodeId)}`,
  );
  const overlay = page.locator('svg[aria-label="Annotation overlay"]');
  await expect(overlay.getByRole("button").first()).toBeVisible();
  await expect(overlay.locator("g.selected")).toHaveCount(1);
  await page.getByRole("button", { name: /Crop \(1\)/ }).click();
  await expect(page.locator(".crop-preview-list button.selected")).toHaveCount(1);
  await page.getByRole("button", { name: "Result", exact: true }).click();
  await expect(overlay.locator("g.selected")).toHaveCount(1);
  await page.screenshot({ path: `${screenshots}/03-run-artifact-lineage.png`, fullPage: true });
});

test("generic Project routes contain no RoboCup-specific copy", async ({ page }) => {
  await page.goto(`/projects/${projectId}/build/pipeline`);
  await expect(page.locator("body")).not.toContainText("RoboCup");
  await expect(page.getByText("Shared Stages", { exact: true })).toBeVisible();
});

test("mixed detector Results, Debug, and Review retain independent evidence", async ({ page, request }, testInfo) => {
  const mixedProjectName = `Mixed Detection ${stamp}`;
  const created = await request.post("/api/projects", { data: {
    id: mixedProjectId,
    yaml: `version: 1
project:
  name: ${mixedProjectName}
  language: en
dataset:
  root: images
runtime:
  max_parallel_images: 1
tasks:
  - id: ball-objects
    display_name: Ball
    kind: bounding_box
    labels: [ball]
    required: true
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.7
export:
  formats: [native, coco, yolo]
`,
  } });
  expect(created.status()).toBe(201);
  const imported = await request.post(`/api/projects/${mixedProjectId}/import`, {
    data: { source: String(testInfo.config.metadata.e2eImport) },
  });
  expect(imported.ok()).toBeTruthy();
  const state = await dashboard(request);
  const mixedImageId = randomUUID();
  const mixedRun = {
    id: mixedRunId,
    project_name: mixedProjectName,
    workflow_name: "Specialist with open-vocabulary fallback",
    workflow_version: "1",
    skill_versions: [],
    model_bindings: [],
    provider: "mock",
    model: "mixed detection",
    status: "completed_with_review",
    controllable: false,
    input_tokens: 0,
    output_tokens: 0,
    cost: "0",
    checkpoint_present: true,
    review_suspended: true,
    artifact_count: 2,
    validation_issue_codes: ["geometry_conflict"],
    retry_count: 0,
    fallback_nodes: ["open_vocabulary"],
    model_identity: "rfdetr-specialist-v1 + locate-anything-v1",
    timed_out: false,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  };
  const rfdetr = {
    source_model_id: "rfdetr-specialist-v1",
    source_artifact_id: "rfdetr-set",
    bbox: [0.1, 0.2, 0.2, 0.2],
    score: { value: 0.87, semantics: "relative_confidence" },
    model_label: "football",
    project_label: "ball",
    source_capability: "object_detection",
  };
  const locate = {
    source_model_id: "locate-anything-v1",
    source_artifact_id: "locate-set",
    bbox: [0.15, 0.2, 0.2, 0.2],
    score: { value: null, semantics: "not_provided" },
    query_id: "target-ball",
    project_label: "ball",
    source_capability: "open_vocabulary_detection",
  };
  const clusterArtifact = {
    kind: "candidate_cluster_set",
    artifact: {
      reference: { artifact_id: "clusters", source_node: "match", port: "candidates", artifact_type: "candidate_cluster_set" },
      image_id: mixedImageId,
      source_detection_sets: [],
      candidates: [
        {
          id: "agreement",
          target_label: "ball",
          representative_bbox: [0.1, 0.2, 0.2, 0.2],
          members: [rfdetr, locate],
          agreement: { multi_source_agreement: { minimum_iou: 0.78, mean_iou: 0.78 } },
        },
        {
          id: "conflict",
          target_label: "ball",
          representative_bbox: [0.52, 0.55, 0.12, 0.12],
          members: [
            { ...rfdetr, source_artifact_id: "rfdetr-conflict", bbox: [0.52, 0.55, 0.12, 0.12] },
            { ...locate, source_artifact_id: "locate-conflict", bbox: [0.72, 0.7, 0.1, 0.1] },
          ],
          agreement: "geometry_conflict",
        },
      ],
    },
  };
  const annotation = {
    id: mixedReviewId,
    image_id: mixedImageId,
    task_id: "ball-objects",
    label: "ball",
    value: { kind: "bounding_box", rect: [0.52, 0.55, 0.12, 0.12] },
    attributes: {},
    source: "model",
    review_status: "pending",
    provenance: {},
    created_at: new Date().toISOString(),
  };
  const progress = { reviewed_count: 0, total_count: 1, remaining_count: 1, current_position: 1 };
  const review = {
    id: mixedReviewId,
    run_id: mixedRunId,
    project_id: mixedProjectId,
    project_name: mixedProjectName,
    annotation,
    workflow_id: "mixed-detection",
    workflow_version: 1,
    image_index: 0,
    source_node: "match",
    source_artifact_id: "clusters",
    refinement_chain: [],
    review_reason: "validation_issue",
    validation_issues: ["geometry_conflict"],
    detection_evidence: [
      { ...rfdetr, source_artifact_id: "rfdetr-conflict", bbox: [0.52, 0.55, 0.12, 0.12] },
      { ...locate, source_artifact_id: "locate-conflict", bbox: [0.72, 0.7, 0.1, 0.1] },
    ],
    candidate_agreement: "geometry_conflict",
    review_explanation: {
      code: "geometry_conflict",
      title: "Needs review",
      summary: "RF-DETR and LocateAnything disagree on the object's location.",
      details: ["Bounding-box IoU: 0.12", "Choose one source box or merge the result manually."],
    },
  };

  await page.route("**/api/projects", (route) => route.fulfill({
    json: { ...state, runs: [mixedRun, ...state.runs], review_queue: 1 },
  }));
  await page.route(`**/api/runs/${mixedRunId}/pipeline-artifacts`, (route) => route.fulfill({ json: {
    run_id: mixedRunId,
    workflow_id: "mixed-detection",
    workflow_version: 1,
    content_hash: "fixture",
    project_id: mixedProjectId,
    image_index: 0,
    nodes: [{
      node_id: "match",
      operation: "core.match_detection_sets",
      status: "succeeded",
      configuration: { id: "match", node_type: "core.match_detection_sets", kind: "candidate_merge", inputs: [], outputs: [], depends_on: [], model_binding: null, parameters: {}, validators: [], refiners: [] },
      inputs: [],
      outputs: [clusterArtifact],
      latency_ms: 8,
      attempts: 1,
      cache_hit: false,
      usage: { input_tokens: 0, output_tokens: 0, cost: "0" },
      route: "review",
      metadata: { evidence_gate: { decision: "review", candidate_count: 2, validation_issue_count: 1, reasons: [{ code: "geometry_conflict", message: "Detector boxes disagree", source_model_ids: ["rfdetr-specialist-v1", "locate-anything-v1"], metrics: { iou: 0.12 } }] } },
    }],
  } }));
  await page.route(`**/api/runs/${mixedRunId}/result-summary`, (route) => route.fulfill({ json: {
    run_id: mixedRunId, project_id: mixedProjectId, status: "completed_with_review", image_count: 1,
    result_count: 2, ready_count: 1, needs_review_count: 1, no_target_count: 0, failed_count: 0,
    fallback_count: 1, cache_hit_count: 2, duration_ms: 42,
    usage: { input_tokens: 0, output_tokens: 0, estimated_cost: "0" }, image_index: 0,
    labels: [{ label: "ball", count: 2 }],
  } }));
  await page.route(`**/api/runs/${mixedRunId}/debug-summary`, (route) => route.fulfill({ json: {
    run_id: mixedRunId, workflow_id: "mixed-detection", workflow_version: 1, node_count: 1,
    succeeded_node_count: 1, failed_node_count: 0, current_node: "match", issues: [], duration_ms: 8,
    usage: { input_tokens: 0, output_tokens: 0, estimated_cost: "0" },
  } }));
  await page.route(`**/api/runs/${mixedRunId}/annotations`, (route) => route.fulfill({ json: {
    run_id: mixedRunId, project_id: mixedProjectId, image_index: 0, annotations: [annotation],
  } }));
  await page.route("**/api/reviews**", (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith(`/${mixedReviewId}`)) return route.fulfill({ json: review });
    if (path.endsWith(`/${mixedReviewId}/next`)) return route.fulfill({ json: { progress } });
    return route.fulfill({ json: { reviews: [review], progress } });
  });

  await page.setViewportSize({ width: 1024, height: 900 });
  await page.goto(`/runs/${mixedRunId}`);
  await expect(page.getByText("2 models agree · IoU 0.78", { exact: true }).first()).toBeVisible();
  await expect(page.getByText("2 models disagree on location", { exact: true }).first()).toBeVisible();
  await expect(page.getByLabel("Detection evidence inspector")).toContainText("RF-DETR");
  await expect(page.getByLabel("Detection evidence inspector")).toContainText("LocateAnything");
  await expect(page.getByLabel("Detection evidence inspector")).toContainText("No confidence");
  await expect(page.getByText("Fallbacks")).toBeVisible();
  await expect(page.getByText("Cache hits")).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();

  const debugUrl = `/runs/${mixedRunId}?view=debug&image=0&node=match`;
  await page.goto(debugUrl);
  await expect(page.getByLabel("Evidence decision")).toContainText("geometry conflict");
  await page.reload();
  await expect(page).toHaveURL(new RegExp(`view=debug&image=0&node=match$`));
  await expect(page.getByLabel("Evidence decision")).toContainText("Detector boxes disagree");

  await page.goto(`/review/${mixedReviewId}?project_id=${mixedProjectId}`);
  await expect(page.getByText("RF-DETR and LocateAnything disagree on the object's location.")).toBeVisible();
  await expect(page.getByLabel("Source model evidence")).toContainText("2 detector results");
  await page.getByRole("button", { name: "Use RF-DETR box" }).click();
  await expect(page.getByRole("button", { name: "Save changes" })).toBeVisible();
  await page.getByText("Execution details").click();
  await expect(page.getByLabel("Annotation attributes JSON")).toContainText("rfdetr-specialist-v1");
  await page.getByRole("button", { name: "Use LocateAnything box" }).click();
  await expect(page.getByLabel("Annotation attributes JSON")).toContainText("locate-anything-v1");
  await expect(page.getByLabel("Annotation attributes JSON")).toContainText("0.72");
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
});

test("Models exposes truthful unavailable and timeout Worker states", async ({ page }) => {
  const models = [
    {
      id: "locate-worker", provider: "http_vision", model: "locate-anything-v1", role: "open_vocabulary_detection",
      scope: "workspace_worker", health_status: "unavailable", health_detail: "weights unavailable",
      capabilities: ["open_vocabulary_detection"], score_semantics: "not_provided", model_version: "1",
      endpoint: "http://127.0.0.1:8791", enabled: true, license_summary: "non-commercial research/evaluation",
      architecture: "locateanything-3b", label_space: [], cost_per_request: "0",
      availability_group: "configured_unavailable",
    },
    {
      id: "rfdetr-worker", provider: "http_vision", model: "rfdetr-v1", role: "object_detection",
      scope: "workspace_worker", health_status: "unknown", capabilities: ["object_detection"],
      score_semantics: "relative_confidence", model_version: "1", endpoint: "http://127.0.0.1:8792",
      enabled: true, license_summary: "checkpoint terms configured", architecture: "rfdetr-small",
      checkpoint_sha256: "a".repeat(64), label_space: ["football"], cost_per_request: "0.001",
      availability_group: "labs",
    },
  ];
  await page.route("**/api/models", (route) => route.fulfill({ json: { models } }));
  await page.route("**/api/models/locate-worker/test", (route) => route.fulfill({ json: {
    model_id: "locate-anything-v1",
    health: { status: "unavailable", detail: "weights unavailable" },
    capabilities: { capabilities: ["open_vocabulary_detection"], score_semantics: "not_provided", supports_visual_prompt: false, supports_batch: false, label_space: [] },
  } }));
  await page.route("**/api/models/rfdetr-worker/test", (route) => route.fulfill({
    status: 504,
    contentType: "application/json",
    body: JSON.stringify({ error: "Detection Worker request timed out after 120 seconds" }),
  }));
  await page.goto("/models");
  await expect(page.getByText("Label space · football")).toBeVisible();
  await expect(page.getByText("Checkpoint · aaaaaaaaaaaa…")).toBeVisible();
  await page.locator("article", { hasText: "locate-worker" }).getByRole("button", { name: "Test connection" }).click();
  await expect(page.getByRole("status")).toContainText("Live · unavailable");
  await expect(page.getByRole("status")).toContainText("Confidence not provided");
  await page.locator("article", { hasText: "rfdetr-worker" }).getByRole("button", { name: "Test connection" }).click();
  await expect(page.getByRole("alert")).toContainText("request timed out");
});

test("Review behaves as a keyboard-operable decision inbox", async ({ page }) => {
  await page.goto(`/review/${reviewId}?project_id=${projectId}`);
  await page.evaluate(() => window.localStorage.removeItem("annotagent.reviewInspectorCollapsed"));
  await page.reload();
  await expect(page.getByLabel("Review progress")).toContainText("0 of 1 results reviewed");
  await expect(page.getByText("Why this needs review", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Accept and next" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Reject & next" })).toBeVisible();
  await expect(page.locator(".review-execution-details")).not.toHaveAttribute("open", "");

  await page.keyboard.press("E");
  await expect(page.getByLabel("Annotation edit details")).toBeVisible();
  const label = page.getByLabel("Label", { exact: true });
  await label.fill("day corrected");
  await expect(page.getByText("This correction will make similar candidates more likely to be reviewed.")).toBeVisible();
  await page.screenshot({ path: `${screenshots}/09-review-inbox.png`, fullPage: true });

  await page.reload();
  await expect(page.getByRole("button", { name: "Accept and next" })).toBeVisible();
  await page.locator("body").press("Space");
  await expect(page.getByLabel("Canvas view", { exact: true })).toHaveValue("before");
  await page.locator("body").press("R");
  await expect(page.getByRole("dialog", { name: "Why is this result incorrect?" })).toBeVisible();
  await expect(page.getByLabel("Reject reason").locator("option")).toHaveCount(5);
  await expect(page.locator('optgroup[label="Enabled Skill reasons"]')).toHaveCount(0);
  await page.screenshot({ path: `${screenshots}/10-review-reject.png`, fullPage: true });
  await page.getByRole("dialog", { name: "Why is this result incorrect?" }).getByRole("button", { name: "Reject & next" }).click();
  await expect(page.getByRole("heading", { name: "Review complete" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Continue to export" })).toBeVisible();
  await expect(page).toHaveURL(new RegExp(`/review\\?project_id=${projectId}$`));
  await page.reload();
  await expect(page.getByLabel("Review progress")).toContainText("1 of 1 results reviewed");
  await expect(page.getByRole("heading", { name: "Review complete" })).toBeVisible();
});

test("Export readiness blocks unresolved reviews and persists a completed export", async ({ page, request }) => {
  const exportReviewId = randomUUID();
  const createdReview = await request.post(`/api/runs/${runId}/annotations`, {
    data: {
      annotation: {
        id: exportReviewId,
        image_id: reviewImageId,
        task_id: "day-class",
        label: "day",
        value: { kind: "classification", labels: ["day"] },
        attributes: {},
        confidence: 0.98,
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

  await page.goto(`/projects/${projectId}/export`);
  await expect(page.getByRole("heading", { name: "Export needs attention" })).toBeVisible();
  await expect(page.getByText("1 annotation still requires a human decision.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Export AnnotAgent Native dataset" })).toBeDisabled();
  await page.getByRole("button", { name: "Resolve" }).click();
  await expect(page).toHaveURL(new RegExp(`/review/${exportReviewId}\\?project_id=${projectId}$`));
  await page.getByRole("button", { name: "Accept and next" }).click();
  await expect(page.getByRole("heading", { name: "Review complete" })).toBeVisible();
  await page.getByRole("button", { name: "Continue to export" }).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/export$`));
  await expect(page.getByRole("heading", { name: "Your dataset is ready" })).toBeVisible();
  await expect(page.getByText("Recommended", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Export AnnotAgent Native dataset" }).click();
  await expect(page.getByRole("heading", { name: "Dataset exported successfully" })).toBeVisible();
  await expect(page.getByText("Result folder", { exact: true })).toBeVisible();
  await expect(page.locator(".export-report")).not.toHaveAttribute("open", "");
  await page.screenshot({ path: `${screenshots}/11-export-complete.png`, fullPage: true });
  await page.reload();
  await expect(page.getByRole("heading", { name: "Dataset exported successfully" })).toBeVisible();

  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
});

test("Build and Runs state survives refresh plus browser history", async ({ page }) => {
  await page.goto(`/projects/${projectId}`);
  await page.getByRole("button", { name: "Build", exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/build/data$`));
  await page.getByLabel("Build steps").getByRole("button").filter({ hasText: "Labels" }).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/build/labels$`));
  await page.goBack();
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/build/data$`));
  await expect(page.getByRole("heading", { name: "Add images to your Project" })).toBeVisible();
  await page.goForward();
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/build/labels$`));
  await expect(page.getByRole("heading", { name: "What do you want to annotate?" })).toBeVisible();

  await page.getByLabel("Active project").selectOption(emptyProjectId);
  await expect(page).toHaveURL(new RegExp(`/projects/${emptyProjectId}/build/labels$`));
  await expect(page.getByLabel("Build step blocked")).toBeVisible();
  await page.getByLabel("Active project").selectOption(projectId);
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/build/labels$`));

  await page.goto(`/runs?project_id=${projectId}`);
  await page.getByLabel("Status filter").selectOption("completed");
  await expect(page).toHaveURL(new RegExp(`/runs\\?project_id=${projectId}&status=completed$`));
  await page.reload();
  await expect(page.getByLabel("Project filter")).toHaveValue(projectId);
  await expect(page.getByLabel("Status filter")).toHaveValue("completed");
});

test("SSE reconnect refreshes Export from server truth", async ({ page, request }) => {
  await page.route("**/api/events", (route) => route.abort("connectionfailed"));
  await page.goto(`/projects/${projectId}/export`);
  await expect(page.getByRole("heading", { name: "Dataset exported successfully" })).toBeVisible();
  await expect(page.locator(".sidebar-foot")).toContainText("SSE reconnecting");
  const reconnectReviewId = randomUUID();
  const createdReview = await request.post(`/api/runs/${runId}/annotations`, {
    data: {
      annotation: {
        id: reconnectReviewId,
        image_id: reviewImageId,
        task_id: "day-class",
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

  await page.unroute("**/api/events");
  await expect(page.locator(".sidebar-foot")).toContainText("SSE connected", { timeout: 15_000 });
  await expect(page.getByRole("heading", { name: "Export needs attention" })).toBeVisible();
  await expect(page.getByText("1 annotation still requires a human decision.")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Dataset exported successfully" })).toHaveCount(0);

  const accepted = await request.post(`/api/reviews/${reconnectReviewId}/accept-and-next`, {
    data: {
      project_id: projectId,
      queue_project_id: projectId,
      decision: "accept",
      reason_code: "accepted_as_is",
      note: "SSE recovery cleanup",
    },
  });
  expect(accepted.ok()).toBeTruthy();
});

test("release surfaces keep one primary action and remain operable at compact viewports", async ({ page }) => {
  const routes = [
    "/",
    "/projects",
    `/projects/${projectId}`,
    `/projects/${projectId}/build/data`,
    `/projects/${projectId}/build/labels`,
    `/projects/${projectId}/build/pipeline`,
    `/projects/${projectId}/build/test`,
    "/runs",
    `/runs/${runId}`,
    `/review?project_id=${projectId}`,
    `/projects/${projectId}/export`,
    "/settings",
  ];
  await page.setViewportSize({ width: 1024, height: 768 });
  for (const route of routes) {
    await page.goto(route);
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "false");
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth), route).toBeTruthy();
    expect(await page.locator("button.primary:visible").count(), `${route} primary actions`).toBeLessThanOrEqual(1);
    expect(await page.locator(".panel .panel").count(), `${route} nested panels`).toBe(0);
    for (const metrics of await page.locator(".metrics-grid, .run-result-metrics, .sample-outcome-metrics, .export-readiness-metrics").all()) {
      const widestRow = await metrics.evaluate((element) => {
        const rows = new Map<number, number>();
        for (const child of Array.from(element.children)) {
          const top = Math.round(child.getBoundingClientRect().top);
          rows.set(top, (rows.get(top) ?? 0) + 1);
        }
        return Math.max(0, ...rows.values());
      });
      expect(widestRow, `${route} equal metrics per row`).toBeLessThanOrEqual(3);
    }
  }
  await page.goto(`/projects/${projectId}`);
  await expect(page.locator(".project-context-facts > span")).toHaveCount(3);
  await page.screenshot({ path: `${screenshots}/12-guided-release-1024.png`, fullPage: true });

  await page.setViewportSize({ width: 720, height: 450 });
  for (const route of [
    `/projects/${projectId}`,
    `/projects/${projectId}/build/data`,
    `/projects/${projectId}/build/labels`,
    `/projects/${projectId}/build/pipeline`,
    `/projects/${projectId}/build/test`,
    `/runs/${runId}`,
    `/review?project_id=${projectId}`,
    `/projects/${projectId}/export`,
  ]) {
    await page.goto(route);
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "false");
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth), route).toBeTruthy();
    const lastControl = page.locator("#main-content button:enabled:visible").last();
    if (await lastControl.count()) {
      await lastControl.scrollIntoViewIfNeeded();
      await expect(lastControl).toBeVisible();
    }
  }
});

test("primary journey controls, focus, and annotation alternatives work from the keyboard", async ({ page, request }) => {
  await page.goto(`/projects/${projectId}`);
  const build = page.getByRole("button", { name: "Build", exact: true });
  await build.focus();
  expect(await build.evaluate((element) => {
    const style = getComputedStyle(element);
    return style.outlineStyle !== "none" && Number.parseFloat(style.outlineWidth) >= 2;
  })).toBeTruthy();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/build/data$`));
  const labelsStep = page.getByRole("button", { name: "Continue to Labels →" });
  await expect(labelsStep).toBeEnabled();
  await labelsStep.focus();
  await expect(labelsStep).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/build/labels$`));

  const fixture = await findCropRun(request, cropProjectId);
  expect(fixture?.run.id).toBe(cropRunId);
  await page.goto(`/runs/${cropRunId}?image=${fixture!.inspection.image_index}`);
  const annotationList = page.getByRole("list", { name: "Run result annotations" });
  await expect(annotationList).toBeVisible();
  const annotation = annotationList.getByRole("button").first();
  await annotation.focus();
  await page.keyboard.press("Enter");
  await expect(annotation).toHaveAttribute("aria-pressed", "true");
  const cropMode = page.getByRole("button", { name: /Crop \(1\)/ });
  await cropMode.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator(".crop-preview-list button.selected")).toHaveCount(1);

  await page.goto(`/projects/${projectId}/export`);
  const supportedFormat = page.locator('input[type="radio"]:enabled').first();
  await supportedFormat.focus();
  await page.keyboard.press("Space");
  await expect(supportedFormat).toBeChecked();
});

test("reduced motion and server-state error recovery are explicit", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto(`/projects/${projectId}`);
  const transitionMilliseconds = await page.locator(".guidance-progress i").evaluate((element) => {
    const values = getComputedStyle(element).transitionDuration.split(",");
    return Math.max(...values.map((value) => value.trim().endsWith("ms")
      ? Number.parseFloat(value)
      : Number.parseFloat(value) * 1_000));
  });
  expect(transitionMilliseconds).toBeLessThanOrEqual(0.01);

  let failReadiness = true;
  await page.route(`**/api/projects/${projectId}/export-readiness`, async (route) => {
    if (failReadiness)
      await route.fulfill({ status: 503, json: { error: "Export readiness is temporarily unavailable." } });
    else
      await route.continue();
  });
  await page.goto(`/projects/${projectId}/export`);
  const alert = page.getByRole("alert").filter({ hasText: "Export readiness is temporarily unavailable." });
  await expect(alert.getByText("AnnotAgent couldn’t complete that action.")).toBeVisible();
  await expect(alert).toContainText("Saved workspace data remains on the server");
  failReadiness = false;
  await alert.getByRole("button", { name: "Retry from latest state" }).click();
  await expect(page.getByRole("heading", { name: "Your dataset is ready" })).toBeVisible();

  await page.unroute(`**/api/projects/${projectId}/export-readiness`);
  let releaseProjects!: () => void;
  const release = new Promise<void>((resolve) => { releaseProjects = resolve; });
  await page.route("**/api/projects", async (route) => {
    await release;
    await route.continue();
  });
  await page.goto("/");
  await expect(page.getByRole("status").filter({ hasText: "Loading workspace state" })).toBeVisible();
  releaseProjects();
  await expect(page.getByRole("heading", { name: "Home" })).toBeVisible();
});
