import { isolatedEvidencePath } from "./evidence";
import { randomUUID } from "node:crypto";
import type { APIRequestContext, Page } from "@playwright/test";
import { expect as baseExpect, fetchWithinMutationLimit, test } from "./fixtures";

const expect = baseExpect.configure({ timeout: 75_000 });
type StopTarget = { kind: string; id: string; task_id: string };

async function projectConversation(request: APIRequestContext) {
  const project = `TEST-conversation-stop-${randomUUID()}`;
  const created = await request.post("/api/projects", { data: {
    id: project,
    yaml: "version: 1\nproject:\n  name: TEST conversation stop\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n",
  } });
  expect(created.ok(), await created.text()).toBe(true);
  const conversation = (await (await request.post(`/api/projects/${project}/conversations`)).json()).conversation_id as string;
  const root = `/api/projects/${project}/conversations/${conversation}`;
  return { project, conversation, root, url: `/projects/${project}/work?conversation=${conversation}` };
}

async function schemaFixture(request: APIRequestContext, count: number) {
  const state = await projectConversation(request);
  const provider = await (await request.post("/api/providers", { data: { display_name: "TEST chat-stop Schema transport", adapter: "open_ai_compatible", base_url: "http://127.0.0.1:8796/openai/v1" } })).json();
  expect((await request.post(`/api/providers/${provider.id}/credential`, { data: { source: "workspace_file", secret: "TEST-chat-stop-only" } })).ok()).toBe(true);
  const model = await (await request.post("/api/model-profiles", { data: {
    provider_id: provider.id, display_name: "TEST slow chat-stop Schema", remote_model_id: "e2e-conversation-classification-schema-background",
    input_modalities: ["text"], task_capabilities: ["text_generation"], protocol_features: { tool_calls: true, structured_output: true },
  } })).json();
  expect((await request.post(`/api/providers/${provider.id}/active-probe`, { data: { model_profile_id: model.id, confirmed_billable: true } })).ok()).toBe(true);
  const revision = (await (await request.get(`/api/projects/${state.project}/goal`)).json()).revision;
  const tasks = [];
  for (let index = 0; index < count; index++) {
    const message = randomUUID(), id = randomUUID();
    expect((await request.post(`${state.root}/messages`, { data: { id: message, text: `TEST ${index + 1}: 按室内、室外给图片分类`, image: null } })).ok()).toBe(true);
    expect((await request.post(`${state.root}/tasks`, { data: { id, source_message_id: message, schema_revision: revision } })).ok()).toBe(true);
    const taskRoot = `${state.root}/tasks/${id}`;
    const previewResponse = await request.get(`${taskRoot}/schema-preview?model_id=${model.id}`);
    expect(previewResponse.ok(), await previewResponse.text()).toBe(true);
    const preview = await previewResponse.json();
    const consent = { call_id: randomUUID(), model_id: model.id, scope_hash: preview.scope_hash, expires_at: preview.expires_at, allow_unknown_cost: true };
    tasks.push({ id, taskRoot, consent });
  }
  return { ...state, model, tasks };
}
type SchemaTask = Awaited<ReturnType<typeof schemaFixture>>["tasks"][number];

function startSchema(request: APIRequestContext, task: SchemaTask) {
  // Keep the real HTTP request in flight; do not fake an active status or hold
  // admission in Playwright. The isolated Provider responds after four seconds.
  return request.post(`${task.taskRoot}/schema-proposals`, { data: task.consent, timeout: 90_000 });
}
async function call(request: APIRequestContext, task: SchemaTask) {
  return (await (await request.get(`${task.taskRoot}/calls`)).json()).find((item: { id: string }) => item.id === task.consent.call_id);
}
async function stopMessage(page: Page, state: Awaited<ReturnType<typeof projectConversation>>) {
  const response = page.waitForResponse(response => response.request().method() === "POST" && new URL(response.url()).pathname === `${state.root}/stop-requests`);
  const composer = page.getByLabel("Your message", { exact: true });
  await composer.fill("停止");
  await composer.locator("xpath=ancestor::form").locator('button[type="submit"]').click();
  const saved = await response;
  expect(saved.ok(), await saved.text()).toBe(true);
  return { input: saved.request().postDataJSON(), record: await saved.json() };
}
function targetFor(record: { targets: StopTarget[] }, task: SchemaTask) {
  const target = record.targets.find(target => target.task_id === task.id && target.id === task.consent.call_id);
  expect(target, JSON.stringify(record)).toBeTruthy();
  return { kind: target!.kind, id: target!.id, task_id: target!.task_id };
}

