import { isolatedEvidencePath } from "./evidence";
import { randomUUID } from "node:crypto";
import type { APIRequestContext, Page } from "@playwright/test";
import { authorize as authorizeFeedback, clarification, sample } from "./conversation-feedback-helpers";
import { expect as baseExpect, fetchWithinMutationLimit, test } from "./fixtures";

const expect = baseExpect.configure({ timeout: 75_000 });

async function read(request: APIRequestContext, path: string) {
  const response = await request.get(path); expect(response.ok(), await response.text()).toBe(true); return response.json();
}
async function post(request: APIRequestContext, path: string, data: unknown) {
  const response = await request.post(path, { data, timeout: 90_000 }); expect(response.ok(), await response.text()).toBe(true); return response.json();
}

async function futureSource(request: APIRequestContext, page: Page, bbox = false, variant = "valid") {
  const base = await sample(request, page, bbox ? "future-base-clarify" : "clarify", bbox);
  const message = { ...base.message, id: randomUUID(), text: bbox
    ? "TEST future rule request: remove bottle from future labels; keep cup. For an occluded cup annotate only its visible region, never guess the hidden extent."
    : "TEST future rule request: remove 室外 from future labels; keep 室内. When scene evidence is obscured, ask for human review instead of inferring an excluded category." };
  await post(request, `${base.root}/messages`, message);
  // The synthetic bbox fixture explicitly creates a pending safety request. This
  // test cancels that TEST request, never bypasses the live pending-human gate.
  if (bbox) for (const waiting of base.savedRequests) await post(request, `${base.taskRoot}/human-requests/${waiting.input.id}/cancel`, {});
  const selected = { ...base, message };
  const feedback = await authorizeFeedback(request, selected);
  await post(request, `${feedback.path}/execute`, {});
  const feedbackStatus = await read(request, feedback.path);
  expect(feedbackStatus.receipt.status).toBe("completed");
  expect(feedbackStatus.decision.Ok.decision).toBe("clarify_scope");
  const answer = { command_id: randomUUID(), expected_context_digest: feedbackStatus.scope_context_digest, choice: { scope: "project_future_rule" } };
  await post(request, `${feedback.path}/scope-answer`, answer);
  const endpoint = `${feedback.path}/future-schema`, source = await read(request, endpoint);
  expect(source.base_schema.definition.task.labels).toEqual(bbox ? ["cup", "bottle"] : ["室内", "室外"]);
  expect(source.record).toBeNull();
  const model = await post(request, "/api/model-profiles", { provider_id: base.provider.id, display_name: `TEST future rule proposer ${variant}`, remote_model_id: `e2e-future-schema-proposal-${variant}`, input_modalities: ["text"], task_capabilities: ["text_generation"], protocol_features: { tool_calls: true, structured_output: true } });
  await post(request, `/api/providers/${base.provider.id}/active-probe`, { model_profile_id: model.id, confirmed_billable: true });
  const defaults = await read(request, "/api/agent-model-bindings");
  expect((await request.put("/api/agent-model-bindings", { data: { ...defaults, pipeline_builder: model.id } })).ok()).toBe(true);
  return { ...selected, feedback, answer, endpoint, source, proposalModel: model };
}
type State = Awaited<ReturnType<typeof futureSource>>;

async function preserved(request: APIRequestContext, state: State) {
  const drafts = await read(request, `/api/workflow-drafts?project_id=${state.project}`);
  return {
    base: await read(request, `/api/projects/${state.project}/conversation-schema-drafts/${state.source.base_schema.id}?revision=${state.source.base_schema.revision}`),
    goal: await read(request, `/api/projects/${state.project}/goal`),
    sample: await read(request, `/api/workflow-drafts/${state.record.draft_id}/sample-test?test_id=${state.record.id}`),
    draft: drafts.drafts.find((draft: any) => draft.id === state.record.draft_id),
    feedback: await read(request, `/api/workflow-sample-tests/${state.record.id}/images/${state.image.image_id}/feedback`),
    exportReadiness: await read(request, `/api/projects/${state.project}/export-readiness`),
    runs: await read(request, `/api/runs?project_id=${state.project}`),
  };
}
async function preview(request: APIRequestContext, state: State, call = randomUUID()) {
  return read(request, `${state.endpoint}/proposal-preview?${new URLSearchParams({ call_id: call, model_id: state.proposalModel.id })}`);
}
async function authorize(request: APIRequestContext, state: State) {
  const value = await preview(request, state), consent = { ...value.consent, allow_unknown_cost: true };
  const saved = await post(request, `${state.endpoint}/proposal`, consent);
  return { value, consent, saved };
}
function forkInput(state: State, status: any) {
  return {
    command_id: randomUUID(), expected_scope_answer_command_id: state.answer.command_id,
    expected_context_digest: state.source.source.context_digest,
    base_schema_id: state.source.base_schema.id, base_schema_revision: state.source.base_schema.revision,
    goal: status.proposal.Ok.goal, decision: status.proposal.Ok.decision,
    proposal_call_id: status.authorization.consent.call_id, proposal_digest: status.proposal_digest,
  };
}
async function proposalCalls(request: APIRequestContext, state: State) {
  return (await read(request, `${state.taskRoot}/calls`)).filter((call: any) => call.evidence?.phase === "future_schema_patch_text");
}

