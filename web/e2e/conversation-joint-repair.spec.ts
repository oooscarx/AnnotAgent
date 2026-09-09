import { randomUUID } from "node:crypto";
import { test, expect } from "./fixtures";
import { sample } from "./conversation-feedback-helpers";
import { isolatedEvidencePath } from "./evidence";
import type { Locator } from "@playwright/test";

async function reviewExactModel(card: Locator, modelName: string) {
  await card.getByRole("button", {name:"Review build and sample authorization",exact:true}).click();
  const choices = card.locator("details").filter({has:card.locator("summary").filter({hasText:"Allowed image models"})});
  await expect(choices).toBeVisible();
  if (!(await choices.evaluate(element => (element as HTMLDetailsElement).open))) await choices.locator("summary").click();
  // A full suite can have more than the 32-model authorization ceiling. Select
  // this scenario's exact fixture, never assume a small registry or widen scope.
  for (const checkbox of await choices.getByRole("checkbox").all()) {
    if (await checkbox.isChecked()) await checkbox.uncheck();
  }
  await choices.getByRole("checkbox", {name:modelName,exact:true}).check();
  await card.getByRole("button", {name:"Review build and sample authorization",exact:true}).click();
}

test("preauthorized pending correction waits without inference and the saved answer resumes one joint repair", async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await sample(request, page, "pending-joint-repair", true);
  const help = state.savedRequests[0];
  const original = await (await request.get(`/api/workflow-drafts/${state.record.draft_id}/sample-test?test_id=${state.record.id}`)).json();
  const query = new URLSearchParams({ consent_id: randomUUID(), builder_operation_id: randomUUID(), sample_operation_id: randomUUID(), schema_id: original.annotation_schema.schema_draft_id, schema_revision: String(original.annotation_schema.revision), planner_model_id: state.model.id, pending_request_id: help.input.id, allowed_models: JSON.stringify([`model-profile:${state.model.id}`]) });
  const preview = await request.get(`${state.taskRoot}/journey-preview?${query}`);
  expect(preview.ok(), await preview.text()).toBe(true);
  const consent = { ...(await preview.json()).consent, allow_unknown_cost: true };
  expect(consent.repair_after_answer).toEqual(help.input);
  const saved = await request.post(`${state.taskRoot}/journey-consents`, { data: consent });
  expect(saved.ok(), await saved.text()).toBe(true);
  const execution = `${state.taskRoot}/journey-consents/${consent.id}/execution`;
  const calls = await (await request.get(`${state.taskRoot}/calls`)).json();
  const waiting = await request.post(execution, { data: {} });
  expect(waiting.ok(), await waiting.text()).toBe(true);
  expect((await waiting.json()).dispatch).toBeNull();
  expect(await (await request.get(`${state.taskRoot}/calls`)).json()).toEqual(calls);
  // Exercise the existing real canvas answer with the new server contract.
  // The visible preauthorization control is a separate UI integration slice.
  let answerBody: any;
  await page.route(`**${state.taskRoot}/human-requests/${help.input.id}/answer`, async route => {
    answerBody = { ...route.request().postDataJSON(), journey_consent_id: consent.id };
    await route.continue({ postData: JSON.stringify(answerBody) });
  });
  await page.goto(`${state.url}&request=${help.input.id}`);
  await page.getByLabel("Correct label", { exact: true }).fill("cup");
  await page.getByRole("spinbutton", { name: "width", exact: true }).fill("0.12");
  const answerResponse = page.waitForResponse(response => response.url().endsWith(`/human-requests/${help.input.id}/answer`) && response.request().method() === "POST");
  await page.getByRole("button", { name: /^Submit correction( and continue)?$/ }).click();
  const answered = await answerResponse;
  expect(answered.ok(), await answered.text()).toBe(true);
  const receipt = await answered.json();
  expect(receipt.status).toBe("applied");
  expect(receipt.journey_resume.error).toBeUndefined();
  await expect.poll(async () => (await (await request.get(execution)).json()).sample?.status, { timeout: 75_000 }).toBe("succeeded");
  const finished = await (await request.get(execution)).json();
  expect(finished.record.resolved_consent.repair.draft_id).toBe(receipt.resume_draft_id);
  expect(finished.sample.draft_id).toBe(receipt.resume_draft_id);
  const finalCalls = await (await request.get(`${state.taskRoot}/calls`)).json();
  const duplicate = await request.post(`${state.taskRoot}/human-requests/${help.input.id}/answer`, { data: answerBody });
  expect(duplicate.ok(), await duplicate.text()).toBe(true);
  expect(await (await request.get(`${state.taskRoot}/calls`)).json()).toEqual(finalCalls);
  const resumed = await request.post(`${state.taskRoot}/human-requests/${help.input.id}/resume`, {data:{}});
  expect(resumed.ok(), await resumed.text()).toBe(true);
  const resumedReceipt = await resumed.json();
  expect(resumedReceipt.journey_resume.consent_id).toBe(consent.id);
  expect(resumedReceipt.journey_resume.error).toBeUndefined();
  expect(await (await request.get(`${state.taskRoot}/calls`)).json()).toEqual(finalCalls);
  await page.reload();
  expect(await (await request.get(`${state.taskRoot}/calls`)).json()).toEqual(finalCalls);
});