test("standalone stop in an empty conversation does not create an annotation goal", async ({ page, request }) => {
  test.setTimeout(180_000);
  const { project, conversation, root } = await projectConversation(request);
  const goalBefore = await (await request.get(`/api/projects/${project}/goal`)).json();
  expect(await (await request.get(`${root}/tasks`)).json()).toEqual([]);
  const writes: string[] = [];
  page.on("request", request => {
    if (request.method() === "POST") writes.push(new URL(request.url()).pathname);
  });
  await page.goto(`/projects/${project}/work?conversation=${conversation}`);
  const composer = page.getByLabel("Your message", { exact: true });
  await composer.fill("停止");
  // The default composer submission must classify a standalone stop before its
  // empty-conversation "prepare goal" shortcut. No Provider is configured here.
  await composer.locator("xpath=ancestor::form").locator('button[type="submit"]').click();
  await expect(composer).toHaveValue("");
  expect(await (await request.get(`${root}/tasks`)).json()).toEqual([]);
  expect(await (await request.get(`/api/projects/${project}/goal`)).json()).toEqual(goalBefore);
  expect(writes.filter(path => path.endsWith("/tasks") || /schema-proposals|journey-consents|builder-operations|sample-operations/.test(path))).toEqual([]);
  const messages = await (await request.get(`${root}/messages`)).json();
  expect(messages).toHaveLength(1);
  expect(messages[0].input).toMatchObject({ text: "停止", image: null, reference: { scope: "stop_request", task_id: null } });
  await expect(page.getByRole("region", { name: "Stop request", exact: true })).toContainText("No active work to stop.");
  await expect(page.getByRole("button", { name: /^Use message \d+ as annotation goal$/ })).toHaveCount(0);
  await expect(page.getByRole("region", { name: "Annotation Schema proposal", exact: true })).toHaveCount(0);
  const writesBeforeReload = [...writes];
  await page.reload();
  await expect(page.getByRole("region", { name: "Stop request", exact: true })).toContainText("No active work to stop.");
  expect(writes).toEqual(writesBeforeReload);
  expect(await (await request.get(`${root}/tasks`)).json()).toEqual([]);
});

test("Composer stop stays independent of unsent text and waits for server acknowledgement", async ({ page, request }) => {
  test.setTimeout(120_000);
  const state = await schemaFixture(request, 1), task = state.tasks[0];
  await page.goto(`${state.url}&task=${task.id}`);
  const input = page.getByLabel("Your message", { exact: true });
  await input.fill("TEST keep this next instruction while stopping");
  await expect(page.getByRole("button", { name: "Stop task", exact: true })).toBeEnabled();
  const execution = startSchema(request, task);
  await expect.poll(async () => (await call(request, task))?.status).toBe("reserved");
  let release!: () => void;
  const held = new Promise<void>(resolve => { release = resolve; });
  let stopWrites = 0;
  await page.route(`**${state.root}/stop-requests`, async route => {
    stopWrites++;
    // Perform the real cancellation, then delay only its network acknowledgement.
    const response = await fetchWithinMutationLimit(route);
    await held;
    await route.fulfill({ response });
  });
  await page.getByRole("button", { name: "Stop task", exact: true }).click();
  await expect(page.getByRole("button", { name: "Stopping…", exact: true })).toBeDisabled();
  await expect(input).toHaveValue("TEST keep this next instruction while stopping");
  await expect(input).toBeEnabled();
  try { await expect.poll(()=>stopWrites).toBe(1); }
  finally { release(); }
  await expect(page.getByRole("region", { name: "Stop request", exact: true })).toContainText(/unknown/i);
  await execution;
  expect((await call(request, task)).status).toBe("in_doubt");
  await expect(input).toHaveValue("TEST keep this next instruction while stopping");
  await input.fill("");
  await page.reload();
  await expect(page.getByRole("region", { name: "Stop request", exact: true })).toContainText(/unknown/i);
  expect(stopWrites).toBe(1);
  expect((await (await request.get(`${task.taskRoot}/budget`)).json()).total_reserved_calls).toBe(1);
});

