import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { APIRequestContext, Page } from "@playwright/test";
import { authorize, clarification, sample } from "./conversation-feedback-helpers";
import { expect as baseExpect, fetchWithinMutationLimit, test } from "./fixtures";

const expect = baseExpect.configure({ timeout: 75_000 });
async function read(request: APIRequestContext, path: string) {
  const response = await request.get(path); expect(response.ok(), await response.text()).toBe(true); return response.json();
}
async function post(request: APIRequestContext, path: string, data: unknown) {
  const response = await request.post(path, { data }); expect(response.ok(), await response.text()).toBe(true); return response.json();
}
function terminal(sample: any) {
  return [...sample.projection.final_candidates, ...sample.projection.review_candidates.map((item: any) => item.candidate)];
}
function typedFeedback(revisions: any[]) {
  // The typed Review endpoint emits shortest f32 decimals while the existing
  // generic feedback JSON endpoint expands the same values to f64. Compare the
  // exact Rust geometry representation, preserving every other field verbatim.
  return revisions.map(revision => revision.corrected_value?.kind === "bounding_box" ? {
    ...revision, corrected_value: { ...revision.corrected_value, rect: revision.corrected_value.rect.map(Math.fround) },
  } : revision);
}

async function imageClassSample(request: APIRequestContext, page: Page, bbox: boolean) {
  const base = await sample(request, page, "image-class", bbox);
  // Only TEST pending requests are explicitly cancelled so the next bounded
  // sample can run. Product authorization never bypasses a waiting human.
  for (const waiting of base.savedRequests) await post(request, `${base.taskRoot}/human-requests/${waiting.input.id}/cancel`, {});
  await page.getByLabel("Add images", { exact: true }).setInputFiles({ name: "TEST-other-image.png", mimeType: "image/png", buffer: readFileSync(resolve("public/brand/core/pwa-192.png")) });
  await expect(page.getByRole("button", { name: "TEST-other-image.png", exact: true })).toBeVisible();
  let draft = base.record.draft_id;
  if (!bbox) {
    // The real classifier protocol still returns ONE label per subject. A real
    // explicit Schema edit permits a later human-authored multi-label baseline;
    // neither fixture nor UI falsely claims the model produced multiple labels.
    const original = await read(request, `/api/workflow-drafts/${draft}/sample-test?test_id=${base.record.id}`);
    const schemaId = original.annotation_schema.schema_draft_id;
    await post(request, `/api/projects/${base.project}/conversation-schema-drafts/${schemaId}`, {
      request_id: randomUUID(), expected_revision: original.annotation_schema.revision,
      decision: { decision: "draft", kind: "classification", labels: ["室内", "室内运动场", "室外"], multi_label: true, attributes: {}, boundary_rules: ["TEST human review may retain multiple scene categories"], rationale: "TEST explicit human-defined multi-label semantics" },
    });
    const selection = { operation_id: randomUUID(), schema_id: schemaId, schema_revision: "2", model_id: base.model.id };
    const preview = await read(request, `${base.taskRoot}/builder-preview?${new URLSearchParams(selection)}`);
    const built = await post(request, `${base.taskRoot}/builder-operations`, { selection: preview.selection, previous_grant_id: preview.previous_grant_id, scope_hash: preview.scope_hash, expires_at: preview.expires_at, allow_unknown_cost: true });
    expect(built.status).toBe("completed"); draft = built.evidence.draft_id;
  }
  const sampleId = randomUUID();
  const preview = await read(request, `${base.taskRoot}/sample-preview?${new URLSearchParams({ draft_id: draft, request_id: sampleId })}`);
  expect(preview.supported).toBe(true); expect(preview.image_count).toBe(2);
  const command = { request_id: sampleId, draft_id: draft, expected_revision: preview.revision, image_indices: [0, 1], authorization_fingerprint: preview.authorization_fingerprint, conversation: { conversation_id: base.conversation, task_id: base.task, previous_grant_id: preview.conversation_budget.previous_grant_id, scope_hash: preview.conversation_budget.scope_hash, expires_at: preview.conversation_budget.expires_at, allow_unknown_cost: true, human_review: true } };
  const sampleRoot = `/api/projects/${base.project}/sample-operations`;
  await post(request, sampleRoot, command);
  await expect.poll(async () => (await read(request, `${sampleRoot}/${sampleId}`)).status).toBe("succeeded");
  await expect.poll(async () => (await read(request, `${sampleRoot}/${sampleId}`)).assistance?.status).toBe("completed");
  const view = await read(request, `/api/workflow-drafts/${draft}/sample-test?test_id=${sampleId}`), record = view.sample_test;
  expect(record.inputs).toHaveLength(2);
  const index = record.inputs.findIndex((image: any) => image.image_id === base.image.image_id);
  expect(index).toBeGreaterThanOrEqual(0);
  const image = record.inputs[index], other = record.inputs[1 - index], candidates = terminal(record.report.samples[index]);
  const candidate = candidates.find((item: any) => item.outcome.label === (bbox ? "cup" : "室内")); expect(candidate).toBeTruthy();
  if (bbox) {
    expect(candidates.filter((item: any) => item.outcome.label === "cup")).toHaveLength(2);
    expect(candidates.filter((item: any) => item.outcome.label === "cupcake")).toHaveLength(1);
    expect(candidates.filter((item: any) => item.outcome.label === "bottle")).toHaveLength(1);
    expect(terminal(record.report.samples[1 - index]).some((item: any) => item.outcome.label === "cup")).toBe(true);
  } else {
    expect(candidate.outcome.value.labels).toEqual(["室内"]);
    await post(request, `/api/workflow-sample-tests/${record.id}/images/${image.image_id}/feedback`, {
      revision_id: randomUUID(), sample_test_id: record.id, image_id: image.image_id, sequence: 1, reason: "wrong_target", outcome_id: candidate.outcome.id,
      corrected_value: { kind: "classification", labels: ["室内", "室内运动场"] }, corrected_label: "室内, 室内运动场",
      note: "TEST human-authored multi-label correction, not model-produced labels", created_at: new Date().toISOString(),
    });
  }
  for (const waiting of await read(request, `${base.taskRoot}/human-requests`)) await post(request, `${base.taskRoot}/human-requests/${waiting.input.id}/cancel`, {});
  const message = { ...base.message, id: randomUUID(), text: "TEST review this class only in the selected image, not other images or future rules", image: { image_id: image.image_id, sha256: image.content_hash }, reference: { ...base.message.reference, draft_id: draft, draft_revision: record.draft_revision, sample_test_id: record.id, candidate_id: candidate.outcome.id, source_artifact_id: candidate.source_artifact_id } };
  await post(request, `${base.root}/messages`, message);
  const selected = { ...base, record, candidate, image, message };
  const feedback = await authorize(request, selected);
  await post(request, `${feedback.path}/execute`, {});
  const status = await read(request, feedback.path); expect(status.decision.Ok.decision).toBe("clarify_scope");
  await post(request, `${feedback.path}/scope-answer`, { command_id: randomUUID(), expected_context_digest: status.scope_context_digest, choice: { scope: "current_image_class" } });
  const url = `/projects/${base.project}/work?${new URLSearchParams({ conversation: base.conversation, task: base.task, draft, test: record.id, image: image.image_id })}`;
  return { ...selected, ...feedback, other, view, candidates, url, target: bbox ? "cup" : "室内" };
}
type State = Awaited<ReturnType<typeof imageClassSample>>;