test("future rules expose an explicit model proposal authorization before editing or saving a new Schema", async ({ page, request }) => {
  test.setTimeout(120_000);
  const state = await clarification(request, page);
  const answer = await request.post(`${state.path}/scope-answer`, { data: {
    command_id: randomUUID(), expected_context_digest: state.status.scope_context_digest,
    choice: { scope: "project_future_rule" },
  } });
  expect(answer.ok(), await answer.text()).toBe(true);
  const before = await (await request.get(`${state.taskRoot}/calls`)).json();
  await page.goto(state.url);
  const card = page.getByRole("region", { name: "Future rule draft", exact: true });
  await expect(card.getByRole("button", { name: "Review model proposal authorization", exact: true })).toBeVisible({ timeout: 15_000 });
  expect(await (await request.get(`${state.taskRoot}/calls`)).json()).toEqual(before);
  expect((await (await request.get(`${state.path}/future-schema`)).json()).record).toBeNull();
});

for (const bbox of [false, true]) {
test(`future ${bbox ? "bbox" : "classification"} model proposal uses the exact tested rules and only forks after explicit acceptance`, async ({ page, request }) => {
  test.setTimeout(240_000);
  const state = await futureSource(request, page, bbox), before = await preserved(request, state);
  const priorCalls = await read(request, `${state.taskRoot}/calls`);
  const value = await preview(request, state);
  expect(value.maximum_calls).toBe(1); expect(value.image_count).toBe(0); expect(value.estimated_cost).toBeNull();
  expect(value.destination).toContain("127.0.0.1:8796");
  expect(value.consent.message_id).toBe(state.message.id);
  expect(value.source).toEqual({ feedback_call_id: state.feedback.consent.call_id, scope_answer_command_id: state.answer.command_id, context_digest: state.source.source.context_digest, base_schema_id: state.source.base_schema.id, base_schema_revision: 1 });
  expect(await read(request, `${state.endpoint}/proposal`)).toBeNull();
  expect((await request.post(`${state.endpoint}/proposal/execute`, { data: { call_id: value.consent.call_id } })).ok()).toBe(false);
  expect((await page.request.post(`${state.endpoint}/proposal`, { data: value.consent })).status()).toBe(403);
  expect((await request.post(`${state.endpoint}/proposal`, { data: value.consent })).ok()).toBe(false);
  const consent = { ...value.consent, allow_unknown_cost: true };
  expect((await request.post(`${state.endpoint}/proposal`, { data: { ...consent, scope_hash: "f".repeat(64) } })).ok()).toBe(false);
  expect((await request.post(`${state.endpoint}/proposal`, { data: { ...consent, auto_apply: true } })).status()).toBe(422);
  const saved = await post(request, `${state.endpoint}/proposal`, consent);
  expect(saved.receipt).toBeNull(); expect(saved.proposal).toBeNull();
  expect(saved.authorization.source).toEqual(value.source);
  expect(saved.authorization.context.base_schema).toEqual(state.source.base_schema);
  expect(saved.authorization.context.feedback.message.input).toEqual(state.message);
  expect(saved.authorization.context.feedback.pixels_supplied).toBe(false);
  expect(saved.authorization.context.scope).toBe("future_tasks_only");
  expect(await read(request, `${state.taskRoot}/calls`)).toEqual(priorCalls);
  expect(await preserved(request, state)).toEqual(before);
  const foreignMessage = randomUUID(), foreignTask = randomUUID();
  await post(request, `${state.root}/messages`, { id: foreignMessage, text: "TEST unrelated later goal; ignore earlier labels", image: null });
  await post(request, `${state.root}/tasks`, { id: foreignTask, source_message_id: foreignMessage, schema_revision: before.goal.revision });
  expect((await request.post(`${state.endpoint.replace(state.task, foreignTask)}/proposal/execute`, { data: { call_id: consent.call_id } })).ok()).toBe(false);
  expect((await request.post(`${state.endpoint}/proposal/execute`, { data: { call_id: consent.call_id, candidate_id: randomUUID() } })).status()).toBe(422);
  const results = await Promise.all([post(request, `${state.endpoint}/proposal/execute`, { call_id: consent.call_id }), post(request, `${state.endpoint}/proposal/execute`, { call_id: consent.call_id })]);
  expect(results[0].authorization.consent.call_id).toBe(consent.call_id);
  const status = await read(request, `${state.endpoint}/proposal`);
  expect(status.receipt.status).toBe("completed"); expect(status.proposal_digest).toMatch(/^[a-f0-9]{64}$/);
  const decision = status.proposal.Ok.decision;
  expect(decision.decision).toBe("draft"); expect(decision.kind).toBe(bbox ? "bounding_box" : "classification");
  expect(decision.labels).toEqual([bbox ? "cup" : "室内"]);
  expect(decision.attributes).toEqual(state.source.base_schema.definition.task.attributes);
  expect(decision.multi_label).toBe(state.source.base_schema.definition.task.multi_label);
  expect(decision.boundary_rules).toEqual([...state.source.base_schema.definition.boundary_rules, bbox
    ? "TEST occluded targets: annotate only the visible region; never infer the hidden extent."
    : "TEST obscured scene evidence requires human review; never infer an excluded category."]);
  expect(decision.rationale).toContain(state.source.base_schema.id);
  expect(await proposalCalls(request, state)).toHaveLength(1);
  expect((await read(request, `${state.endpoint}`)).record).toBeNull();
  expect(await preserved(request, state)).toEqual(before);
  const input = forkInput(state, status), budget = await read(request, `${state.taskRoot}/budget`);
  expect((await request.post(state.endpoint, { data: { ...input, proposal_digest: "f".repeat(64) } })).ok()).toBe(false);
  expect((await request.post(state.endpoint, { data: { ...input, proposal_call_id: randomUUID() } })).ok()).toBe(false);
  const adopted = await post(request, state.endpoint, input);
  expect(adopted.schema.id).not.toBe(state.source.base_schema.id); expect(adopted.schema.revision).toBe(1);
  expect(adopted.schema.definition.task.labels).toEqual(decision.labels);
  expect(adopted.record.input.proposal_call_id).toBe(consent.call_id);
  expect(adopted.record.input.proposal_digest).toBe(status.proposal_digest);
  expect(await post(request, state.endpoint, input)).toEqual(adopted);
  expect(await preserved(request, state)).toEqual(before);
  expect(await read(request, `${state.taskRoot}/budget`)).toEqual(budget);
  expect(await proposalCalls(request, state)).toHaveLength(1);
  const selection = { operation_id: randomUUID(), schema_id: adopted.schema.id, schema_revision: "1", model_id: state.model.id };
  const builder = await read(request, `${state.taskRoot}/builder-preview?${new URLSearchParams(selection)}`);
  expect(builder.selection.schema_id).toBe(adopted.schema.id); expect(builder.selection.schema_revision).toBe(1);
  expect(await proposalCalls(request, state)).toHaveLength(1);
  // Saving the suggestion is not authorization to build or sample. These are
  // separate, explicit TEST-only admissions against the new exact Schema.
  const builderAuthorization = { selection: builder.selection, previous_grant_id: builder.previous_grant_id, scope_hash: builder.scope_hash, expires_at: builder.expires_at, allow_unknown_cost: true };
  expect((await request.post(`${state.taskRoot}/builder-operations`, { data: { ...builderAuthorization, allow_unknown_cost: false } })).ok()).toBe(false);
  const operation = await post(request, `${state.taskRoot}/builder-operations`, builderAuthorization);
  expect(operation.status).toBe("completed");
  expect(operation.evidence.schema_id).toBe(adopted.schema.id); expect(operation.evidence.schema_revision).toBe(1);
  expect(operation.evidence.draft_id).not.toBe(state.record.draft_id);
  expect(operation.evidence.samples_tested).toBe(false); expect(operation.evidence.published).toBe(false);
  expect(await preserved(request, state)).toEqual(before);
  const callsAfterBuilder = await read(request, `${state.taskRoot}/calls`), budgetAfterBuilder = await read(request, `${state.taskRoot}/budget`);
  await post(request, `${state.taskRoot}/builder-operations`, builderAuthorization);
  expect(await read(request, `${state.taskRoot}/calls`)).toEqual(callsAfterBuilder);
  const sampleId = randomUUID(), newDraft = operation.evidence.draft_id;
  const samplePreview = await read(request, `${state.taskRoot}/sample-preview?${new URLSearchParams({ draft_id: newDraft, request_id: sampleId })}`);
  expect(samplePreview.supported).toBe(true); expect(samplePreview.image_count).toBe(1); expect(samplePreview.estimated_cost).toBeNull();
  for (const model of samplePreview.models) expect(model.destination).toContain("127.0.0.1:8796");
  const sampleRequest = { request_id: sampleId, draft_id: newDraft, expected_revision: samplePreview.revision, image_indices: [0], authorization_fingerprint: samplePreview.authorization_fingerprint, conversation: { conversation_id: state.conversation, task_id: state.task, previous_grant_id: samplePreview.conversation_budget.previous_grant_id, scope_hash: samplePreview.conversation_budget.scope_hash, expires_at: samplePreview.conversation_budget.expires_at, allow_unknown_cost: true, human_review: true } };
  const sampleRoot = `/api/projects/${state.project}/sample-operations`;
  expect((await request.post(sampleRoot, { data: { ...sampleRequest, conversation: { ...sampleRequest.conversation, allow_unknown_cost: false } } })).ok()).toBe(false);
  expect(await read(request, `${state.taskRoot}/calls`)).toEqual(callsAfterBuilder);
  await post(request, sampleRoot, sampleRequest);
  await expect.poll(async () => (await read(request, `${sampleRoot}/${sampleId}`)).status).toBe("succeeded");
  await expect.poll(async () => (await read(request, `${sampleRoot}/${sampleId}`)).assistance?.status).toBe("completed");
  const sampleView = await read(request, `/api/workflow-drafts/${newDraft}/sample-test?test_id=${sampleId}`);
  expect(sampleView.annotation_schema.schema_draft_id).toBe(adopted.schema.id); expect(sampleView.annotation_schema.revision).toBe(1);
  expect(sampleView.annotation_schema.goal).toBe(adopted.schema.definition.goal);
  expect(sampleView.annotation_schema.task).toEqual(adopted.schema.definition.task);
  expect(sampleView.sample_test.inputs).toEqual(state.record.inputs);
  const result = sampleView.sample_test.report.samples[0]; expect(result.failed).toBe(false);
  const terminal = [...result.projection.final_candidates, ...result.projection.review_candidates.map((item: any) => item.candidate)];
  expect(terminal.length).toBeGreaterThan(0);
  for (const candidate of terminal) expect(decision.labels).toContain(candidate.outcome.label);
  expect(await preserved(request, state)).toEqual(before);
  expect(await proposalCalls(request, state)).toHaveLength(1);
  const callsAfterSample = await read(request, `${state.taskRoot}/calls`), budgetAfterSample = await read(request, `${state.taskRoot}/budget`);
  expect(callsAfterSample.length).toBeGreaterThan(callsAfterBuilder.length); expect(budgetAfterSample).not.toEqual(budgetAfterBuilder);
  await post(request, sampleRoot, sampleRequest);
  expect(await read(request, `${state.taskRoot}/calls`)).toEqual(callsAfterSample); expect(await read(request, `${state.taskRoot}/budget`)).toEqual(budgetAfterSample);
  expect(await preserved(request, state)).toEqual(before);
});
}