test("chat stop cancels the actual reserved Schema call once and restores without another POST", async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await schemaFixture(request, 1), task = state.tasks[0];
  await page.goto(`${state.url}&task=${task.id}`);
  const writes: string[] = [];
  page.on("request", request => { if (request.method() === "POST") writes.push(new URL(request.url()).pathname); });
  const execution = startSchema(request, task);
  await expect.poll(async () => (await call(request, task))?.status).toBe("reserved");
  const budgetBefore = await (await request.get(`${task.taskRoot}/budget`)).json();
  const saved = await stopMessage(page, state);
  expect(saved.input.reference).toEqual({ scope: "stop_request", task_id: task.id });
  expect(saved.record.status).toBe("cancel_requested");
  expect(saved.record.targets).toHaveLength(1);
  expect(saved.record.selected_target).toEqual(targetFor(saved.record, task));
  await execution;
  await expect.poll(async () => (await call(request, task))?.status).toBe("in_doubt");
  const receipt = await call(request, task);
  expect(receipt.evidence.error).toContain("Remote completion and cost are unknown");
  expect(await (await request.get(`${task.taskRoot}/calls/${task.consent.call_id}/schema-draft`)).json()).toBeNull();
  expect(await (await request.get(`${task.taskRoot}/budget`)).json()).toEqual(budgetBefore);
  expect((await (await request.get(`${state.root}/tasks`)).json())).toHaveLength(1);
  const card = page.getByRole("region", { name: "Stop request", exact: true });
  await expect(card).toContainText("Cancellation request saved.");
  await card.screenshot({ path: isolatedEvidencePath("../docs/execution/conversational-workspace/chat-stop-schema-receipt.png"), animations: "disabled" });
  const path = `${state.root}/stop-requests/${saved.input.id}`;
  const stored = await (await request.get(path)).json();
  const replay = await request.post(`${state.root}/stop-requests`, { data: saved.input });
  expect(replay.ok(), await replay.text()).toBe(true);
  expect((await replay.json()).selected_target).toEqual(stored.selected_target);
  const beforeReload = [...writes];
  await page.reload();
  await expect(card).toContainText("Cancellation request saved.");
  await page.goto("/projects");
  await page.goBack();
  await expect(card).toContainText("Cancellation request saved.");
  expect(writes).toEqual(beforeReload);
  expect(await call(request, task)).toEqual(receipt);
  expect((await (await request.get(`${task.taskRoot}/calls`)).json())).toHaveLength(1);
});