for (const bbox of [true,false]) {
for (const transport of ["normal","lost_ack","storage_blocked","storage_corrupt"]) {
const lostAck=transport==="lost_ack";
test(`image class repair continues in chat with separate Builder and sample consent: ${bbox ? "bbox" : "classification"} ${transport}`, async ({page,request})=>{
  test.setTimeout(240_000);
  const state=await imageClassSample(request,page,bbox), group=await openGroup(request,state);
  await post(request,`${group.endpoint}/answer`,{command_id:randomUUID(),expected_scope_digest:group.review.scope_digest,actions:group.review.scope.members.map((member:any)=>({action:"keep",outcome_id:member.outcome.id,source_artifact_id:member.source_artifact_id}))});
  const applied=await post(request,`${group.endpoint}/resume`,{});
  expect(applied.status).toBe("applied");
  const callsBefore=await read(request,`${state.taskRoot}/calls`);
  await page.goto(state.url);
  const card=page.getByRole("region",{name:"Repair annotation pipeline",exact:true});
  await expect(card.getByRole("button",{name:"Review Builder authorization",exact:true})).toBeVisible();
  await page.reload();
  await expect(card.getByRole("button",{name:"Review Builder authorization",exact:true})).toBeVisible();
  expect(await read(request,`${state.taskRoot}/calls`)).toEqual(callsBefore);
  if(transport==="storage_corrupt"){
    const original=await read(request,`/api/workflow-drafts/${state.record.draft_id}/sample-test?test_id=${state.record.id}`);
    const schema=original.annotation_schema;
    const key=`annotagent.builder-pending:${state.project}:${state.conversation}:${state.task}:${schema.schema_draft_id}:${schema.revision}:image_class_review:${group.review.id}`;
    await page.evaluate(key=>sessionStorage.setItem(key,"{TEST damaged envelope"),key);
    await page.reload();
    await expect(card.getByRole("button",{name:"Review Builder authorization",exact:true})).toBeDisabled();
    await card.getByRole("button",{name:"Discard unreadable local Builder retry record",exact:true}).click();
    expect(await read(request,`${state.taskRoot}/calls`)).toEqual(callsBefore);
    await expect(card.getByRole("button",{name:"Review Builder authorization",exact:true})).toBeEnabled();
  }
  if(transport==="storage_blocked")await page.evaluate(()=>{
    const original=Storage.prototype.setItem;
    Storage.prototype.setItem=function(key,value){if(key.startsWith("annotagent.builder-pending:"))throw new DOMException("TEST browser storage unavailable","QuotaExceededError");return original.call(this,key,value);};
    (window as unknown as {restoreTestBuilderStorage:()=>void}).restoreTestBuilderStorage=()=>{Storage.prototype.setItem=original;};
  });
  await card.getByRole("button",{name:"Review Builder authorization",exact:true}).click();
  await card.getByRole("checkbox",{name:"Allow this bounded Builder request; actual cost is unknown"}).check();
  let blockedReads=false, builderPosts=0;
  const operationRoute=`**${state.taskRoot}/builder-operations*`;
  if(lostAck)await page.route(operationRoute,async route=>{
    if(route.request().method()==="POST") {
      builderPosts++;
      const result=await fetchWithinMutationLimit(route);
      expect(result.ok(),await result.text()).toBe(true);
      blockedReads=true;await route.abort("failed");
    } else if(blockedReads)await route.abort("failed");
    else await route.continue();
  });
  await card.getByRole("button",{name:"Build Pipeline Draft",exact:true}).click();
  if(transport==="storage_blocked"){
    await expect(card.getByRole("alert")).toContainText("No model call was started");
    expect(await read(request,`${state.taskRoot}/calls`)).toEqual(callsBefore);
    expect((await read(request,`${state.taskRoot}/builder-operations?image_class_review_id=${group.review.id}`)).items).toHaveLength(0);
    await page.evaluate(()=>(window as unknown as {restoreTestBuilderStorage:()=>void}).restoreTestBuilderStorage());
    await card.getByRole("button",{name:"Build Pipeline Draft",exact:true}).click();
  }
  if(lostAck){
    await expect(card.getByRole("button",{name:"Retry the same Builder request",exact:true})).toBeVisible();
    expect(await page.evaluate(()=>Object.keys(sessionStorage).some(key=>key.startsWith("annotagent.builder-pending:")))).toBe(true);
    await page.unroute(operationRoute);await page.reload();
    expect(builderPosts).toBe(1);
  }
  await expect(card.getByRole("button",{name:"Review sample authorization",exact:true})).toBeVisible();
  const operations=await read(request,`${state.taskRoot}/builder-operations?image_class_review_id=${group.review.id}`);
  expect(operations.items).toHaveLength(1);
  expect(operations.items[0].operation.status).toBe("completed");
  expect(operations.items[0].operation.evidence.repair_source.reference.review_id).toBe(group.review.id);
  expect(operations.items[0].session.working_draft.draft_id).toBe(applied.repair_draft_id);
  const callsAfterBuilder=await read(request,`${state.taskRoot}/calls`);
  await page.reload();
  await expect(card.getByRole("button",{name:"Review sample authorization",exact:true})).toBeVisible();
  expect(await read(request,`${state.taskRoot}/calls`)).toEqual(callsAfterBuilder);
  await card.getByRole("button",{name:"Review sample authorization",exact:true}).click();
  await card.getByRole("checkbox",{name:"Allow these sample images to be sent to the listed models; actual cost is unknown"}).check();
  await card.getByRole("button",{name:"Test these samples",exact:true}).click();
  await expect(card.getByRole("button",{name:"View sample results in canvas",exact:true})).toBeVisible();
  await card.getByRole("button",{name:"View sample results in canvas",exact:true}).click();
  await expect(page).toHaveURL(new RegExp(`draft=${applied.repair_draft_id}`));
  await expect(page).toHaveURL(new RegExp(`image=${state.image.image_id}`));
  const url=page.url(), settledCalls=await read(request,`${state.taskRoot}/calls`);
  await page.reload();await expect(page).toHaveURL(url);
  await expect(card.getByRole("button",{name:"View sample results in canvas",exact:true})).toBeVisible();
  expect(await read(request,`${state.taskRoot}/calls`)).toEqual(settledCalls);
  const detailsLink=card.getByRole("link",{name:"Open saved Pipeline details",exact:true});
  await expect(detailsLink).toBeVisible();
  await detailsLink.click();
  await expect(page.getByRole("button",{name:"Back to annotation workspace",exact:true})).toBeVisible();
  await expect(page).toHaveURL(/workspace_return=/);
  await page.reload();
  await page.getByRole("button",{name:"Back to annotation workspace",exact:true}).click();
  await expect(page).toHaveURL(url);
  await expect(card.getByRole("button",{name:"View sample results in canvas",exact:true})).toBeVisible();
  expect(await read(request,`${state.taskRoot}/calls`)).toEqual(settledCalls);
  if(transport==="normal"){await card.getByRole("button",{name:"View sample results in canvas",exact:true}).scrollIntoViewIfNeeded();await page.screenshot({path:resolve(`../docs/execution/conversational-workspace/image-class-builder-${bbox ? "continuation" : "classification"}.png`),fullPage:true});}
  else {await page.setViewportSize({width:390,height:844});await card.getByRole("button",{name:"View sample results in canvas",exact:true}).scrollIntoViewIfNeeded();await expect(card.getByRole("button",{name:"View sample results in canvas",exact:true})).toBeVisible();await expect(page).toHaveURL(url);expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1)).toBe(true);}
});
}
}