for (const variant of ["invalid", "unknown", "clarify"]) {
test(`future model ${variant} output never creates a Schema or silently reissues the request`, async ({ page, request }) => {
  test.setTimeout(240_000);
  const state = await futureSource(request, page, false, variant), before = await preserved(request, state);
  const { consent } = await authorize(request, state);
  await post(request, `${state.endpoint}/proposal/execute`, { call_id: consent.call_id });
  const status = await read(request, `${state.endpoint}/proposal`);
  expect(status.receipt.status).toBe(variant === "unknown" ? "in_doubt" : "completed");
  if (variant === "invalid") expect(status.proposal.Err).toBeTruthy();
  if (variant === "unknown") { expect(status.proposal).toBeNull(); expect(status.error).toMatch(/unknown|unavailable/i); }
  if (variant === "clarify") { expect(status.proposal.Ok.decision.decision).toBe("clarify"); expect(status.proposal.Ok.decision.question).toContain("occluded"); }
  expect((await read(request, state.endpoint)).record).toBeNull();
  expect(await preserved(request, state)).toEqual(before);
  const calls = await read(request, `${state.taskRoot}/calls`), budget = await read(request, `${state.taskRoot}/budget`);
  const writes: string[] = [];
  page.on("request", request => { if (request.method() === "POST") writes.push(request.url()); });
  await page.goto(state.url); await page.reload();
  await expect(page.getByRole("region", { name: "Future rule draft", exact: true })).toBeVisible();
  expect(writes).toEqual([]); expect(await read(request, `${state.taskRoot}/calls`)).toEqual(calls);
  expect(await read(request, `${state.taskRoot}/budget`)).toEqual(budget);
  await post(request, `${state.endpoint}/proposal/execute`, { call_id: consent.call_id });
  expect(await read(request, `${state.taskRoot}/calls`)).toEqual(calls);
  expect((await read(request, state.endpoint)).record).toBeNull();
});
}

