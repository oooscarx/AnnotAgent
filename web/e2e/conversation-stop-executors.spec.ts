import { isolatedEvidencePath } from "./evidence";
import { randomUUID } from "node:crypto";
import { resolve } from "node:path";
import type { APIRequestContext, Page } from "@playwright/test";
import { expect as baseExpect, test } from "./fixtures";

const expect = baseExpect.configure({ timeout: 75_000 });

// These are real service/cancellation integration tests against the isolated
// TEST workspace and deterministic HTTP transport, never the live 8787 workspace.
// In-flight work comes from the fixture's existing delays, not invented API states.
async function read(request: APIRequestContext, path: string) {
  const response = await request.get(path);
  expect(response.ok(), await response.text()).toBe(true);
  return response.json();
}
async function post(request: APIRequestContext, path: string, data: unknown) {
  const response = await request.post(path, { data, timeout: 90_000 });
  expect(response.ok(), await response.text()).toBe(true);
  return response.json();
}
async function fixture(request: APIRequestContext, page: Page, remote: string) {
  const provider = await post(request, "/api/providers", { display_name: "TEST chat Stop executors", adapter: "open_ai_compatible", base_url: "http://127.0.0.1:8796/openai/v1" });
  await post(request, `/api/providers/${provider.id}/credential`, { source: "workspace_file", secret: "TEST-stop-executors-only" });
  const model = await post(request, "/api/model-profiles", { provider_id: provider.id, display_name: "TEST delayed executor", remote_model_id: remote, input_modalities: ["text", "image"], task_capabilities: ["text_generation", "vision_language", "image_classification"], protocol_features: { tool_calls: true, structured_output: true } });
  await post(request, `/api/providers/${provider.id}/active-probe`, { model_profile_id: model.id, confirmed_billable: true });
  const project = `TEST-chat-stop-executor-${randomUUID()}`;
  await post(request, "/api/projects", { id: project, yaml: "version: 1\nproject:\n  name: TEST chat Stop executors\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n" });
  expect((await request.put(`/api/projects/${project}/model-bindings`, { data: { bindings: [{ capability: "image_classification", role: "classification", match_kind: "capability", model_profile_id: model.id, locked: false }] } })).ok()).toBe(true);
  await page.goto(`/projects/${project}/work`);
  await page.getByLabel("Add images", { exact: true }).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await expect(page.getByText("Images saved on this server. This upload did not start inference.", { exact: true })).toBeVisible();
  const conversation = (await post(request, `/api/projects/${project}/conversations`, {})).conversation_id;
  const root = `/api/projects/${project}/conversations/${conversation}`, task = randomUUID(), source = randomUUID();
  const revision = (await read(request, `/api/projects/${project}/goal`)).revision;
  await post(request, `${root}/messages`, { id: source, text: "TEST: Classify this image as day.", image: null });
  await post(request, `${root}/tasks`, { id: task, source_message_id: source, schema_revision: revision });
  const taskRoot = `${root}/tasks/${task}`;
  const schema = await post(request, `${taskRoot}/human-schema-drafts`, { request_id: randomUUID(), decision: { decision: "draft", kind: "classification", labels: ["day"], multi_label: false, attributes: {}, boundary_rules: ["TEST classification only."], rationale: "Human-defined TEST Schema; no Schema inference." } });
  return { project, conversation, root, taskRoot, task, model, schema };
}
type State = Awaited<ReturnType<typeof fixture>>;

