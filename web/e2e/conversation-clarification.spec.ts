import {randomUUID} from "node:crypto";
import {resolve} from "node:path";
import {expect,test,fetchWithinMutationLimit} from "./fixtures";

for(const mode of ["answer","cancel"] as const){
test(`Schema clarification ${mode}: same-task persistence and explicit continuation`,async({page,request})=>{
  test.setTimeout(120_000);
  // The browser suite shares only this isolated TEST registry between scenarios.
  // Retire prior scenario transports so fixture template selection is deterministic.
  const existingProfiles=(await (await request.get("/api/model-profiles")).json()).models;
  for(const profile of existingProfiles){
    if(profile.display_name.startsWith("Conversation TEST "))
      expect((await request.patch(`/api/model-profiles/${profile.id}`,{data:{enabled:false}})).ok()).toBe(true);
  }
  const provider=await (await request.post("/api/providers",{data:{display_name:"Clarification TEST",adapter:"open_ai_compatible",base_url:"http://127.0.0.1:8796/openai/v1"}})).json();
  expect((await request.post(`/api/providers/${provider.id}/credential`,{data:{source:"workspace_file",secret:"TEST-clarification-only"}})).ok()).toBe(true);
  const model=await (await request.post("/api/model-profiles",{data:{provider_id:provider.id,display_name:"Conversation TEST clarification",remote_model_id:"e2e-conversation-clarify",input_modalities:["text","image"],task_capabilities:["text_generation","vision_language","image_classification"],protocol_features:{tool_calls:true,structured_output:true}}})).json();
  expect((await request.post(`/api/providers/${provider.id}/active-probe`,{data:{model_profile_id:model.id,confirmed_billable:true}})).ok()).toBe(true);
  const defaults=await (await request.get("/api/agent-model-bindings")).json();
  expect((await request.put("/api/agent-model-bindings",{data:{...defaults,pipeline_builder:model.id}})).ok()).toBe(true);
  const project=`clarification-${Date.now()}`;
  expect((await request.post("/api/projects",{data:{id:project,yaml:"version: 1\nproject:\n  name: TEST clarification\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}})).ok()).toBe(true);
  expect((await request.put(`/api/projects/${project}/model-bindings`,{data:{bindings:[{capability:"image_classification",role:"classification",match_kind:"capability",model_profile_id:model.id,locked:false}]}})).ok()).toBe(true);
  await page.goto(`/projects/${project}/work`);
  await page.getByLabel("Add images",{exact:true}).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
    await expect(page.getByText("Images saved on this server. This upload did not start inference.",{exact:true})).toBeVisible();
  await page.getByLabel("Your message",{exact:true}).fill("TEST: annotate these images; help clarify the output");
  if(mode==="answer"){
    let interrupted=false;
    await page.route("**/conversations/*/tasks",async route=>{
      if(route.request().method()!=="POST"||interrupted)return route.continue();
      interrupted=true;
      const response=await fetchWithinMutationLimit(route);expect(response.ok(),await response.text()).toBe(true);
      await route.abort("failed");
    });
    await page.getByRole("button",{name:"Save goal and prepare labels",exact:true}).click();
    await expect(page.getByText("Message saved; goal preparation is not confirmed. Retry uses the same message and restores any saved task.",{exact:true})).toBeVisible();
    await page.getByRole("button",{name:"Retry saving message",exact:true}).click();
    await expect(page).toHaveURL(/task=/);
    await expect(page.getByLabel("Schema model authorization",{exact:true})).toBeVisible();
    const owner=(await (await request.get(`/api/projects/${project}/conversations`)).json()).conversation_id;
    const task=(await (await request.get(`/api/projects/${project}/conversations/${owner}/tasks`)).json())[0];
    expect((await (await request.get(`/api/projects/${project}/conversations/${owner}/tasks/${task.input.id}/budget`)).json()).total_reserved_calls).toBe(0);
    expect(await (await request.get(`/api/projects/${project}/conversations/${owner}/messages`)).json()).toHaveLength(1);
    await page.getByLabel("Schema model authorization",{exact:true}).screenshot({path:"../docs/execution/conversational-workspace/first-goal-authorization.png",animations:"disabled"});
  }else{
    await page.getByRole("button",{name:"Save message",exact:true}).click();
    await page.getByRole("button",{name:"Prepare label proposal",exact:true}).click();
  }
  await page.getByRole("checkbox",{name:/Allow this text request/}).check();
  await page.getByRole("button",{name:"Generate label proposal",exact:true}).click();
  await expect(page.getByText("Clarification needed",{exact:true})).toBeVisible();
  const conversation=(await (await request.get(`/api/projects/${project}/conversations`)).json()).conversation_id;
  const root=`/api/projects/${project}/conversations/${conversation}/tasks`;
  const tasks=await (await request.get(root)).json();expect(tasks).toHaveLength(1);
  const task=tasks[0],taskRoot=`${root}/${task.input.id}`;
  const calls=await (await request.get(`${taskRoot}/calls`)).json();
  const call=calls[0].id;
  const questionPath=`${taskRoot}/calls/${call}/clarification`;
  const question=await (await request.get(questionPath)).json();
  expect(question.status).toBe("pending");expect(question.kind).toBe("clarify_task");expect(question.task_id).toBe(task.input.id);
  const before=await (await request.get(`${taskRoot}/budget`)).json();
  if(mode==="cancel"){
    await page.getByRole("button",{name:"Answer this clarification",exact:true}).click();
    await page.getByLabel("Labels · one per line",{exact:true}).fill("TEST unsaved answer");
    await page.route(`**${questionPath}/cancel`,async route=>{const response=await fetchWithinMutationLimit(route);expect(response.ok(),await response.text()).toBe(true);await route.abort("failed");},{times:1});
    page.once("dialog",dialog=>dialog.accept());
    await page.getByRole("button",{name:"Cancel clarification",exact:true}).click();
    await expect(page.getByRole("alert")).toContainText("Cancellation is not confirmed");
    await page.reload();
    await expect(page.getByText(/^Clarification cancelled\./)).toBeVisible();
    await expect(page.getByRole("button",{name:"Answer this clarification",exact:true})).toHaveCount(0);
    const reference={call_id:call,expected_schema_revision:question.expected_schema_revision};
    expect((await request.post(`${questionPath}/cancel`,{data:reference})).ok()).toBe(true);
    expect((await request.post(`${taskRoot}/human-schema-drafts`,{data:{request_id:randomUUID(),clarification:reference,decision:{decision:"draft",kind:"classification",labels:["TEST late"],multi_label:false,attributes:{},boundary_rules:[],rationale:"TEST late answer"}}})).ok()).toBe(false);
    expect(await (await request.get(`${taskRoot}/human-schema-drafts`)).json()).toEqual([]);
    expect((await (await request.get(questionPath)).json()).status).toBe("cancelled");
    expect(await (await request.get(`${taskRoot}/budget`)).json()).toEqual(before);
    await page.getByRole("region",{name:"Annotation Schema proposal",exact:true}).screenshot({path:"../docs/execution/conversational-workspace/clarification-cancelled.png",animations:"disabled"});
    return;
  }
  await page.reload();
  await page.getByRole("button",{name:"Answer this clarification",exact:true}).click();
  await expect(page.getByLabel("Answer annotation clarification",{exact:true})).toContainText(question.question);
  await page.getByLabel("Output type",{exact:true}).selectOption("classification");
  await page.getByLabel("Labels · one per line",{exact:true}).fill("室内\n室外");
  await page.getByLabel("Boundary rules · optional",{exact:true}).fill("按相机所在环境分类，不按窗外的背景分类");
  let answer:any;
  await page.route(`**${taskRoot}/human-schema-drafts`,async route=>{answer=route.request().postDataJSON();const response=await fetchWithinMutationLimit(route);expect(response.ok(),await response.text()).toBe(true);await route.abort("failed");},{times:1});
  await page.getByRole("button",{name:"Save answer and continue",exact:true}).click();
  await page.getByRole("button",{name:"Retry same label save",exact:true}).click();
  await expect(page.getByRole("region",{name:"Saved label draft",exact:true})).toContainText("Revision 1");
  const applied=await (await request.get(questionPath)).json();expect(applied.status).toBe("applied");expect(applied.schema_draft_id).toBeTruthy();
  expect(answer.clarification).toEqual({call_id:call,expected_schema_revision:question.expected_schema_revision});
  expect((await request.post(`${taskRoot}/human-schema-drafts`,{data:{...answer,request_id:randomUUID()}})).ok()).toBe(false);
  expect((await request.post(`${taskRoot}/human-schema-drafts`,{data:{...answer,clarification:{...answer.clarification,expected_schema_revision:"stale"}}})).ok()).toBe(false);
  expect((await request.get(questionPath.replace(task.input.id,randomUUID()))).ok()).toBe(false);
  await page.reload();
  await expect(page.getByRole("region",{name:"Saved label draft",exact:true})).toContainText("Revision 1");
  await page.getByRole("button",{name:"Review Builder authorization",exact:true}).click();
  await expect(page.getByLabel("Builder model authorization",{exact:true})).toContainText("1 calls already used");
  expect(await (await request.get(`${taskRoot}/budget`)).json()).toEqual(before);
  expect(await (await request.get(root)).json()).toHaveLength(1);
  await page.getByRole("region",{name:"Saved label draft",exact:true}).screenshot({path:"../docs/execution/conversational-workspace/clarification-answer-restored.png",animations:"disabled"});
  await page.getByRole("checkbox",{name:/Allow this bounded Builder request/}).check();
  await page.getByRole("button",{name:"Build Pipeline Draft",exact:true}).click();
  await expect(page.getByRole("button",{name:"Review sample authorization",exact:true})).toBeEnabled();
  await page.reload();
  await expect(page.getByText("Builder outcome saved",{exact:true})).toBeVisible();
  const savedPlan=page.locator(".conversation-completed-stage");
  await expect(savedPlan).not.toHaveAttribute("open","");
  await expect(page.getByRole("button",{name:"Review another build request",exact:true})).toBeHidden();
  await savedPlan.locator("summary").focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("link",{name:"Open saved Pipeline details",exact:true})).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("button",{name:"Review sample authorization",exact:true})).toBeVisible();
  const answered=page.getByLabel("Saved clarification answer",{exact:true});
  await expect(answered).toContainText("Clarification answered");
  await answered.getByText("Why AnnotAgent asked",{exact:true}).click();
  await expect(answered).toContainText(question.question);
  await page.getByRole("button",{name:"Review sample authorization",exact:true}).click();
  await page.getByRole("checkbox",{name:/Allow these sample images/}).check();
  const submitted=page.waitForRequest(req=>req.method()==="POST"&&req.url().endsWith("/sample-operations"));
  await page.getByRole("button",{name:"Test these samples",exact:true}).click();
  const envelope=(await submitted).postDataJSON();
  await expect(page.getByRole("button",{name:"View sample results in canvas",exact:true})).toBeVisible();
  expect(envelope.conversation.task_id).toBe(task.input.id);
  const saved=(await (await request.get(`/api/workflow-drafts/${envelope.draft_id}/sample-test?test_id=${envelope.request_id}`)).json()).sample_test;
  expect(saved.report.validation.valid).toBe(true);
  expect(saved.report.samples[0].nodes.some((node:any)=>node.metadata.model==="e2e-conversation-clarify")).toBe(true);
  const projection=saved.report.samples[0].projection;
  expect(projection.final_candidates.length+projection.review_candidates.length).toBeGreaterThan(0);
  const after=await (await request.get(`${taskRoot}/budget`)).json();
  expect(after.planning_reserved_calls).toBeGreaterThan(before.planning_reserved_calls);
  let writes=0;
  page.on("request",req=>{if(["POST","PUT","PATCH","DELETE"].includes(req.method()))writes++;});
  await page.reload();
  await page.getByRole("button",{name:"View sample results in canvas",exact:true}).click();
  await expect(page.getByLabel("Saved sample results",{exact:true})).toBeVisible();
  await expect(page.locator(".conversation-sample-canvas .annotation-canvas image")).toBeVisible();
  expect(await (await request.get(`${taskRoot}/budget`)).json()).toEqual(after);
  expect(await (await request.get(root)).json()).toHaveLength(1);
  expect(writes).toBe(0);
  await page.screenshot({path:"../docs/execution/conversational-workspace/clarification-sample-result.png",fullPage:true,animations:"disabled"});
  await page.getByRole("button",{name:"Edit labels and boundary rules",exact:true}).click();
  await page.getByLabel("Labels · one per line",{exact:true}).fill("室内\n室外\n不确定");
  await page.getByRole("button",{name:"Save Schema changes",exact:true}).click();
  await expect(page.getByText("Labels are now revision 2; this saved operation has not been rebuilt for those changes.",{exact:true})).toBeVisible();
  await expect(page.locator(".conversation-completed-stage")).toHaveCount(0);
  expect(await (await request.get(`${taskRoot}/budget`)).json()).toEqual(after);
});
}