for (const change of ["model", "tested-schema"]) {
test(`future model authorization refuses a changed ${change} without another model call`, async ({ page, request }) => {
  test.setTimeout(240_000);
  const state = await futureSource(request, page), { consent } = await authorize(request, state);
  if (change === "model") expect((await request.patch(`/api/model-profiles/${state.proposalModel.id}`, { data: { remote_model_id: "e2e-future-schema-proposal-changed" } })).ok()).toBe(true);
  else await post(request, `/api/projects/${state.project}/conversation-schema-drafts/${state.source.base_schema.id}`, {
    request_id: randomUUID(), expected_revision: 1,
    decision: { decision: "draft", kind: "classification", labels: ["TEST different rules"], multi_label: false, attributes: {}, boundary_rules: ["TEST changed after consent"], rationale: "TEST explicit user edit" },
  });
  const calls = await read(request, `${state.taskRoot}/calls`), before = await preserved(request, state);
  expect((await request.post(`${state.endpoint}/proposal/execute`, { data: { call_id: consent.call_id } })).ok()).toBe(false);
  expect(await read(request, `${state.taskRoot}/calls`)).toEqual(calls);
  expect(await preserved(request, state)).toEqual(before);
  expect((await read(request, `${state.endpoint}/proposal`)).receipt).toBeNull();
});
}