for (const retryAdmission of [false,true]) test(`pending request UI restores authorization and submits a real correction without request interception${retryAdmission ? " after admission failure" : ""}`, async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await sample(request, page, `pending-repair-ui-${retryAdmission}`, true);
  const help = state.savedRequests[0];
  await page.goto(`${state.url}&request=${help.input.id}`);
  const pendingCard = page.getByRole("region", { name: "Build and test annotation plan", exact: true }).filter({has:page.getByRole("heading",{name:"Continue after your correction",exact:true})});
  await reviewExactModel(pendingCard, state.model.display_name);
  const panel = pendingCard.getByLabel("Build and sample authorization",{exact:true});
  await expect(panel).toContainText("does not authorize future corrections");
  await panel.getByRole("checkbox",{name:/Allow this plan and sample test/}).check();
  const consentResponse = page.waitForResponse(response=>response.request().method()==="POST"&&response.url().endsWith(`${state.taskRoot}/journey-consents`));
  await panel.getByRole("button",{name:"Authorize continuation after this answer",exact:true}).click();
  const consent = (await (await consentResponse).json()).consent;
  await expect(pendingCard).toContainText("waiting for your correction");
  const calls = await (await request.get(`${state.taskRoot}/calls`)).json();
  await page.reload();
  await expect(pendingCard).toContainText("waiting for your correction");
  expect(await (await request.get(`${state.taskRoot}/calls`)).json()).toEqual(calls);
  await pendingCard.getByRole("button",{name:"Revoke continuation permission",exact:true}).scrollIntoViewIfNeeded();
  await page.getByRole("region",{name:"Project images",exact:true}).evaluate(element=>element.scrollTo(0,0));
  await page.screenshot({path:isolatedEvidencePath("../docs/execution/conversational-workspace/pending-answer-authorization.png"),fullPage:true});
  await page.getByLabel("Correct label",{exact:true}).fill("cup");
  await page.getByRole("spinbutton",{name:"width",exact:true}).fill("0.12");
  if(retryAdmission)expect((await request.patch(`/api/model-profiles/${state.model.id}`,{data:{enabled:false}})).ok()).toBe(true);
  const answerResponse = page.waitForResponse(response=>response.request().method()==="POST"&&response.url().endsWith(`/human-requests/${help.input.id}/answer`));
  await page.getByRole("button",{name:"Submit correction and continue",exact:true}).click();
  const answer = await answerResponse;
  expect(answer.request().postDataJSON().journey_consent_id).toBe(consent.id);
  const answered=await answer.json();
  expect(answered.status).toBe("applied");
  if(retryAdmission){
    expect( answered.journey_resume.error).toBeTruthy();
    const execution=`${state.taskRoot}/journey-consents/${consent.id}/execution`;
    const failed=await (await request.get(execution)).json();
    expect(failed.answer_delivery.status).toBe("failed");
    expect(failed.answer_delivery.error).toBeTruthy();
    expect(failed.dispatch).toBeNull();
    expect(await (await request.get(`${state.taskRoot}/calls`)).json()).toEqual(calls);
    await page.reload();
    await expect(page.getByRole("alert").filter({hasText:"Correction saved; continuation needs attention:"})).toBeVisible();
    expect((await request.patch(`/api/model-profiles/${state.model.id}`,{data:{enabled:true}})).ok()).toBe(true);
    const retry=await request.post(`${state.taskRoot}/human-requests/${help.input.id}/resume`,{data:{}});
    expect(retry.ok(),await retry.text()).toBe(true);
    // Disabling/re-enabling changes the Model Profile revision. Availability
    // alone cannot restore an old grant for a different frozen binding.
    const retried=await retry.json();
    expect(retried.status).toBe("applied");
    expect(retried.journey_resume.consent_id).toBe(consent.id);
    expect(retried.journey_resume.error).toBeTruthy();
    expect(await (await request.get(`${state.taskRoot}/calls`)).json()).toEqual(calls);
    await page.reload();
    await expect(page.getByRole("alert").filter({hasText:"Correction saved; continuation needs attention:"})).toBeVisible();
    return;
  }else expect(answered.journey_resume.error).toBeUndefined();
  const repair = page.getByRole("region",{name:"Repair annotation pipeline",exact:true});
  await expect(repair.getByText("Sample results saved",{exact:true})).toBeVisible({timeout:75_000});
  await repair.getByRole("button",{name:"View sample results in canvas",exact:true}).click();
  await expect(page.getByText("Saved workspace loaded",{exact:true})).toBeVisible();
  await expect(page.getByText("Checking whether this sample still matches the current plan...",{exact:true})).toBeHidden();
  await page.getByRole("region",{name:"Project images",exact:true}).evaluate(element=>element.scrollTo(0,0));
  await page.screenshot({path:isolatedEvidencePath("../docs/execution/conversational-workspace/pending-answer-result.png"),fullPage:true});
});

