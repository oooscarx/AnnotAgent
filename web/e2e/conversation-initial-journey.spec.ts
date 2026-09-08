import { isolatedEvidencePath } from "./evidence";
import {randomUUID} from "node:crypto";
import {resolve} from "node:path";
import {test,expect as baseExpect,fetchWithinMutationLimit} from "./fixtures";

// The shared TEST server may reject uploads before execution for its 60-second
// mutation window. Observe the fixture's bounded 65-second wait, not only 10 seconds.
const expect=baseExpect.configure({timeout:75_000});

for(const kind of ["ui-classification","ui-clarify","classification","bbox","clarify","invalid-schema","clarify-resume","clarify-legacy","clarify-model-change"]){
test(`initial goal journey ${kind} preserves one consent through actual Schema and child services`,async({page,request})=>{
  test.setTimeout(90_000);
  const provider=await (await request.post("/api/providers",{data:{display_name:"TEST initial journey",adapter:"open_ai_compatible",base_url:"http://127.0.0.1:8796/openai/v1"}})).json();
  expect((await request.post(`/api/providers/${provider.id}/credential`,{data:{source:"workspace_file",secret:"TEST-initial-journey-only"}})).ok()).toBe(true);
  const remote=kind==="ui-classification"?"e2e-conversation-classification-schema-background":kind==="ui-clarify"||kind.startsWith("clarify")?"e2e-conversation-clarify":`e2e-conversation-${kind}`;
  const model=await (await request.post("/api/model-profiles",{data:{provider_id:provider.id,display_name:`TEST initial ${kind}`,remote_model_id:remote,input_modalities:["text","image"],task_capabilities:["text_generation","vision_language","image_classification"],protocol_features:{tool_calls:true,structured_output:true}}})).json();
  expect((await request.post(`/api/providers/${provider.id}/active-probe`,{data:{model_profile_id:model.id,confirmed_billable:true}})).ok()).toBe(true);
  const project=`TEST-initial-journey-${kind}-${Date.now()}`;
  expect((await request.post("/api/projects",{data:{id:project,yaml:"version: 1\nproject:\n  name: TEST initial journey\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}})).ok()).toBe(true);
  expect((await request.put(`/api/projects/${project}/model-bindings`,{data:{bindings:[{capability:"image_classification",role:"classification",match_kind:"capability",model_profile_id:model.id,locked:false}]}})).ok()).toBe(true);
  await page.goto(`/projects/${project}/work`);
  await page.getByLabel("Add images",{exact:true}).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await expect(page.getByText("Images saved on this server. This upload did not start inference.",{exact:true})).toBeVisible();
  const conversation=(await (await request.post(`/api/projects/${project}/conversations`)).json()).conversation_id;
  const root=`/api/projects/${project}/conversations/${conversation}`;
  const message=randomUUID();let task=randomUUID();
  const goal=kind.endsWith("classification")?"按室内、室外给图片分类":"Find cups, not bottles. Draw tight bounding boxes.";
  const revision=(await (await request.get(`/api/projects/${project}/goal`)).json()).revision;
  if(!kind.startsWith("ui-")){
    expect((await request.post(`${root}/messages`,{data:{id:message,text:goal,image:null}})).ok()).toBe(true);
    expect((await request.post(`${root}/tasks`,{data:{id:task,source_message_id:message,schema_revision:revision}})).ok()).toBe(true);
  }
  let taskRoot=`${root}/tasks/${task}`;
  if(kind.startsWith("ui-")){
    const defaults=await (await request.get("/api/agent-model-bindings")).json();
    expect((await request.put("/api/agent-model-bindings",{data:{...defaults,pipeline_builder:model.id}})).ok()).toBe(true);
    await page.goto(`/projects/${project}/work?conversation=${conversation}`);
    await page.getByLabel("Your message",{exact:true}).fill(goal);
    await page.getByRole("button",{name:"Save message",exact:true}).click();
    const admitted=page.waitForResponse(response=>response.url().endsWith("/tasks")&&response.request().method()==="POST");
    await page.getByRole("button",{name:"Prepare annotation request",exact:true}).click();
    task=(await (await admitted).json()).input.id;taskRoot=`${root}/tasks/${task}`;
    const panel=page.getByRole("region",{name:"Build and test annotation plan",exact:true});
    await expect(panel.locator(".journey-model-choices input").first()).toBeAttached();
    if(await panel.locator(".journey-model-choices input").count()>32){
      await expect(panel.locator(".journey-model-choices input:checked")).toHaveCount(0);
      await panel.getByRole("checkbox",{name:model.display_name,exact:true}).check();
      await panel.getByRole("button",{name:"Review build and sample authorization",exact:true}).click();
    }
    const authorization=panel.getByLabel("Build and sample authorization",{exact:true});
    await expect(authorization).toContainText("9 planning calls (including one label proposal)");
    await authorization.getByRole("checkbox",{name:/Allow this plan and sample test/}).check();
    if(kind==="ui-classification")await authorization.screenshot({path:isolatedEvidencePath("../docs/execution/conversational-workspace/initial-goal-consent.png"),animations:"disabled"});
    const execution=page.waitForResponse(response=>response.url().endsWith("/execution")&&response.request().method()==="POST");
    await authorization.getByRole("button",{name:"Build plan and test samples",exact:true}).click();
    const response=await execution;expect(response.ok(),await response.text()).toBe(true);
    const receipt=await response.json();expect(receipt.record.consent.schema_proposal.allow_unknown_cost).toBe(true);
    const writes:string[]=[];page.on("request",request=>{if(request.method()==="POST"&&/journey-consents|schema-proposals|builder-operations|sample-operations/.test(request.url()))writes.push(request.url());});
    await page.reload();
    if(kind==="ui-clarify"){
      await expect(page.getByText("Clarification needed",{exact:true})).toBeVisible();
      await expect(page.getByRole("button",{name:"Answer this clarification",exact:true})).toBeVisible();
      const state=await (await request.get(`${taskRoot}/journey-consents/${receipt.record.consent.id}/execution`)).json();
      expect(state.builder).toBeNull();expect(state.sample).toBeNull();
      await page.getByRole("region",{name:"Annotation Schema proposal",exact:true}).screenshot({path:isolatedEvidencePath("../docs/execution/conversational-workspace/initial-goal-clarification.png"),animations:"disabled"});
      expect(writes).toEqual([]);
      await page.getByRole("button",{name:"Answer this clarification",exact:true}).click();
      await page.getByLabel("Output type",{exact:true}).selectOption("classification");
      await page.getByLabel("Labels · one per line",{exact:true}).fill("室内\n室外");
      await page.setViewportSize({width:1280,height:1100});
      const answerPanel=page.getByRole("region",{name:"Define labels without a model",exact:true});
      await answerPanel.evaluate(element=>element.scrollIntoView({block:"center"}));
      await answerPanel.screenshot({path:isolatedEvidencePath("../docs/execution/conversational-workspace/initial-clarification-continuation.png"),animations:"disabled"});
      await page.setViewportSize({width:1280,height:800});
      const answerSaved=page.waitForRequest(request=>request.url().endsWith("/human-schema-drafts")&&request.method()==="POST");
      // The server owns continuation even if acknowledgement of the saved answer is lost.
      await page.route(`**${taskRoot}/human-schema-drafts`,async route=>{const response=await fetchWithinMutationLimit(route);expect(response.ok(),await response.text()).toBe(true);await route.abort("failed");},{times:1});
      await page.getByRole("button",{name:"Save answer and continue",exact:true}).click();
      expect((await answerSaved).postDataJSON().journey_consent_id).toBe(receipt.record.consent.id);
      await page.getByRole("button",{name:"Retry same label save",exact:true}).click();
      await expect(page.getByRole("region",{name:"Saved label draft",exact:true})).toContainText("Revision 1");
      await page.reload();
      await expect(panel.getByRole("button",{name:"View sample results in canvas",exact:true})).toBeVisible();
      const resumed=await (await request.get(`${taskRoot}/journey-consents/${receipt.record.consent.id}/execution`)).json();
      expect(resumed.record.consent).toEqual(receipt.record.consent);
      expect(resumed.record.resolved_consent.schema_id).toBe(resumed.clarification.schema_draft_id);
      expect((await (await request.get(`${taskRoot}/calls`)).json()).filter((call:any)=>call.evidence?.decision)).toHaveLength(1);
      await panel.getByRole("button",{name:"View sample results in canvas",exact:true}).click();
      await expect(page.getByLabel("Saved sample results",{exact:true})).toBeVisible();
      await page.screenshot({path:isolatedEvidencePath("../docs/execution/conversational-workspace/initial-clarification-result.png"),fullPage:true,animations:"disabled"});
    }else{
      await expect(page.getByRole("button",{name:"Stop build and sample task",exact:true})).toBeVisible();
      await expect(panel.getByRole("button",{name:"View sample results in canvas",exact:true})).toBeVisible();
      await expect(page.getByText("Schema Draft saved · Revision 1",{exact:true})).toBeVisible();
      await expect(page.getByText(/These records use labels revision 0/)).toHaveCount(0);
      await panel.getByRole("button",{name:"View sample results in canvas",exact:true}).click();
      await expect(page.getByLabel("Saved sample results",{exact:true})).toBeVisible();
      const url=page.url();await page.reload();await expect(page).toHaveURL(url);await expect(page.getByLabel("Saved sample results",{exact:true})).toBeVisible();
      await page.screenshot({path:isolatedEvidencePath("../docs/execution/conversational-workspace/initial-goal-result.png"),fullPage:true,animations:"disabled"});
    }
    expect(writes).toEqual([]);expect((await (await request.get(`${taskRoot}/journey-consents`)).json()).items).toHaveLength(1);
    return;
  }
  const selection={consent_id:randomUUID(),schema_call_id:randomUUID(),builder_operation_id:randomUUID(),sample_operation_id:randomUUID(),planner_model_id:model.id,allowed_models:JSON.stringify([`model-profile:${model.id}`])};
  const previewResponse=await request.get(`${taskRoot}/journey-preview?${new URLSearchParams(selection)}`);expect(previewResponse.ok(),await previewResponse.text()).toBe(true);
  const preview=await previewResponse.json();expect(preview.consent.schema_revision).toBe(0);expect(preview.estimated_cost).toBeNull();
  expect((await request.post(`${taskRoot}/journey-consents`,{data:preview.consent})).status()).toBe(400);
  const consent={...preview.consent,allow_unknown_cost:true,schema_proposal:{...preview.consent.schema_proposal,allow_unknown_cost:true}};
  if(kind==="clarify-legacy")delete consent.continue_after_clarification;
  const saved=await request.post(`${taskRoot}/journey-consents`,{data:consent});expect(saved.ok(),await saved.text()).toBe(true);
  expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual([]);
  const path=`${taskRoot}/journey-consents/${consent.id}/execution`;
  const start=await request.post(path,{data:{}});expect(start.ok(),await start.text()).toBe(true);
  await expect.poll(async()=> (await (await request.get(path)).json()).dispatch.status,{timeout:30_000}).toBe("settled");
  const state=await (await request.get(path)).json();expect(state.dispatch.error).toBeNull();expect(state.schema.status).toBe("completed");
  expect(state.record.consent).toEqual(consent);
  if(kind.startsWith("clarify")||kind==="invalid-schema"){
    if(kind.startsWith("clarify"))expect(state.schema.evidence.decision.Ok.decision).toBe("clarify");else expect(state.schema.evidence.decision.Err).toBeTruthy();
    expect(state.builder).toBeNull();expect(state.sample).toBeNull();expect(state.record.resolved_consent).toBeNull();
  }else{
    expect(state.record.resolved_consent.schema_revision).toBe(1);expect(state.record.resolved_consent.schema_proposal).toBeUndefined();
    expect(state.record.resolved_consent.previous_grant_id).toBe(consent.schema_proposal.call_id);
    expect(state.builder.status).toBe("completed");
    await expect.poll(async()=> (await (await request.get(path)).json()).sample?.status).toBe("succeeded");
    const report=(await (await request.get(`/api/workflow-drafts/${state.sample.draft_id}/sample-test?test_id=${consent.sample_operation_id}`)).json()).sample_test.report;
    expect(report.validation.valid).toBe(true);expect(report.samples).toHaveLength(1);
  }
  if(kind.startsWith("clarify-")){
    const question=(await (await request.get(`${taskRoot}/calls/${consent.schema_proposal.call_id}/clarification`)).json());
    const answer={request_id:randomUUID(),journey_consent_id:consent.id,clarification:{call_id:consent.schema_proposal.call_id,expected_schema_revision:question.expected_schema_revision},decision:{decision:"draft",kind:"classification",labels:["室内","室外"],multi_label:false,attributes:{},boundary_rules:[],rationale:"TEST human clarification"}};
    if(kind==="clarify-legacy"){
      expect((await request.post(`${taskRoot}/human-schema-drafts`,{data:answer})).status()).toBe(400);
      delete (answer as any).journey_consent_id;
    }
    if(kind==="clarify-model-change")expect((await request.patch(`/api/model-profiles/${model.id}`,{data:{enabled:false}})).ok()).toBe(true);
    const answerResponse=await request.post(`${taskRoot}/human-schema-drafts`,{data:answer});expect(answerResponse.ok(),await answerResponse.text()).toBe(true);
    const savedAnswer=await answerResponse.json();expect(savedAnswer.revision).toBe(1);
    if(kind!=="clarify-legacy")await expect.poll(async()=>(await (await request.get(path)).json()).dispatch.status).toBe("settled");
    const after=await (await request.get(path)).json();expect(after.record.consent).toEqual(consent);
    if(kind==="clarify-resume"){
      await expect.poll(async()=>(await (await request.get(path)).json()).sample?.status).toBe("succeeded");
      expect(after.record.resolved_consent.schema_id).toBe(savedAnswer.id);
    }else {expect(after.sample).toBeNull();expect(after.builder).toBeNull();}
    if(kind==="clarify-model-change")expect(after.dispatch.error).toBeTruthy();
    const previousCalls=await (await request.get(`${taskRoot}/calls`)).json();
    expect((await request.post(`${taskRoot}/human-schema-drafts`,{data:answer})).ok()).toBe(true);
    if(kind!=="clarify-legacy")await expect.poll(async()=>(await (await request.get(path)).json()).dispatch.status).toBe("settled");
    expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual(previousCalls);
  }
  const calls=await (await request.get(`${taskRoot}/calls`)).json();
  expect(calls.filter((call:any)=>call.evidence?.decision)).toHaveLength(1);
  expect((await request.post(path,{data:{}})).ok()).toBe(true);
  await expect.poll(async()=> (await (await request.get(path)).json()).dispatch.status).toBe("settled");
  expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual(calls);
  expect((await (await request.get(`${taskRoot}/journey-consents`)).json()).items).toHaveLength(1);
});
}