const feedbackPath = (state: State, image = state.image.image_id) => `/api/workflow-sample-tests/${state.record.id}/images/${image}/feedback`;
async function unchanged(request: APIRequestContext, state: State) {
  const drafts = (await read(request, `/api/workflow-drafts?project_id=${state.project}`)).drafts;
  return {
    original: await read(request, `/api/workflow-drafts/${state.record.draft_id}/sample-test?test_id=${state.record.id}`),
    sourceDraft: drafts.find((value: any) => value.id === state.record.draft_id),
    otherImage: await read(request, feedbackPath(state, state.other.image_id)),
    goal: await read(request, `/api/projects/${state.project}/goal`),
    calls: await read(request, `${state.taskRoot}/calls`), budget: await read(request, `${state.taskRoot}/budget`),
    runs: await read(request, `/api/runs?project_id=${state.project}`), export: await read(request, `/api/projects/${state.project}/export-readiness`),
  };
}
async function previewGroup(request: APIRequestContext, state: State) {
  return read(request, `${state.path}/image-class-preview?${new URLSearchParams({ target_label: state.target })}`);
}
async function openGroup(request: APIRequestContext, state: State) {
  const preview = await previewGroup(request, state), input = { id: randomUUID(), feedback_call_id: state.consent.call_id, target_label: state.target, expected_scope_digest: preview.scope_digest };
  const review = await post(request, `${state.path}/image-class`, input);
  return { preview, input, review, endpoint: `${state.taskRoot}/image-class-reviews/${input.id}` };
}