test("future bbox proposal stays editable, preserves removed-label evidence and saves a separate human-reviewed Schema", async ({ page, request }) => {
  test.setTimeout(240_000);
  const state = await futureSource(request, page, true), before = await preserved(request, state);
  await page.goto(state.url);
  const card = page.getByRole("region", { name: "Future rule draft", exact: true });
  const model = card.getByRole("region", { name: "Future rule model proposal", exact: true });
  const posts: { path: string; input: any }[] = [];
  page.on("request", request => { if (request.method() === "POST") posts.push({ path: new URL(request.url()).pathname, input: request.postDataJSON() }); });
  await model.getByRole("button", { name: "Review model proposal authorization", exact: true }).click();
  const consent = model.getByRole("region", { name: "Future rule model authorization", exact: true });
  await expect(consent).toContainText("Cost unknown"); await expect(consent).toContainText("127.0.0.1:8796");
  await expect(consent).toContainText("No image pixels");
  await expect(consent.getByRole("button", { name: "Propose future rules", exact: true })).toBeDisabled();
  expect(posts).toEqual([]); expect(await proposalCalls(request, state)).toEqual([]);
  await consent.getByRole("checkbox", { name: "Allow this one future-rule text request; actual cost is unknown", exact: true }).check();
  await consent.getByRole("button", { name: "Propose future rules", exact: true }).click();
  await expect(model.getByText("Future-rule suggestion saved; not yet adopted", { exact: true })).toBeVisible();
  await expect(model.getByRole("region", { name: "Future rule changes", exact: true })).toContainText("bottle");
  await expect(model.getByRole("region", { name: "Future rule changes", exact: true })).toContainText("visible region");
  expect(posts.filter(item => item.path === state.endpoint)).toEqual([]);
  expect((await read(request, state.endpoint)).record).toBeNull(); expect(await preserved(request, state)).toEqual(before);
  const status = await read(request, `${state.endpoint}/proposal`);
  expect(await proposalCalls(request, state)).toHaveLength(1);
  await page.setViewportSize({ width: 1280, height: 1400 });
  const divider = page.getByRole("separator", { name: "Resize conversation panel", exact: true });
  for (let index = 0; index < 13; index++) await divider.press("ArrowRight");
  await model.evaluate(element => element.scrollIntoView({ block: "center" }));
  await expect(model.getByRole("button", { name: "Save proposed future rule draft", exact: true })).toBeInViewport();
  await page.screenshot({ path: isolatedEvidencePath("../docs/execution/conversational-workspace/future-model-proposal-diff.png"), animations: "disabled" });
  await model.getByRole("button", { name: "Edit proposed future rules", exact: true }).click();
  await expect(card.getByLabel("Labels · one per line", { exact: true })).toHaveValue("cup");
  const humanRule = "TEST human review: ambiguous occlusion must remain a review item.";
  const rules = await card.getByLabel("Boundary rules · one per line", { exact: true }).inputValue();
  await card.getByLabel("Boundary rules · one per line", { exact: true }).fill(`${rules}\n${humanRule}`);
  await card.getByLabel("Future task goal", { exact: true }).fill("TEST human-reviewed future cup boxes; exclude bottles and retain visible occlusion boundaries.");
  const postsBeforeReload = posts.length;
  await page.reload();
  await expect(card.getByLabel("Boundary rules · one per line", { exact: true })).toHaveValue(`${rules}\n${humanRule}`);
  expect(posts).toHaveLength(postsBeforeReload);
  const budget = await read(request, `${state.taskRoot}/budget`);
  await card.getByRole("button", { name: "Save future rule draft", exact: true }).click();
  await expect(card.getByText("Future rule draft saved", { exact: true })).toBeVisible();
  const adopted = await read(request, state.endpoint);
  expect(adopted.record.input.proposal_call_id).toBe(status.authorization.consent.call_id);
  expect(adopted.record.input.proposal_digest).toBe(status.proposal_digest);
  expect(adopted.schema.definition.task.labels).toEqual(["cup"]);
  expect(adopted.schema.definition.boundary_rules).toEqual([...status.proposal.Ok.decision.boundary_rules, humanRule]);
  expect(adopted.schema.id).not.toBe(state.source.base_schema.id);
  expect(await preserved(request, state)).toEqual(before); expect(await read(request, `${state.taskRoot}/budget`)).toEqual(budget);
  const saveCount = posts.filter(item => item.path === state.endpoint).length;
  expect(saveCount).toBe(1);
  await page.reload();
  await expect(card.getByText("Future rule draft saved", { exact: true })).toBeVisible();
  expect(posts.filter(item => item.path === state.endpoint)).toHaveLength(saveCount);
  expect(await proposalCalls(request, state)).toHaveLength(1);
  const original = page.getByRole("region", { name: "Annotation Schema proposal", exact: true }).getByRole("region", { name: "Saved label draft", exact: true });
  await expect(original.getByText("bottle", { exact: true }).first()).toBeVisible();
  const builder = card.getByRole("region", { name: "Build and test annotation plan", exact: true });
  await expect(builder.getByRole("button", { name: "Review build and sample authorization", exact: true })).toBeVisible();
  await expect(builder.getByRole("button", { name: "View sample results in canvas", exact: true })).toHaveCount(0);
  const comparison = card.getByRole("region", { name: "Future rule changes", exact: true }).locator(".conversation-future-schema-comparison").first();
  expect(await comparison.evaluate(element => element.getBoundingClientRect().width)).toBeLessThan(440);
  await expect.poll(async () => comparison.evaluate(element => {
    const [before, after] = Array.from(element.children).map(child => child.getBoundingClientRect());
    return after.top >= before.bottom;
  }), { timeout: 2500, message: "Default narrow conversation content must stack the two rule descriptions, even on a desktop viewport" }).toBe(true);
  await page.setViewportSize({ width: 1280, height: 1800 });
  // Reload resets the workspace split preference. Reapply its real accessible
  // control so both columns of the rule comparison are readable in this capture.
  for (let index = 0; index < 13; index++) await divider.press("ArrowRight");
  await expect(divider).toHaveAttribute("aria-valuenow", "50");
  await card.evaluate(element => element.scrollIntoView({ block: "center" }));
  await expect(card.getByText("Future rule draft saved", { exact: true })).toBeInViewport();
  await expect(builder.getByRole("button", { name: "Review build and sample authorization", exact: true })).toBeInViewport();
  await page.screenshot({ path: isolatedEvidencePath("../docs/execution/conversational-workspace/future-model-proposal-human-reviewed.png"), animations: "disabled" });
});