async function build(request: APIRequestContext, state: State) {
  const query = new URLSearchParams({ operation_id: randomUUID(), schema_id: state.schema.id, schema_revision: String(state.schema.revision), model_id: state.model.id });
  const preview = await read(request, `${state.taskRoot}/builder-preview?${query}`);
  const operation = await post(request, `${state.taskRoot}/builder-operations`, { selection: preview.selection, repair: preview.repair, previous_grant_id: preview.previous_grant_id, scope_hash: preview.scope_hash, expires_at: preview.expires_at, allow_unknown_cost: true });
  expect(operation.status, JSON.stringify(operation)).toBe("completed");
  expect(operation.evidence.draft_id).toBeTruthy();
  const id=operation.evidence.draft_id as string;
  const draft=(await read(request,`/api/workflow-drafts?project_id=${state.project}`)).drafts.find((entry:any)=>entry.id===id);
  expect(draft).toBeTruthy();
  // These tests exercise cancellation, not the planner's model preference.
  // Explicitly bind its editable Draft to this test's delayed transport so
  // another model accumulated by the suite cannot make the sample finish early.
  const modelNodes=draft.nodes.filter((node:any)=>node.model_profile_binding||node.model_binding);
  expect(modelNodes).toHaveLength(1);
  modelNodes[0].model_profile_binding={model_profile_id:state.model.id,locked:true};
  const updated=await request.patch(`/api/workflow-drafts/${id}`,{headers:{"if-match":String(draft.revision)},data:draft});
  expect(updated.ok(),await updated.text()).toBe(true);
  return id;
}
async function startSample(request: APIRequestContext, state: State, draft: string) {
  const preview = await read(request, `${state.taskRoot}/sample-preview?${new URLSearchParams({ draft_id: draft, request_id: randomUUID() })}`);
  expect(preview.supported).toBe(true);
  expect(preview.image_count).toBe(1);
  expect(preview.models.map((model:any)=>model.id)).toEqual([state.model.id]);
  const envelope = { request_id: preview.request_id, draft_id: draft, expected_revision: preview.revision, image_indices: [0], authorization_fingerprint: preview.authorization_fingerprint, conversation: { conversation_id: state.conversation, task_id: state.task, previous_grant_id: preview.conversation_budget.previous_grant_id, scope_hash: preview.conversation_budget.scope_hash, expires_at: preview.conversation_budget.expires_at, allow_unknown_cost: true, human_review: true } };
  const operation = await post(request, `/api/projects/${state.project}/sample-operations`, envelope);
  return { operation, envelope, path: `/api/projects/${state.project}/sample-operations/${operation.id}` };
}
async function stop(request: APIRequestContext, state: State, kind: string, targetId: string) {
  const message = { id: randomUUID(), text: "停止", image: null, reference: { scope: "stop_request", task_id: state.task } };
  const record = await post(request, `${state.root}/stop-requests`, message);
  expect(record.status, JSON.stringify(record)).toBe("cancel_requested");
  expect(record.dispatch_error).toBeNull();
  // Child calls/grants are not shown as competing operations of their Journey.
  expect(record.targets).toHaveLength(1);
  expect(record.selected_target).toEqual({ kind, id: targetId, task_id: state.task });
  return { message, record, path: `${state.root}/stop-requests/${message.id}` };
}
async function replayStop(request: APIRequestContext, state: State, stopped: Awaited<ReturnType<typeof stop>>) {
  const calls = await read(request, `${state.taskRoot}/calls`);
  const budget = await read(request, `${state.taskRoot}/budget`);
  const replay = await post(request, `${state.root}/stop-requests`, stopped.message);
  expect(replay.selected_target).toEqual(stopped.record.selected_target);
  expect(replay.targets).toEqual(stopped.record.targets);
  expect((await read(request, stopped.path)).selected_target).toEqual(stopped.record.selected_target);
  expect(await read(request, `${state.taskRoot}/calls`)).toEqual(calls);
  expect(await read(request, `${state.taskRoot}/budget`)).toEqual(budget);
  expect((await read(request, `${state.root}/messages`)).filter((message: any) => message.input.reference?.scope === "stop_request")).toHaveLength(1);
}