// TEST transport on 8796 and a disposable 8791 workspace only. These checks
// concern scoped human changes, never Live model quality or formal annotations.
test("a saved current-image class scope offers an explicit review entry without applying changes", async ({ page, request }) => {
  test.setTimeout(120_000);
  const state = await clarification(request, page);
  const input = { command_id: randomUUID(), expected_context_digest: state.status.scope_context_digest, choice: { scope: "current_image_class" } };
  const answer = await request.post(`${state.path}/scope-answer`, { data: input });
  expect(answer.ok(), await answer.text()).toBe(true);
  const calls = await (await request.get(`${state.taskRoot}/calls`)).json();
  const feedbackPath = `/api/workflow-sample-tests/${state.record.id}/images/${state.image.image_id}/feedback`;
  const before = await (await request.get(feedbackPath)).json();
  await page.goto(state.url);
  const card = page.getByRole("region", { name: "Feedback on selected candidate", exact: true });
  await expect(card.getByText("Scope answer saved", { exact: true })).toBeVisible();
  await expect(card.getByRole("button", { name: "Review this class in the current image", exact: true })).toBeVisible();
  expect(await (await request.get(`${state.taskRoot}/calls`)).json()).toEqual(calls);
  expect(await (await request.get(feedbackPath)).json()).toEqual(before);
});