test("chat stop with no selected task asks which active operation and cancels only that frozen target", async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await schemaFixture(request, 2), [first, second] = state.tasks;
  expect((await (await request.get(`${state.root}/task-selection`)).json()).task_id).toBeNull();
  await page.goto(state.url);
  const executions = state.tasks.map(task => startSchema(request, task));
  await expect.poll(async () => Promise.all(state.tasks.map(async task => (await call(request, task))?.status))).toEqual(["reserved", "reserved"]);
  const saved = await stopMessage(page, state);
  expect(saved.input.reference).toEqual({ scope: "stop_request", task_id: null });
  expect(saved.record.status).toBe("needs_selection");
  expect(saved.record.selected_target).toBeNull();
  expect(saved.record.targets).toHaveLength(2);
  const firstTarget = targetFor(saved.record, first), secondTarget = targetFor(saved.record, second);
  for (const task of state.tasks) expect(await (await request.get(`${task.taskRoot}/cancellations`)).json()).toEqual([]);
  const card = page.getByRole("region", { name: "Stop request", exact: true });
  await expect(card.getByRole("group", { name: "Which operation should stop?", exact: true })).toBeVisible();
  await expect(card.getByRole("button", { name: "Stop selected operation", exact: true })).toBeDisabled();
  // The target label contains its stable operation ID so two identical model
  // names never force the user/test to guess by newest position.
  await card.getByRole("radio", { name: new RegExp(firstTarget.id) }).check();
  await card.getByRole("button", { name: "Stop selected operation", exact: true }).click();
  await Promise.all(executions);
  await expect.poll(async () => (await call(request, first))?.status).toBe("in_doubt");
  await expect.poll(async () => (await call(request, second))?.status).toBe("completed");
  expect(await (await request.get(`${second.taskRoot}/cancellations`)).json()).toEqual([]);
  const path = `${state.root}/stop-requests/${saved.input.id}`;
  expect((await (await request.get(path)).json()).selected_target).toEqual(firstTarget);
  const changed = await request.post(`${path}/select`, { data: { target: secondTarget } });
  expect(changed.status()).toBe(400);
  expect((await (await request.get(path)).json()).selected_target).toEqual(firstTarget);
  const writes: string[] = [];
  page.on("request", request => { if (request.method() === "POST") writes.push(request.url()); });
  await page.reload();
  await expect(card).toContainText("Cancellation request saved.");
  expect(new URL(page.url()).searchParams.get("task")).toBeNull();
  expect(writes).toEqual([]);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: "Conversation", exact: true }).click();
  await card.screenshot({ path: isolatedEvidencePath("../docs/execution/conversational-workspace/chat-stop-selected-390.png"), animations: "disabled" });
});

