import {randomUUID} from "node:crypto";
import {resolve} from "node:path";
import {test,expect} from "./fixtures";

for(const kind of ["classification","bbox","clarify","invalid-schema"]){
test(`initial goal journey ${kind} preserves one consent through actual Schema and child services`,async({page,request})=>{
  test.setTimeout(90_000);
  const provider=await (await request.post("/api/providers",{data:{display_name:"TEST initial journey",adapter:"open_ai_compatible",base_url:"http://127.0.0.1:8796/openai/v1"}})).json();
  expect((await request.post(`/api/providers/${provider.id}/credential`,{data:{source:"workspace_file",secret:"TEST-initial-journey-only"}})).ok()).toBe(true);
  const model=await (await request.post("/api/model-profiles",{data:{provider_id:provider.id,display_name:`TEST initial ${kind}`,remote_model_id:`e2e-conversation-${kind}`,input_modalities:["text","image"],task_capabilities:["text_generation","vision_language","image_classification"],protocol_features:{tool_calls:true,structured_output:true}}})).json();
  expect((await request.post(`/api/providers/${provider.id}/active-probe`,{data:{model_profile_id:model.id,confirmed_billable:true}})).ok()).toBe(true);
  const project=`TEST-initial-journey-${kind}-${Date.now()}`;
  expect((await request.post("/api/projects",{data:{id:project,yaml:"version: 1\nproject:\n  name: TEST initial journey\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}})).ok()).toBe(true);
  expect((await request.put(`/api/projects/${project}/model-bindings`,{data:{bindings:[{capability:"image_classification",role:"classification",match_kind:"capability",model_profile_id:model.id,locked:false}]}})).ok()).toBe(true);
  await page.goto(`/projects/${project}/work`);
  await page.getByLabel("Add images",{exact:true}).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await expect(page.getByText("Images saved on this server. This upload did not start inference.",{exact:true})).toBeVisible();
  const conversation=(await (await request.post(`/api/projects/${project}/conversations`)).json()).conversation_id;
  const root=`/api/projects/${project}/conversations/${conversation}`;
  const message=randomUUID(),task=randomUUID();
  const goal=kind==="classification"?"按室内、室外给图片分类":"Find cups, not bottles. Draw tight bounding boxes.";
  expect((await request.post(`${root}/messages`,{data:{id:message,text:goal,image:null}})).ok()).toBe(true);
  const revision=(await (await request.get(`/api/projects/${project}/goal`)).json()).revision;
  expect((await request.post(`${root}/tasks`,{data:{id:task,source_message_id:message,schema_revision:revision}})).ok()).toBe(true);
  const taskRoot=`${root}/tasks/${task}`;
  const selection={consent_id:randomUUID(),schema_call_id:randomUUID(),builder_operation_id:randomUUID(),sample_operation_id:randomUUID(),planner_model_id:model.id,allowed_models:JSON.stringify([`model-profile:${model.id}`])};
  const previewResponse=await request.get(`${taskRoot}/journey-preview?${new URLSearchParams(selection)}`);expect(previewResponse.ok(),await previewResponse.text()).toBe(true);
  const preview=await previewResponse.json();expect(preview.consent.schema_revision).toBe(0);expect(preview.estimated_cost).toBeNull();
  expect((await request.post(`${taskRoot}/journey-consents`,{data:preview.consent})).status()).toBe(400);
  const consent={...preview.consent,allow_unknown_cost:true,schema_proposal:{...preview.consent.schema_proposal,allow_unknown_cost:true}};
  const saved=await request.post(`${taskRoot}/journey-consents`,{data:consent});expect(saved.ok(),await saved.text()).toBe(true);
  expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual([]);
  const path=`${taskRoot}/journey-consents/${consent.id}/execution`;
  const start=await request.post(path,{data:{}});expect(start.ok(),await start.text()).toBe(true);
  await expect.poll(async()=> (await (await request.get(path)).json()).dispatch.status,{timeout:30_000}).toBe("settled");
  const state=await (await request.get(path)).json();expect(state.dispatch.error).toBeNull();expect(state.schema.status).toBe("completed");
  expect(state.record.consent).toEqual(consent);
  if(kind==="clarify"||kind==="invalid-schema"){
    if(kind==="clarify")expect(state.schema.evidence.decision.Ok.decision).toBe("clarify");else expect(state.schema.evidence.decision.Err).toBeTruthy();
    expect(state.builder).toBeNull();expect(state.sample).toBeNull();expect(state.record.resolved_consent).toBeNull();
  }else{
    expect(state.record.resolved_consent.schema_revision).toBe(1);expect(state.record.resolved_consent.schema_proposal).toBeUndefined();
    expect(state.record.resolved_consent.previous_grant_id).toBe(consent.schema_proposal.call_id);
    expect(state.builder.status).toBe("completed");
    await expect.poll(async()=> (await (await request.get(path)).json()).sample?.status).toBe("succeeded");
    const report=(await (await request.get(`/api/workflow-drafts/${state.sample.draft_id}/sample-test?test_id=${consent.sample_operation_id}`)).json()).sample_test.report;
    expect(report.validation.valid).toBe(true);expect(report.samples).toHaveLength(1);
  }
  const calls=await (await request.get(`${taskRoot}/calls`)).json();
  expect(calls.filter((call:any)=>call.evidence?.decision)).toHaveLength(1);
  expect((await request.post(path,{data:{}})).ok()).toBe(true);
  await expect.poll(async()=> (await (await request.get(path)).json()).dispatch.status).toBe("settled");
  expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual(calls);
  expect((await (await request.get(`${taskRoot}/journey-consents`)).json()).items).toHaveLength(1);
});
}