for (const bbox of [true, false]) {
test(`image class ${bbox ? "bbox group" : "human multi-label classification baseline"} applies one atomic correction without touching another class or image`, async ({ page, request }) => {
  test.setTimeout(240_000);
  const state = await imageClassSample(request, page, bbox), before = await unchanged(request, state);
  const revisionsBefore = await read(request, feedbackPath(state));
  const draftsBefore = (await read(request, `/api/workflow-drafts?project_id=${state.project}`)).drafts;
  if (!bbox) {
    expect((await request.get(`${state.path}/image-class-preview`)).ok()).toBe(false);
    expect((await request.get(`${state.path}/image-class-preview?target_label=${encodeURIComponent("室")}`)).ok()).toBe(false);
  }
  const preview = await previewGroup(request, state);
  expect(preview.scope.target_label).toBe(state.target);
  expect(preview.scope.sample_test_id).toBe(state.record.id); expect(preview.scope.image_id).toBe(state.image.image_id);
  expect(preview.scope.members).toHaveLength(bbox ? 2 : 1);
  expect(preview.scope.baseline_feedback).toEqual(revisionsBefore.revisions);
  expect(await read(request, `${state.path}/image-class`)).toBeNull();
  expect(await unchanged(request, state)).toEqual(before);
  const { review, input, endpoint } = await openGroup(request, state);
  expect(review.status).toBe("pending"); expect(review.revisions).toEqual([]); expect(review.repair_draft_id).toBeNull();
  expect(await read(request, feedbackPath(state))).toEqual(revisionsBefore);
  expect(await post(request, `${state.path}/image-class`, input)).toEqual(review);
  const keys = review.scope.members.map((member: any) => ({ outcome_id: member.outcome.id, source_artifact_id: member.source_artifact_id }));
  const corrected = { kind: "bounding_box", rect: [0.14, 0.22, 0.12, 0.18] };
  const actions = bbox ? [{ action: "edit", ...keys[0], corrected_value: corrected, corrected_label: state.target }, { action: "exclude", ...keys[1] }] : [{ action: "exclude", ...keys[0] }];
  const answer = { command_id: randomUUID(), expected_scope_digest: review.scope_digest, actions };
  const saved = await post(request, `${endpoint}/answer`, answer);
  expect(saved.status).toBe("applied"); expect(saved.answer).toEqual(answer); expect(saved.repair_draft_id).toBeTruthy();
  expect(saved.revisions).toHaveLength(actions.length);
  if (bbox) {
    expect(saved.revisions.find((value: any) => value.outcome_id === keys[0].outcome_id).corrected_value).toEqual(corrected);
    expect(saved.revisions.find((value: any) => value.outcome_id === keys[1].outcome_id).reason).toBe("exclude_target");
    for (const other of state.candidates.filter((value: any) => value.outcome.label !== "cup")) expect(saved.revisions.some((value: any) => value.outcome_id === other.outcome.id)).toBe(false);
  } else {
    expect(saved.revisions[0].corrected_value).toEqual({ kind: "classification", labels: ["室内运动场"] });
    expect(saved.revisions[0].reason).not.toBe("exclude_target");
    expect(saved.scope.members[0].outcome.value.labels).toEqual(["室内"]); // Original prediction is not rewritten.
  }
  const feedback = await read(request, feedbackPath(state));
  expect(typedFeedback(feedback.revisions)).toEqual(typedFeedback([...revisionsBefore.revisions, ...saved.revisions]));
  expect(await unchanged(request, state)).toEqual(before);
  const drafts = (await read(request, `/api/workflow-drafts?project_id=${state.project}`)).drafts;
  expect(drafts).toHaveLength(draftsBefore.length + 1);
  expect(drafts.some((value: any) => value.id === saved.repair_draft_id)).toBe(true);
  expect(await post(request, `${endpoint}/answer`, answer)).toEqual(saved);
  expect(await post(request, `${endpoint}/resume`, {})).toEqual(saved);
  expect((await read(request, `/api/workflow-drafts?project_id=${state.project}`)).drafts).toEqual(drafts);
  expect(await unchanged(request, state)).toEqual(before); expect(await read(request, feedbackPath(state))).toEqual(feedback);
  expect((await request.post(`${endpoint}/answer`, { data: { ...answer, command_id: randomUUID() } })).ok()).toBe(false);
});
}