test("one explicit repair consent preserves its exact correction and continues through Builder and sample", async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await sample(request, page, "joint-repair", true);
  const help = state.savedRequests[0];
  expect(help).toBeTruthy();
  await page.goto(`${state.url}&request=${help.input.id}`);
  await page.getByLabel("Correct label", { exact: true }).fill("cup");
  await page.getByRole("spinbutton", { name: "width", exact: true }).fill("0.12");
  await page.getByRole("button", { name: "Submit correction", exact: true }).click();
  await expect.poll(async () => (await (await request.get(`${state.taskRoot}/human-requests`)).json()).find((value:any) => value.input.id === help.input.id)?.status, { timeout: 75_000 }).toBe("applied");
  const corrected = (await (await request.get(`${state.taskRoot}/human-requests`)).json()).find((value:any) => value.input.id === help.input.id);
  const original = await (await request.get(`/api/workflow-drafts/${state.record.draft_id}/sample-test?test_id=${state.record.id}`)).json();
  const query = new URLSearchParams({ consent_id: randomUUID(), builder_operation_id: randomUUID(), sample_operation_id: randomUUID(), schema_id: original.annotation_schema.schema_draft_id, schema_revision: String(original.annotation_schema.revision), planner_model_id: state.model.id, repair_request_id: help.input.id, allowed_models: JSON.stringify([`model-profile:${state.model.id}`]) });
  const preview = await request.get(`${state.taskRoot}/journey-preview?${query}`);
  expect(preview.ok(), await preview.text()).toBe(true);
  const value = await preview.json();
  expect(value.consent.repair.request_id).toBe(help.input.id);
  expect(value.consent.repair.draft_id).toBe(corrected.resume_draft_id);
  expect(value.consent.repair.content_hash).toMatch(/^[a-f0-9]{64}$/);
  const consent = { ...value.consent, allow_unknown_cost: true };
  const beforeCalls = await (await request.get(`${state.taskRoot}/calls`)).json();
  const saved = await request.post(`${state.taskRoot}/journey-consents`, { data: consent });
  expect(saved.ok(), await saved.text()).toBe(true);
  expect((await request.post(`${state.taskRoot}/journey-consents`, { data: { ...consent, repair: { ...consent.repair, revision: consent.repair.revision + 1 } } })).ok()).toBe(false);
  const execution = `${state.taskRoot}/journey-consents/${consent.id}/execution`;
  await request.get(execution);
  expect(await (await request.get(`${state.taskRoot}/calls`)).json()).toEqual(beforeCalls);
  const started = await request.post(execution, { data: {} });
  expect(started.ok(), await started.text()).toBe(true);
  await expect.poll(async () => (await (await request.get(execution)).json()).sample?.status, { timeout: 75_000 }).toBe("succeeded");
  const completed = await (await request.get(execution)).json();
  expect(completed.record.consent.repair).toEqual(consent.repair);
  expect(completed.sample.id).toBe(consent.sample_operation_id);
  expect(completed.sample.draft_id).toBe(corrected.resume_draft_id);
  expect(completed.builder.evidence.draft_id).toBe(corrected.resume_draft_id);
  const calls = await (await request.get(`${state.taskRoot}/calls`)).json();
  expect((await request.post(execution, { data: {} })).ok()).toBe(true);
  expect(await (await request.get(`${state.taskRoot}/calls`)).json()).toEqual(calls);
});