test("chat Stop cancels the real Journey Builder and never starts its authorized sample", async ({ request, page }) => {
  test.setTimeout(240_000);
  const state = await fixture(request, page, "e2e-conversation-classification-background");
  const query = new URLSearchParams({ consent_id: randomUUID(), builder_operation_id: randomUUID(), sample_operation_id: randomUUID(), schema_id: state.schema.id, schema_revision: String(state.schema.revision), planner_model_id: state.model.id, allowed_models: JSON.stringify([`model-profile:${state.model.id}`]) });
  const preview = await read(request, `${state.taskRoot}/journey-preview?${query}`);
  const consent = { ...preview.consent, allow_unknown_cost: true };
  await post(request, `${state.taskRoot}/journey-consents`, consent);
  const path = `${state.taskRoot}/journey-consents/${consent.id}/execution`;
  expect((await post(request, path, {})).dispatch.status).toBe("running");
  await expect.poll(async () => (await read(request, path)).builder?.status).toBe("reserved");
  await expect.poll(async () => (await read(request, `${state.taskRoot}/calls`)).some((call: any) => call.status === "reserved")).toBe(true);
  const budget = await read(request, `${state.taskRoot}/budget`);
  const stopped = await stop(request, state, "journey", consent.id);
  await expect.poll(async () => (await read(request, path)).dispatch.status).toBe("settled");
  const settled = await read(request, path);
  expect(settled.record.revoked).toBe(true);
  expect(settled.sample).toBeNull();
  await expect.poll(async () => (await read(request, stopped.path)).observation?.state).toBe("unknown");
  expect((await read(request, `${state.taskRoot}/sample-operations`)).items).toEqual([]);
  expect((await read(request, `${state.taskRoot}/builder-operations`)).items).toHaveLength(1);
  expect((await read(request, `${state.taskRoot}/calls`)).some((call: any) => call.evidence?.phase === "sample_inference")).toBe(false);
  expect((await read(request, `${state.taskRoot}/budget`)).total_reserved_calls).toBeGreaterThanOrEqual(budget.total_reserved_calls);
  expect((await request.post(path, { data: {} })).status()).toBe(400);
  await replayStop(request, state, stopped);
});

test("chat Stop cancels a genuinely running standalone sample without refunding its admitted call", async ({ request, page }) => {
  test.setTimeout(240_000);
  const state = await fixture(request, page, "e2e-slow-sample");
  const draft = await build(request, state), sample = await startSample(request, state, draft);
  await expect.poll(async () => (await read(request, sample.path)).status).toBe("running");
  // Admission persists the receipt before response evidence exists; phase is
  // recorded only after completion. The standalone Builder already settled.
  await expect.poll(async () => (await read(request, `${state.taskRoot}/calls`)).some((call: any) => call.status === "reserved")).toBe(true);
  const budget = await read(request, `${state.taskRoot}/budget`);
  const stopped = await stop(request, state, "sample", sample.operation.id);
  await expect.poll(async () => (await read(request, sample.path)).status).toBe("cancelled");
  await expect.poll(async () => (await read(request, `${state.taskRoot}/calls`)).filter((call: any) => call.status === "reserved")).toEqual([]);
  await expect.poll(async () => (await read(request, stopped.path)).observation?.state).toBe("unknown");
  expect((await read(request, `${state.taskRoot}/sample-operations`)).items).toHaveLength(1);
  expect(await read(request, `${state.taskRoot}/processing-operations`)).toEqual([]);
  expect((await read(request, `${state.taskRoot}/budget`)).total_reserved_calls).toBe(budget.total_reserved_calls);
  await replayStop(request, state, stopped);
  // The explicit original sample retry is also idempotent, not a fresh worker.
  expect((await post(request, `/api/projects/${state.project}/sample-operations`, sample.envelope)).status).toBe("cancelled");
  expect((await read(request, `${state.taskRoot}/sample-operations`)).items).toHaveLength(1);
});