test("a saved stop snapshot never substitutes a new active call after its original target finishes", async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await schemaFixture(request, 3), [first, other, later] = state.tasks;
  // Bootstrap this distinct browser request context, deliberately omitting its
  // CSRF header below. Otherwise the test stops at missing-session 401 instead.
  expect((await page.request.get("/api/session")).ok()).toBe(true);
  const executions = [first, other].map(task => startSchema(request, task));
  await expect.poll(async () => Promise.all([first, other].map(async task => (await call(request, task))?.status))).toEqual(["reserved", "reserved"]);
  const input = { id: randomUUID(), text: "停止", image: null, reference: { scope: "stop_request", task_id: null } };
  const response = await request.post(`${state.root}/stop-requests`, { data: input });
  expect(response.ok(), await response.text()).toBe(true);
  const frozen = await response.json();
  expect(frozen.status).toBe("needs_selection");
  expect(frozen.targets).toHaveLength(2);
  const target = targetFor(frozen, first), path = `${state.root}/stop-requests/${input.id}`;
  await Promise.all(executions);
  const completed = await Promise.all([first, other].map(task => call(request, task)));
  expect(completed.map(receipt => receipt.status)).toEqual(["completed", "completed"]);
  const schemas = await Promise.all([first, other].map(async task => (await (await request.get(`${task.taskRoot}/calls/${task.consent.call_id}/schema-draft`)).json())));
  const laterExecution = startSchema(request, later);
  await expect.poll(async () => (await call(request, later))?.status).toBe("reserved");
  const frozenReload = await (await request.get(path)).json();
  expect(frozenReload.targets).toEqual(frozen.targets);
  const replay = await request.post(`${state.root}/stop-requests`, { data: input });
  expect((await replay.json()).targets).toEqual(frozen.targets);
  const budgetBefore = await (await request.get(`${later.taskRoot}/budget`)).json();
  // No CSRF and foreign/hidden selectors must not turn this immutable request
  // into "stop whichever thing is running now".
  expect((await page.request.post(`${path}/select`, { data: { target } })).status()).toBe(403);
  expect((await request.post(`${path}/select`, { data: { target: { kind: "call", id: later.consent.call_id, task_id: later.id } } })).status()).toBe(400);
  expect((await request.post(`${path}/select`, { data: { target, stop_all: true } })).status()).toBe(422);
  const selected = await request.post(`${path}/select`, { data: { target } });
  expect(selected.ok(), await selected.text()).toBe(true);
  expect((await selected.json()).status).toBe("finished");
  expect((await (await request.get(path)).json()).selected_target).toEqual(target);
  expect(await (await request.get(`${later.taskRoot}/cancellations`)).json()).toEqual([]);
  await laterExecution;
  expect((await call(request, later)).status).toBe("completed");
  expect(await (await request.get(`${later.taskRoot}/budget`)).json()).toEqual(budgetBefore);
  for (const [index, task] of [first, other].entries()) {
    expect(await call(request, task)).toEqual(completed[index]);
    expect(await (await request.get(`${task.taskRoot}/calls/${task.consent.call_id}/schema-draft`)).json()).toEqual(schemas[index]);
    expect(await (await request.get(`${task.taskRoot}/cancellations`)).json()).toEqual([]);
  }
  const writes: string[] = [];
  page.on("request", request => { if (request.method() === "POST") writes.push(request.url()); });
  await page.goto(state.url);
  const card = page.getByRole("region", { name: "Stop request", exact: true });
  await expect(card).toBeVisible();
  await page.reload();
  await expect(card).toBeVisible();
  expect(writes).toEqual([]);
  expect((await (await request.get(path)).json()).targets).toEqual(frozen.targets);
});

test("chat stop revokes a saved but unadmitted Schema authorization and cannot resume it after the call limit is raised", async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await schemaFixture(request, 1), task = state.tasks[0];
  const limitPath = `/api/projects/${state.project}/conversation-call-limit`;
  expect((await request.post(limitPath, { data: { id: randomUUID(), expected_revision: 0, maximum_calls: 0 } })).ok()).toBe(true);
  const rejected = await request.post(`${task.taskRoot}/schema-proposals`, { data: task.consent });
  expect(rejected.status()).toBe(400);
  expect(await rejected.text()).toContain("Project conversation call limit exhausted");
  expect(await (await request.get(`${task.taskRoot}/schema-authorizations/pending`)).json()).toEqual(task.consent);
  expect(await (await request.get(`${task.taskRoot}/calls`)).json()).toEqual([]);
  await page.goto(`${state.url}&task=${task.id}`);
  const saved = await stopMessage(page, state);
  expect(saved.record.status).toBe("cancel_requested");
  expect(saved.record.targets).toHaveLength(1);
  // A typed, unadmitted Schema authorization retains its exact call identity;
  // generic grants use the separate "authorization" target kind.
  expect(saved.record.selected_target).toEqual({ kind: "call", id: task.consent.call_id, task_id: task.id });
  expect(saved.record.targets[0].state).toBe("authorized_not_started");
  expect((await (await request.get(`${task.taskRoot}/cancellations`)).json()).some((item: { call_id: string }) => item.call_id === task.consent.call_id)).toBe(true);
  const limit = await (await request.get(limitPath)).json();
  expect((await request.post(limitPath, { data: { id: randomUUID(), expected_revision: limit.revision, maximum_calls: 1 } })).ok()).toBe(true);
  const budget = await (await request.get(`${task.taskRoot}/budget`)).json();
  const resume = await request.post(`${task.taskRoot}/schema-proposals`, { data: task.consent });
  expect(resume.status()).toBe(400);
  expect(await resume.text()).toMatch(/cancel|stopped/i);
  expect(await (await request.get(`${task.taskRoot}/calls`)).json()).toEqual([]);
  expect(await (await request.get(`${task.taskRoot}/budget`)).json()).toEqual(budget);
  const writes: string[] = [];
  page.on("request", request => { if (request.method() === "POST") writes.push(request.url()); });
  await page.reload();
  await expect(page.getByRole("region", { name: "Stop request", exact: true })).toContainText("Cancellation request saved.");
  expect(writes).toEqual([]);
});