test("future proposal authorization with a lost ACK restores the original call before any explicit execution", async ({ page, request }) => {
  test.setTimeout(240_000);
  const state = await futureSource(request, page), before = await preserved(request, state);
  await page.goto(state.url);
  const card = page.getByRole("region", { name: "Future rule model proposal", exact: true });
  await card.getByRole("button", { name: "Review model proposal authorization", exact: true }).click();
  await card.getByRole("checkbox", { name: "Allow this one future-rule text request; actual cost is unknown", exact: true }).check();
  let saved: any, blockRecovery = true;
  const posts: { path: string; input: any }[] = [];
  page.on("request", request => { if (request.method() === "POST") posts.push({ path: new URL(request.url()).pathname, input: request.postDataJSON() }); });
  await page.route(`**${state.endpoint}/proposal`, async route => {
    if (route.request().method() === "GET" && blockRecovery) return route.abort("failed");
    if (route.request().method() !== "POST" || saved) return route.fallback();
    const response = await fetchWithinMutationLimit(route); expect(response.ok(), await response.text()).toBe(true);
    saved = await response.json(); await route.abort("failed");
  });
  await card.getByRole("button", { name: "Propose future rules", exact: true }).click();
  await expect(card.getByRole("button", { name: "Retry same future rule request", exact: true })).toBeEnabled();
  expect(saved.receipt).toBeNull(); expect(await proposalCalls(request, state)).toEqual([]);
  expect(posts.filter(item => item.path.endsWith("/proposal/execute"))).toEqual([]);
  blockRecovery = false;
  await page.reload();
  await expect(card.getByRole("button", { name: "Continue saved future rule request", exact: true })).toBeVisible();
  expect(posts).toHaveLength(1); expect(await proposalCalls(request, state)).toEqual([]);
  const originalCall = saved.authorization.consent.call_id;
  await card.getByRole("button", { name: "Continue saved future rule request", exact: true }).click();
  await expect(card.getByText("Future-rule suggestion saved; not yet adopted", { exact: true })).toBeVisible();
  const completed = await read(request, `${state.endpoint}/proposal`);
  expect(completed.authorization.consent.call_id).toBe(originalCall);
  expect(posts.filter(item => item.path.endsWith("/proposal/execute")).map(item => item.input)).toEqual([{ call_id: originalCall }]);
  expect(posts.filter(item => item.path.endsWith("/proposal")).every(item => item.input.call_id === originalCall)).toBe(true);
  expect(await proposalCalls(request, state)).toHaveLength(1); expect(await preserved(request, state)).toEqual(before);
  expect((await read(request, state.endpoint)).record).toBeNull();
});