test("chat Stop reaches the real running dataset Batch and preserves its sample and publication", async ({ request, page }) => {
  test.setTimeout(240_000);
  const state = await fixture(request, page, "e2e-slow-sample");
  const draft = await build(request, state), sample = await startSample(request, state, draft);
  await expect.poll(async () => (await read(request, sample.path)).status).toBe("succeeded");
  await expect.poll(async () => (await read(request, sample.path)).assistance?.status).toBe("completed");
  const savedSamplePath = `/api/workflow-drafts/${draft}/sample-test?test_id=${sample.operation.id}`;
  const savedSample = await read(request, savedSamplePath);
  const selection = { draft_id: draft, sample_test_id: sample.operation.id, limit: 1 };
  const preview = await read(request, `/api/projects/${state.project}/processing-preview?${new URLSearchParams({ ...selection, limit: "1" })}`);
  expect(preview.conversation.task_id).toBe(state.task);
  expect(preview.models[0].remote_model_id).toBe("e2e-slow-sample");
  const command = { request_id: randomUUID(), selection, expected_revision: preview.revision, authorization_fingerprint: preview.authorization_fingerprint };
  const operation = await post(request, `/api/projects/${state.project}/processing-operations`, command);
  expect(operation.phase, JSON.stringify(operation)).toBe("started");
  const batchPath = `/api/batches/${operation.batch_id}`;
  await expect.poll(async () => (await read(request, batchPath)).batch.status).toBe("running");
  // Wait for real inference accounting as well as the Batch scheduler state.
  await expect.poll(async () => (await read(request, `${state.taskRoot}/budget`)).processing_reserved_calls).toBe(1);
  const running = await read(request, batchPath), budget = await read(request, `${state.taskRoot}/budget`);
  const childRuns = running.batch.images.map((image: any) => image.child_run_id).filter(Boolean);
  expect(childRuns).toHaveLength(1);
  const frozenWorkflow = running.checkpoint.batch.workflow_snapshot;
  // Exercise the normal processing workspace, not an API-only shortcut. The
  // same composer must remain available while a processing operation is open.
  await page.goto(`/projects/${state.project}/work?${new URLSearchParams({ conversation: state.conversation, task: state.task, draft, test: sample.operation.id, image: savedSample.sample_test.inputs[0].image_id, processing: operation.id })}`);
  const composer = page.getByRole("textbox", { name: "Your message", exact: true });
  await expect(composer).toBeVisible();
  await expect.poll(async () => (await read(request, batchPath)).batch.status).toBe("running");
  const stopResponse = page.waitForResponse(response => response.request().method() === "POST" && new URL(response.url()).pathname === `${state.root}/stop-requests`);
  await composer.fill("停止");
  await composer.locator("xpath=ancestor::form").locator('button[type="submit"]').click();
  const response = await stopResponse;
  expect(response.ok(), await response.text()).toBe(true);
  const message = response.request().postDataJSON(), record = await response.json();
  expect(message.reference).toEqual({ scope: "stop_request", task_id: state.task });
  expect(message.image).toBeNull();
  expect(record.status).toBe("cancel_requested");
  expect(record.dispatch_error).toBeNull();
  expect(record.targets).toHaveLength(1);
  expect(record.selected_target).toEqual({ kind: "processing", id: operation.id, task_id: state.task });
  const stopped = { message, record, path: `${state.root}/stop-requests/${message.id}` };
  await expect.poll(async () => (await read(request, batchPath)).batch.status).toBe("cancelled");
  // A cancelled scheduler is insufficient: its already admitted child worker
  // must actually receive the cancellation, not finish inference unnoticed.
  for (const childRunId of childRuns) {
    await expect.poll(async () => (await read(request, `/api/runs/${childRunId}`)).run.status).toBe("cancelled");
  }
  await expect.poll(async () => (await read(request, stopped.path)).observation?.state).not.toBe("cancel_pending");
  expect((await read(request, batchPath)).batch.images.map((image: any) => image.child_run_id).filter(Boolean)).toEqual(childRuns);
  expect((await read(request, batchPath)).checkpoint.batch.workflow_snapshot).toEqual(frozenWorkflow);
  expect(await read(request, savedSamplePath)).toEqual(savedSample);
  expect((await read(request, `${state.taskRoot}/budget`)).processing_reserved_calls).toBe(budget.processing_reserved_calls);
  const replay = await post(request, `/api/projects/${state.project}/processing-operations`, command);
  expect(replay.batch_id).toBe(operation.batch_id);
  expect(await read(request, `${state.taskRoot}/processing-operations`)).toHaveLength(1);
  await replayStop(request, state, stopped);
  const card = page.getByRole("region", { name: "Stop request", exact: true });
  await expect(card).toContainText("Cancellation request saved.");
  await card.evaluate(element => element.scrollIntoView({ block: "center" }));
  await page.screenshot({ path: isolatedEvidencePath("../docs/execution/conversational-workspace/chat-stop-batch-workspace.png"), animations: "disabled" });
});