test("a lost stop acknowledgement restores its original empty snapshot without stopping a subsequently started call", async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await schemaFixture(request, 1), task = state.tasks[0];
  await page.goto(state.url);
  let savedInput: any, savedRecord: any, blockRecovery = true;
  const posts: unknown[] = [];
  page.on("request", request => {
    if (request.method() === "POST" && new URL(request.url()).pathname === `${state.root}/stop-requests`) posts.push(request.postDataJSON());
  });
  await page.route(`**${state.root}/stop-requests/*`, async route => {
    if (route.request().method() === "GET" && blockRecovery) return route.abort("failed");
    return route.fallback();
  });
  await page.route(`**${state.root}/stop-requests`, async route => {
    if (route.request().method() !== "POST") return route.fallback();
    savedInput = route.request().postDataJSON();
    const response = await fetchWithinMutationLimit(route);
    expect(response.ok(), await response.text()).toBe(true);
    savedRecord = await response.json();
    await route.abort("failed"); // Actual server save succeeded; its ACK was lost.
  }, { times: 1 });
  const composer = page.getByLabel("Your message", { exact: true });
  await composer.fill("停止");
  await composer.locator("xpath=ancestor::form").locator('button[type="submit"]').click();
  await expect(page.getByRole("status").filter({ hasText: /^The stop request acknowledgement is unknown\./ })).toBeVisible();
  expect(savedRecord.status).toBe("no_active_work");
  expect(savedRecord.targets).toEqual([]);
  expect(savedInput.reference.task_id).toBeNull();
  expect(posts).toHaveLength(1);
  const execution = startSchema(request, task);
  await expect.poll(async () => (await call(request, task))?.status).toBe("reserved");
  blockRecovery = false;
  await page.reload();
  await expect(page.getByRole("region", { name: "Stop request", exact: true })).toContainText("No active work to stop.");
  expect(posts).toHaveLength(1);
  const restored = await (await request.get(`${state.root}/stop-requests/${savedInput.id}`)).json();
  expect(restored.message).toEqual(savedRecord.message);
  expect(restored.targets).toEqual([]);
  expect(restored.selected_target).toBeNull();
  expect(await (await request.get(`${task.taskRoot}/cancellations`)).json()).toEqual([]);
  await execution;
  expect((await call(request, task)).status).toBe("completed");
  expect(posts).toHaveLength(1);
  expect((await (await request.get(`${state.root}/messages`)).json()).filter((item: any) => item.input.id === savedInput.id)).toHaveLength(1);
});