test("a genuinely running future-rule model proposal can be stopped without a Schema fork or an automatic retry", async ({ page, request }) => {
  test.setTimeout(240_000);
  const state = await futureSource(request, page, false, "slow"), before = await preserved(request, state);
  const { consent } = await authorize(request, state);
  await page.goto(state.url);
  const card = page.getByRole("region", { name: "Future rule model proposal", exact: true });
  await card.getByRole("button", { name: "Continue saved future rule request", exact: true }).click();
  await expect.poll(async () => (await read(request, `${state.endpoint}/proposal`)).receipt?.status).toBe("reserved");
  await card.getByRole("button", { name: "Stop future rule request", exact: true }).click();
  await expect.poll(async () => (await read(request, `${state.endpoint}/proposal`)).receipt?.status).toBe("in_doubt");
  const cancelled = await read(request, `${state.endpoint}/proposal`);
  expect(cancelled.cancelled).toBe(true); expect(cancelled.authorization.consent.call_id).toBe(consent.call_id);
  expect((await read(request, state.endpoint)).record).toBeNull(); expect(await preserved(request, state)).toEqual(before);
  const calls = await read(request, `${state.taskRoot}/calls`), writes: string[] = [];
  page.on("request", request => { if (request.method() === "POST") writes.push(request.url()); });
  await page.reload();
  await expect(card).toContainText("Future-rule model request cancelled.");
  await expect(card.getByRole("button", { name: "Save proposed future rule draft", exact: true })).toHaveCount(0);
  expect(writes).toEqual([]); expect(await read(request, `${state.taskRoot}/calls`)).toEqual(calls);
});