test("the correction card authorizes repair and sample together and restores only its own results", async ({ page, request }) => {
  test.setTimeout(180_000);
  const state = await sample(request, page, "joint-repair-ui", true);
  const help = state.savedRequests[0];
  await page.goto(`${state.url}&request=${help.input.id}`);
  await page.getByLabel("Correct label", { exact: true }).fill("cup");
  await page.getByRole("spinbutton", { name: "width", exact: true }).fill("0.12");
  await page.getByRole("button", { name: "Submit correction", exact: true }).click();
  const repair = page.getByRole("region", { name: "Repair annotation pipeline", exact: true });
  const joint = repair.getByRole("region", { name: "Build and test annotation plan", exact: true });
  await reviewExactModel(joint, state.model.display_name);
  const consentPanel = joint.getByLabel("Build and sample authorization", { exact: true });
  await expect(consentPanel).toContainText("not future corrections");
  await expect(consentPanel).toContainText("Cost unknown");
  const posts: string[] = [];
  page.on("request", req => { if (req.method() === "POST") posts.push(new URL(req.url()).pathname); });
  const acknowledged = page.waitForResponse(response => response.request().method() === "POST" && response.url().endsWith(`${state.taskRoot}/journey-consents`));
  await consentPanel.getByRole("checkbox", { name: /Allow this plan and sample test/ }).check();
  await consentPanel.getByRole("button", { name: "Build plan and test samples", exact: true }).click();
  const response = await acknowledged;
  expect(response.ok(), await response.text()).toBe(true);
  const saved = await response.json();
  expect(saved.consent.repair.request_id).toBe(help.input.id);
  await expect(joint.getByText("Sample results saved", { exact: true })).toBeVisible({ timeout: 75_000 });
  expect(posts.filter(path => path.endsWith("/journey-consents"))).toHaveLength(1);
  expect(posts.filter(path => path.endsWith("/execution"))).toHaveLength(1);
  expect(posts.filter(path => /builder-operations|sample-operations/.test(path))).toEqual([]);
  const count = posts.length;
  await page.reload();
  await expect(joint.getByText("Sample results saved", { exact: true })).toBeVisible();
  expect(posts).toHaveLength(count);
  const original = page.getByRole("region", { name: "Annotation Schema proposal", exact: true });
  await expect(original.getByRole("link", { name: "View actual plan and execution details", exact: true })).toHaveAttribute("href", new RegExp(`draft=${state.record.draft_id}`));
  await expect(joint.getByRole("link", { name: "View actual plan and execution details", exact: true })).toHaveAttribute("href", new RegExp(`draft=${saved.consent.repair.draft_id}`));
  expect(saved.consent.repair.draft_id).not.toBe(state.record.draft_id);
  const advancedButton = await repair.getByRole("button", { name: "Review Builder authorization", exact: true }).boundingBox();
  const advancedNote = await repair.getByText("Advanced: build only, then authorize samples separately.", { exact: true }).boundingBox();
  expect(advancedNote!.y).toBeGreaterThanOrEqual(advancedButton!.y + advancedButton!.height + 8);
  await joint.getByRole("button", { name: "View sample results in canvas", exact: true }).scrollIntoViewIfNeeded();
  await page.screenshot({ path: isolatedEvidencePath("../docs/execution/conversational-workspace/joint-repair-result.png"), fullPage: true });
});