test("image class review refuses foreign, widened and stale actions without partial feedback or draft creation", async ({ page, request }) => {
  test.setTimeout(240_000);
  const state = await imageClassSample(request, page, true), before = await unchanged(request, state);
  const preview = await previewGroup(request, state), input = { id: randomUUID(), feedback_call_id: state.consent.call_id, target_label: "cup", expected_scope_digest: preview.scope_digest };
  expect((await page.request.post(`${state.path}/image-class`, { data: input })).status()).toBe(403);
  expect((await request.post(`${state.path}/image-class`, { data: input, headers: { Origin: "https://foreign.invalid" } })).status()).toBe(403);
  expect((await request.post(`${state.path}/image-class`, { data: { ...input, image_id: state.other.image_id } })).status()).toBe(422);
  expect((await request.post(`${state.path}/image-class`, { data: { ...input, expected_scope_digest: "f".repeat(64) } })).ok()).toBe(false);
  const foreignMessage = randomUUID(), foreignTask = randomUUID();
  await post(request, `${state.root}/messages`, { id: foreignMessage, text: "TEST different task has no authority over this class review", image: null });
  await post(request, `${state.root}/tasks`, { id: foreignTask, source_message_id: foreignMessage, schema_revision: before.goal.revision });
  expect((await request.get(`${state.path.replace(state.task, foreignTask)}/image-class-preview?target_label=cup`)).ok()).toBe(false);
  expect((await request.post(`${state.path.replace(state.task, foreignTask)}/image-class`, { data: input })).ok()).toBe(false);
  const review = await post(request, `${state.path}/image-class`, input), endpoint = `${state.taskRoot}/image-class-reviews/${input.id}`;
  expect((await request.post(`${state.path}/image-class`, { data: { ...input, id: randomUUID() } })).ok()).toBe(false);
  const actions = review.scope.members.map((member: any) => ({ action: "exclude", outcome_id: member.outcome.id, source_artifact_id: member.source_artifact_id }));
  const command = { command_id: randomUUID(), expected_scope_digest: review.scope_digest, actions };
  const foreign = state.candidates.find((value: any) => value.outcome.label === "bottle");
  const revisions = await read(request, feedbackPath(state)), drafts = await read(request, `/api/workflow-drafts?project_id=${state.project}`);
  for (const changed of [
    { ...command, expected_scope_digest: "e".repeat(64) },
    { ...command, actions: actions.slice(0, 1) },
    { ...command, actions: [actions[0], actions[0]] },
    { ...command, actions: [actions[0], { ...actions[1], outcome_id: foreign.outcome.id, source_artifact_id: foreign.source_artifact_id }] },
    { ...command, actions: [actions[0], { ...actions[1], source_artifact_id: randomUUID() }] },
    { ...command, actions: [{ ...actions[0], action: "replace_label", replacement: "bottle" }, actions[1]] },
  ]) expect((await request.post(`${endpoint}/answer`, { data: changed })).ok()).toBe(false);
  expect((await request.post(`${endpoint}/answer`, { data: { ...command, publish: true } })).status()).toBe(422);
  expect(await read(request, endpoint)).toEqual(review);
  expect(await read(request, feedbackPath(state))).toEqual(revisions); expect(await read(request, `/api/workflow-drafts?project_id=${state.project}`)).toEqual(drafts);
  expect(await unchanged(request, state)).toEqual(before);
  // Real competing write on another candidate advances the image-wide revision.
  // The group must reject atomically, not save its first cup then fail halfway.
  await post(request, feedbackPath(state), { revision_id: randomUUID(), sample_test_id: state.record.id, image_id: state.image.image_id, sequence: review.scope.baseline_sequence + 1, reason: "cannot_judge", outcome_id: foreign.outcome.id, corrected_value: null, corrected_label: null, note: "TEST competing browser image revision", created_at: new Date().toISOString() });
  const competing = await read(request, feedbackPath(state));
  expect((await request.post(`${endpoint}/answer`, { data: command })).ok()).toBe(false);
  expect(await read(request, feedbackPath(state))).toEqual(competing); expect(await read(request, endpoint)).toEqual(review);
  expect(await read(request, `/api/workflow-drafts?project_id=${state.project}`)).toEqual(drafts);
  expect(await unchanged(request, state)).toEqual(before);
  const cancelled = await post(request, `${endpoint}/cancel`, {}); expect(cancelled.status).toBe("cancelled");
  expect((await request.post(`${endpoint}/answer`, { data: command })).ok()).toBe(false);
  expect(await read(request, feedbackPath(state))).toEqual(competing);
});