test("a manually saved future draft never hides the independently running proposal or its stop control", async ({ page, request }) => {
  test.setTimeout(240_000);
  const state = await futureSource(request, page, false, "manual-slow"), before = await preserved(request, state);
  const { consent } = await authorize(request, state);
  await page.goto(state.url);
  const card = page.getByRole("region", { name: "Future rule draft", exact: true });
  const model = card.getByRole("region", { name: "Future rule model proposal", exact: true });
  await model.getByRole("button", { name: "Continue saved future rule request", exact: true }).click();
  await expect.poll(async () => (await read(request, `${state.endpoint}/proposal`)).receipt?.status).toBe("reserved");
  await card.getByRole("button", { name: "Edit future rule draft", exact: true }).click();
  await card.getByLabel("Future task goal", { exact: true }).fill("TEST independently human-authored future indoor task");
  await card.getByLabel("Labels · one per line", { exact: true }).fill("室内");
  await card.getByLabel("Boundary rules · one per line", { exact: true }).fill("TEST preserve uncertainty for human review");
  await card.getByRole("button", { name: "Save future rule draft", exact: true }).click();
  await expect(card.getByText("Future rule draft saved", { exact: true })).toBeVisible();
  const adopted = await read(request, state.endpoint);
  expect(adopted.record.input.proposal_call_id ?? null).toBeNull();
  expect((await read(request, `${state.endpoint}/proposal`)).receipt.status).toBe("reserved");
  await expect(model.getByRole("button", { name: "Stop future rule request", exact: true })).toBeVisible({ timeout: 2500 });
  await expect(model).toContainText(/Request admitted|still running|in flight/i);
  await model.getByRole("button", { name: "Stop future rule request", exact: true }).click();
  await expect.poll(async () => (await read(request, `${state.endpoint}/proposal`)).cancelled).toBe(true);
  await expect.poll(async () => (await read(request, `${state.endpoint}/proposal`)).receipt.status).toBe("in_doubt");
  expect(await read(request, state.endpoint)).toEqual(adopted); expect(await preserved(request, state)).toEqual(before);
  const writes: string[] = [];
  page.on("request", value => { if (value.method() === "POST") writes.push(value.url()); });
  await page.reload();
  await expect(card.getByText("Future rule draft saved", { exact: true })).toBeVisible();
  await expect(model).toContainText(/cancelled/i);
  expect(writes).toEqual([]); expect(await read(request, state.endpoint)).toEqual(adopted);
  expect((await read(request, `${state.endpoint}/proposal`)).authorization.consent.call_id).toBe(consent.call_id);
});

test("a future proposal cancelled in another tab after a lost authorization ACK retains its original identity", async ({ page, request }) => {
  test.setTimeout(240_000);
  const state = await futureSource(request, page), before = await preserved(request, state);
  await page.goto(state.url);
  const card = page.getByRole("region", { name: "Future rule model proposal", exact: true });
  await card.getByRole("button", { name: "Review model proposal authorization", exact: true }).click();
  await card.getByRole("checkbox", { name: "Allow this one future-rule text request; actual cost is unknown", exact: true }).check();
  let input: any;
  const posts: { path: string; input: any }[] = [];
  page.on("request", value => { if (value.method() === "POST") posts.push({ path: new URL(value.url()).pathname, input: value.postDataJSON() }); });
  await page.route(`**${state.endpoint}/proposal`, async route => {
    if (route.request().method() !== "POST") return route.fallback();
    input = route.request().postDataJSON(); await route.abort("failed");
  }, { times: 1 });
  await card.getByRole("button", { name: "Propose future rules", exact: true }).click();
  await expect(card.getByRole("button", { name: "Retry same future rule request", exact: true })).toBeEnabled();
  expect(await read(request, `${state.endpoint}/proposal`)).toBeNull();
  // Another authenticated client cancels exactly the retained, not-yet-admitted
  // call. This is a real persisted tombstone, not a mocked status response.
  const cancellation = await post(request, `${state.taskRoot}/calls/${input.call_id}/cancel`, {});
  expect(cancellation.call_id).toBe(input.call_id);
  const rejected = page.waitForResponse(value => value.url().endsWith(`${state.endpoint}/proposal`) && value.request().method() === "POST");
  await card.getByRole("button", { name: "Retry same future rule request", exact: true }).click();
  expect((await rejected).status()).toBe(400);
  await expect(card).toContainText("Cancellation saved for this future-rule request", { timeout: 2500 });
  await expect(card.getByRole("button", { name: "Review model proposal authorization", exact: true })).toHaveCount(0);
  await page.reload();
  await expect(card).toContainText("Cancellation saved for this future-rule request", { timeout: 2500 });
  await expect(card.getByRole("button", { name: "Review model proposal authorization", exact: true })).toHaveCount(0);
  expect(posts.filter(value => value.path.endsWith("/proposal"))).toHaveLength(2);
  expect(posts.filter(value => value.path.endsWith("/proposal")).every(value => value.input.call_id === input.call_id)).toBe(true);
  expect(posts.filter(value => value.path.endsWith("/execute"))).toEqual([]);
  expect(await proposalCalls(request, state)).toEqual([]); expect(await preserved(request, state)).toEqual(before);
  expect(await read(request, `${state.endpoint}/proposal`)).toBeNull();
});