test("composition state suppresses stop submission and a normal stop-sign request remains an annotation goal", async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await projectConversation(request);
  await page.goto(state.url);
  const writes: string[] = [];
  page.on("request", request => { if (request.method() === "POST") writes.push(new URL(request.url()).pathname); });
  const composer = page.getByLabel("Your message", { exact: true });
  await composer.fill("停止");
  // Browser CompositionEvents exercise the implemented IME guard. This is not
  // a claim that Playwright drove a real OS Chinese input-method installation.
  await composer.dispatchEvent("compositionstart", { data: "停" });
  await composer.locator("xpath=ancestor::form").locator('button[type="submit"]').click();
  expect(await (await request.get(`${state.root}/messages`)).json()).toEqual([]);
  expect(await (await request.get(`${state.root}/tasks`)).json()).toEqual([]);
  expect(writes).toEqual([]);
  await expect(composer).toHaveValue("停止");
  await composer.dispatchEvent("compositionend", { data: "停止" });
  await composer.fill("请标注 stop sign");
  await composer.locator("xpath=ancestor::form").locator('button[type="submit"]').click();
  await expect(composer).toHaveValue("");
  const messages = await (await request.get(`${state.root}/messages`)).json();
  expect(messages).toHaveLength(1);
  expect(messages[0].input.text).toBe("请标注 stop sign");
  expect(messages[0].input.reference ?? null).toBeNull();
  const tasks = await (await request.get(`${state.root}/tasks`)).json();
  expect(tasks).toHaveLength(1);
  expect(tasks[0].input.source_message_id).toBe(messages[0].input.id);
  expect(await (await request.get(`${state.root}/tasks/${tasks[0].input.id}/calls`)).json()).toEqual([]);
  expect(writes.filter(path => path.includes("/stop-requests"))).toEqual([]);
  await expect(page.getByRole("region", { name: "Stop request", exact: true })).toHaveCount(0);
});

test("stop messages reject foreign task scope, hidden controls and non-command text without writing a journal entry", async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await projectConversation(request), foreign = await projectConversation(request);
  const foreignMessage = randomUUID(), foreignTask = randomUUID();
  const revision = (await (await request.get(`/api/projects/${foreign.project}/goal`)).json()).revision;
  expect((await request.post(`${foreign.root}/messages`, { data: { id: foreignMessage, text: "TEST foreign goal", image: null } })).ok()).toBe(true);
  expect((await request.post(`${foreign.root}/tasks`, { data: { id: foreignTask, source_message_id: foreignMessage, schema_revision: revision } })).ok()).toBe(true);
  const input = { id: randomUUID(), text: "停止", image: null, reference: { scope: "stop_request", task_id: null } };
  expect((await page.request.get("/api/session")).ok()).toBe(true);
  expect((await page.request.post(`${state.root}/stop-requests`, { data: input })).status()).toBe(403);
  expect((await request.post(`${state.root}/stop-requests`, { data: { ...input, reference: { ...input.reference, task_id: foreignTask } } })).status()).toBe(400);
  expect((await request.post(`${state.root}/stop-requests`, { data: { ...input, stop_all: true } })).status()).toBe(422);
  expect((await request.post(`${state.root}/stop-requests`, { data: { ...input, text: "请标注 stop sign" } })).status()).toBe(400);
  expect(await (await request.get(`${state.root}/messages`)).json()).toEqual([]);
  expect(await (await request.get(`${state.root}/tasks`)).json()).toEqual([]);
  expect((await (await request.get(`${foreign.root}/messages`)).json())).toHaveLength(1);
  expect(await (await request.get(`${foreign.root}/tasks/${foreignTask}/cancellations`)).json()).toEqual([]);
  const saved = await request.post(`${state.root}/stop-requests`, { data: input });
  expect(saved.ok(), await saved.text()).toBe(true);
  const foreignRead = await request.get(`${foreign.root}/stop-requests/${input.id}`);
  expect(foreignRead.status()).toBe(400);
  const differentText = await request.post(`${state.root}/stop-requests`, { data: { ...input, text: "stop" } });
  expect(differentText.status()).toBe(400);
  expect((await (await request.get(`${state.root}/stop-requests/${input.id}`)).json()).message.input).toEqual(input);
});