for (const bbox of [true, false]) {
test(`image class ${bbox ? "bbox" : "human multi-label classification"} canvas saves one scoped batch and restores it without hidden execution`, async ({ page, request }) => {
  test.setTimeout(240_000);
  const state = await imageClassSample(request, page, bbox), before = await unchanged(request, state);
  const draftCount = (await read(request, `/api/workflow-drafts?project_id=${state.project}`)).drafts.length;
  await page.goto(state.url);
  const card = page.getByRole("region", { name: "Review this class in the current image", exact: true });
  const writes: { path: string; input: any }[] = [];
  page.on("request", value => { if (value.method() === "POST") writes.push({ path: new URL(value.url()).pathname, input: value.postDataJSON() }); });
  const prepare = card.getByRole("button", { name: "Review this class in the current image", exact: true });
  if (!bbox) {
    await expect(prepare).toBeDisabled();
    const choice = card.getByLabel("Class to review", { exact: true });
    await expect(choice.locator("option")).toHaveText(["Choose one original class", "室内", "室内运动场"]);
    await choice.selectOption("室内");
  }
  await prepare.click();
  const scope = card.getByRole("region", { name: "Image-class review scope", exact: true });
  await expect(scope).toContainText(state.image.image_id); await expect(scope).toContainText(state.target);
  expect(writes).toEqual([]); expect(await read(request, `${state.path}/image-class`)).toBeNull();
  await scope.getByRole("button", { name: "Open image-class review", exact: true }).click();
  const canvas = page.getByRole("region", { name: "Image-class review", exact: true });
  await expect(canvas).toBeVisible();
  const review = await read(request, `${state.path}/image-class`), endpoint = `${state.taskRoot}/image-class-reviews/${review.id}`;
  expect(new URL(page.url()).searchParams.get("class_review")).toBe(review.id);
  expect(new URL(page.url()).searchParams.get("image")).toBe(state.image.image_id);
  expect(writes).toHaveLength(1); expect(writes[0].input.id).toBe(review.id);
  const first = review.scope.members[0].outcome.id;
  await canvas.getByLabel("Candidate in this class", { exact: true }).selectOption(first);
  if (bbox) {
    // A temporarily invalid coordinate remains the user's value when another
    // field changes; the form must not silently reset it to the prediction.
    await canvas.getByRole("spinbutton", { name: "width", exact: true }).fill("0");
    await expect(canvas.getByRole("button", { name: "Save all class decisions", exact: true })).toBeDisabled();
    await canvas.getByRole("spinbutton", { name: "x", exact: true }).fill("0.14");
    await expect(canvas.getByRole("spinbutton", { name: "width", exact: true })).toHaveValue("0");
    await canvas.getByRole("spinbutton", { name: "width", exact: true }).fill("0.123456789");
    await expect(canvas.getByRole("spinbutton", { name: "x", exact: true })).toHaveValue("0.14");
    await canvas.getByLabel("Candidate in this class", { exact: true }).selectOption(review.scope.members[1].outcome.id);
    await canvas.getByRole("button", { name: "Exclude selected target", exact: true }).click();
  } else await canvas.getByLabel("Replacement class", { exact: true }).fill("室外");
  const beforeReload = writes.length;
  page.once("dialog", dialog => dialog.accept());
  await page.reload();
  await expect(canvas).toBeVisible(); expect(writes).toHaveLength(beforeReload);
  if (bbox) {
    await canvas.getByLabel("Candidate in this class", { exact: true }).selectOption(first);
    await expect(canvas.getByRole("spinbutton", { name: "width", exact: true })).toHaveValue("0.123456789");
  } else await expect(canvas.getByLabel("Replacement class", { exact: true })).toHaveValue("室外");
  // Measure the rendered SVG, not an unmatched CSS rule or a screenshot's
  // empty container. The narrow screen supports browsing and explicit save.
  await page.setViewportSize({ width: 1280, height: 1800 });
  await expect.poll(async () => (await canvas.locator("svg.annotation-canvas").boundingBox())?.height).toBe(650);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("navigation", { name: "Workspace panels", exact: true }).getByRole("button", { name: "Images (2)", exact: true }).click();
  await expect.poll(async () => (await canvas.locator("svg.annotation-canvas").boundingBox())?.height).toBe(300);
  await canvas.getByRole("button", { name: "Save all class decisions", exact: true }).scrollIntoViewIfNeeded();
  await expect(canvas.getByRole("button", { name: "Save all class decisions", exact: true })).toBeInViewport();
  expect(new URL(page.url()).searchParams.get("image")).toBe(state.image.image_id); expect(writes).toHaveLength(beforeReload);
  await page.setViewportSize({ width: 1280, height: 1800 });
  await expect(canvas.getByLabel("Candidate in this class", { exact: true })).toHaveValue(first);
  if (bbox) await expect(canvas.getByRole("spinbutton", { name: "width", exact: true })).toHaveValue("0.123456789");
  else await expect(canvas.getByLabel("Replacement class", { exact: true })).toHaveValue("室外");
  await canvas.evaluate(element => element.scrollIntoView({ block: "center" }));
  await expect(canvas.getByRole("button", { name: "Save all class decisions", exact: true })).toBeInViewport();
  await page.screenshot({ path: `../docs/execution/conversational-workspace/image-class-${bbox ? "bbox" : "classification"}-review.png`, animations: "disabled" });
  if (bbox) {
    let aborted = false;
    await page.route(`**${endpoint}/answer`, async route => { aborted = true; await route.abort("failed"); }, { times: 1 });
    await canvas.getByRole("button", { name: "Save all class decisions", exact: true }).click();
    await expect.poll(() => aborted).toBe(true);
    await expect(canvas.getByRole("button", { name: "Retry same class decisions", exact: true })).toBeEnabled();
    expect((await read(request, endpoint)).answer).toBeNull();
    page.once("dialog", dialog => dialog.accept());
    await page.reload();
    await expect(canvas.getByRole("button", { name: "Retry same class decisions", exact: true })).toBeEnabled();
    await page.route(`**${endpoint}/answer`, async route => { const response = await fetchWithinMutationLimit(route); expect(response.ok(), await response.text()).toBe(true); await route.abort("failed"); }, { times: 1 });
    await canvas.getByRole("button", { name: "Retry same class decisions", exact: true }).click();
  } else await canvas.getByRole("button", { name: "Save all class decisions", exact: true }).click();
  await expect(canvas.getByText("Image-class decisions saved", { exact: true })).toBeVisible();
  await expect(canvas.getByRole("region", { name: "Compare conflicting class decisions", exact: true })).toHaveCount(0);
  const applied = await read(request, endpoint); expect(applied.status).toBe("applied"); expect(applied.repair_draft_id).toBeTruthy();
  const answers = writes.filter(value => value.path.endsWith("/answer")); expect(answers).toHaveLength(bbox ? 2 : 1);
  if (bbox) expect(answers[1].input).toEqual(answers[0].input);
  expect({ ...applied.answer, actions: typedFeedback(applied.answer.actions) }).toEqual({ ...answers[0].input, actions: typedFeedback(answers[0].input.actions) });
  if (bbox) expect(Math.fround(applied.answer.actions[0].corrected_value.rect[2])).toBe(Math.fround(0.123456789));
  expect((await read(request, `/api/workflow-drafts?project_id=${state.project}`)).drafts).toHaveLength(draftCount + 1);
  expect(await unchanged(request, state)).toEqual(before);
  if (!bbox) expect(applied.revisions[0].corrected_value.labels).toEqual(["室外", "室内运动场"]);
  const savedWrites = writes.length;
  await page.reload();
  await expect(canvas.getByText("Image-class decisions saved", { exact: true })).toBeVisible();
  await expect(canvas.getByRole("region", { name: "Compare conflicting class decisions", exact: true })).toHaveCount(0);
  expect(writes).toHaveLength(savedWrites);
  if (bbox) {
    await canvas.getByRole("button", { name: "Inspect revision Draft", exact: true }).click();
    await expect.poll(() => new URL(page.url()).searchParams.get("draft")).toBe(applied.repair_draft_id);
    await expect(page.getByRole("button", { name: "Return to image-class review", exact: true })).toBeVisible();
    await page.reload();
    await page.getByRole("button", { name: "Return to image-class review", exact: true }).click();
    await expect(canvas.getByText("Image-class decisions saved", { exact: true })).toBeVisible();
    const restored = new URL(page.url()).searchParams;
    expect(restored.get("class_review")).toBe(review.id); expect(restored.get("image")).toBe(state.image.image_id);
    expect(restored.get("draft")).toBe(state.record.draft_id); expect(restored.get("test")).toBe(state.record.id);
    await expect(canvas.getByLabel("Candidate in this class", { exact: true })).toHaveValue(first);
    expect(writes).toHaveLength(savedWrites);
  }
  await canvas.getByRole("button", { name: "Return to sample results", exact: true }).click();
  await expect(page.getByRole("region", { name: "Saved sample results", exact: true })).toBeVisible();
  expect(new URL(page.url()).searchParams.get("class_review")).toBeNull();
  expect(new URL(page.url()).searchParams.get("image")).toBe(state.image.image_id);
  if (bbox) await expect(page.locator(".conversation-sample-canvas .annotation-shape rect.aa-annotation-shape")).toHaveCount(3);
  else await expect(page.getByLabel("Image classification results", { exact: true })).toContainText("室内运动场");
  expect(writes).toHaveLength(savedWrites); expect(await unchanged(request, state)).toEqual(before);
});
}