test("a lost stop selection acknowledgement restores the same target and never repeats selection on refresh", async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await schemaFixture(request, 2), [first, second] = state.tasks;
  await page.goto(state.url);
  const executions = state.tasks.map(task => startSchema(request, task));
  await expect.poll(async () => Promise.all(state.tasks.map(async task => (await call(request, task))?.status))).toEqual(["reserved", "reserved"]);
  const saved = await stopMessage(page, state), target = targetFor(saved.record, first);
  expect(saved.record.status).toBe("needs_selection");
  const card = page.getByRole("region", { name: "Stop request", exact: true });
  await card.getByRole("radio", { name: new RegExp(first.consent.call_id) }).check();
  const path = `${state.root}/stop-requests/${saved.input.id}`;
  let blockRecovery = true;
  const selections: unknown[] = [];
  page.on("request", request => {
    if (request.method() === "POST" && new URL(request.url()).pathname === `${path}/select`) selections.push(request.postDataJSON());
  });
  await page.route(`**${path}`, async route => {
    if (route.request().method() === "GET" && blockRecovery) return route.abort("failed");
    return route.fallback();
  });
  await page.route(`**${path}/select`, async route => {
    const response = await fetchWithinMutationLimit(route);
    expect(response.ok(), await response.text()).toBe(true);
    expect((await response.json()).selected_target).toEqual(target);
    await route.abort("failed");
  }, { times: 1 });
  await card.getByRole("button", { name: "Stop selected operation", exact: true }).click();
  await expect(card.getByRole("button", { name: "Retry same stop selection", exact: true })).toBeEnabled();
  expect(selections).toEqual([{ target }]);
  await Promise.all(executions);
  expect((await call(request, first)).status).toBe("in_doubt");
  expect((await call(request, second)).status).toBe("completed");
  expect(await (await request.get(`${second.taskRoot}/cancellations`)).json()).toEqual([]);
  blockRecovery = false;
  await page.reload();
  await expect(card).toContainText("Cancellation request saved.");
  await expect(card.getByRole("radio")).toHaveCount(0);
  await expect(card.getByRole("button", { name: "Retry same stop selection", exact: true })).toHaveCount(0);
  expect((await (await request.get(path)).json()).selected_target).toEqual(target);
  expect(selections).toEqual([{ target }]);
  expect((await (await request.get(`${state.root}/messages`)).json()).filter((item: any) => item.input.id === saved.input.id)).toHaveLength(1);
});

test("an explicitly selected task stop leaves another actual running task untouched", async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await schemaFixture(request, 2), [first, second] = state.tasks;
  await page.goto(`${state.url}&task=${first.id}`);
  const executions = state.tasks.map(task => startSchema(request, task));
  await expect.poll(async () => Promise.all(state.tasks.map(async task => (await call(request, task))?.status))).toEqual(["reserved", "reserved"]);
  const saved = await stopMessage(page, state);
  expect(saved.input.reference.task_id).toBe(first.id);
  expect(saved.record.status).toBe("cancel_requested");
  expect(saved.record.targets).toHaveLength(1);
  expect(saved.record.selected_target).toEqual(targetFor(saved.record, first));
  await Promise.all(executions);
  expect((await call(request, first)).status).toBe("in_doubt");
  expect((await call(request, second)).status).toBe("completed");
  expect(await (await request.get(`${second.taskRoot}/cancellations`)).json()).toEqual([]);
  const secondSchema = await (await request.get(`${second.taskRoot}/calls/${second.consent.call_id}/schema-draft`)).json();
  expect(secondSchema.source_call_id).toBe(second.consent.call_id);
  expect(secondSchema.revision).toBe(1);
  const writes: string[] = [];
  page.on("request", request => { if (request.method() === "POST") writes.push(request.url()); });
  await page.reload();
  await expect(page.getByRole("region", { name: "Stop request", exact: true })).toContainText("Cancellation request saved.");
  expect(new URL(page.url()).searchParams.get("task")).toBe(first.id);
  expect(writes).toEqual([]);
  expect(await (await request.get(`${second.taskRoot}/calls/${second.consent.call_id}/schema-draft`)).json()).toEqual(secondSchema);
});
