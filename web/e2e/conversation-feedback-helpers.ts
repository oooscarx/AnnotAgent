import {randomUUID} from "node:crypto";
import {resolve} from "node:path";
import type {APIRequestContext, Page} from "@playwright/test";
import {expect as baseExpect} from "./fixtures";

const expect=baseExpect.configure({timeout:75_000});

// All records and credentials in this file belong to the isolated TEST workspace
// and its deterministic 8796 transport. The fixture does not prove live semantics
// or accuracy, and never touches the user's 8787 workspace.
export async function sample(request:APIRequestContext,page:Page,scenario:string,bbox=false){
  const provider=await (await request.post("/api/providers",{data:{display_name:"TEST candidate feedback",adapter:"open_ai_compatible",base_url:"http://127.0.0.1:8796/openai/v1"}})).json();
  expect((await request.post(`/api/providers/${provider.id}/credential`,{data:{source:"workspace_file",secret:"TEST-feedback-only"}})).ok()).toBe(true);
  const model=await (await request.post("/api/model-profiles",{data:{provider_id:provider.id,display_name:`TEST feedback ${scenario}`,remote_model_id:`e2e-conversation-${bbox?"bbox":"classification"}-feedback-${scenario}`,input_modalities:["text","image"],task_capabilities:["text_generation","vision_language","image_classification"],protocol_features:{tool_calls:true,structured_output:true}}})).json();
  expect((await request.post(`/api/providers/${provider.id}/active-probe`,{data:{model_profile_id:model.id,confirmed_billable:true}})).ok()).toBe(true);
  const defaults=await (await request.get("/api/agent-model-bindings")).json();
  expect((await request.put("/api/agent-model-bindings",{data:{...defaults,pipeline_builder:model.id}})).ok()).toBe(true);
  const project=`TEST-feedback-${scenario}-${randomUUID()}`;
  expect((await request.post("/api/projects",{data:{id:project,yaml:"version: 1\nproject:\n  name: TEST saved candidate feedback\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}})).ok()).toBe(true);
  expect((await request.put(`/api/projects/${project}/model-bindings`,{data:{bindings:[{capability:"image_classification",role:"classification",match_kind:"capability",model_profile_id:model.id,locked:false}]}})).ok()).toBe(true);
  await page.goto(`/projects/${project}/work`);
  await page.getByLabel("Add images",{exact:true}).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await expect(page.getByText("Images saved on this server. This upload did not start inference.",{exact:true})).toBeVisible();
  const conversation=(await (await request.post(`/api/projects/${project}/conversations`)).json()).conversation_id;
  const root=`/api/projects/${project}/conversations/${conversation}`;
  const sourceMessage=randomUUID(), task=randomUUID();
  const schemaRevision=(await (await request.get(`/api/projects/${project}/goal`)).json()).revision;
  expect((await request.post(`${root}/messages`,{data:{id:sourceMessage,text:bbox?"Find cups, not bottles. Draw tight bounding boxes.":"按室内和室外给图片分类",image:null}})).ok()).toBe(true);
  expect((await request.post(`${root}/tasks`,{data:{id:task,source_message_id:sourceMessage,schema_revision:schemaRevision}})).ok()).toBe(true);
  const taskRoot=`${root}/tasks/${task}`;
  const query=new URLSearchParams({consent_id:randomUUID(),schema_call_id:randomUUID(),builder_operation_id:randomUUID(),sample_operation_id:randomUUID(),planner_model_id:model.id,allowed_models:JSON.stringify([`model-profile:${model.id}`])});
  const preview=await request.get(`${taskRoot}/journey-preview?${query}`);expect(preview.ok(),await preview.text()).toBe(true);
  const proposed=(await preview.json()).consent;
  const consent={...proposed,allow_unknown_cost:true,schema_proposal:{...proposed.schema_proposal,allow_unknown_cost:true}};
  expect((await request.post(`${taskRoot}/journey-consents`,{data:consent})).ok()).toBe(true);
  const path=`${taskRoot}/journey-consents/${consent.id}/execution`;
  const start=await request.post(path,{data:{}});expect(start.ok(),await start.text()).toBe(true);
  await expect.poll(async()=> (await (await request.get(path)).json()).dispatch.status).toBe("settled");
  // The coordinator settles after handing off the existing sample worker. Its
  // terminal receipt and subsequent human-assistance delivery are separate facts.
  await expect.poll(async()=> (await (await request.get(path)).json()).sample?.status).toBe("succeeded");
  await expect.poll(async()=> (await (await request.get(path)).json()).sample?.assistance?.status).toBe("completed");
  const completed=await (await request.get(path)).json();
  expect(completed.dispatch.error).toBeNull();expect(completed.sample.status).toBe("succeeded");
  const record=(await (await request.get(`/api/workflow-drafts/${completed.sample.draft_id}/sample-test?test_id=${consent.sample_operation_id}`)).json()).sample_test;
  const projection=record.report.samples[0].projection;
  const candidate=projection.final_candidates[0]??projection.review_candidates[0]?.candidate;
  expect(candidate).toBeTruthy();
  const image=record.inputs[0];
  const tasks=await (await request.get(`${root}/tasks`)).json();
  const reference={scope:"sample_candidate",task_id:task,project_schema_revision:tasks.find((value:any)=>value.input.id===task).input.schema_revision,draft_id:record.draft_id,draft_revision:record.draft_revision,sample_test_id:record.id,candidate_id:candidate.outcome.id,source_artifact_id:candidate.source_artifact_id};
  const message={id:randomUUID(),text:"TEST selected candidate needs a correction, not a project-wide label change.",image:{image_id:image.image_id,sha256:image.content_hash},reference};
  expect((await request.post(`${root}/messages`,{data:message})).ok()).toBe(true);
  const savedRequests=await (await request.get(`${taskRoot}/human-requests`)).json();
  // TEST bbox samples intentionally contain a safety-review request. The direct
  // link scenario preserves it; classification scenarios use actual ready sample
  // results and need no cancellation or exemption from the pending-input block.
  const url=`/projects/${project}/work?${new URLSearchParams({conversation,task,draft:record.draft_id,test:record.id,image:image.image_id})}`;
  return {project,conversation,root,taskRoot,task,model,provider,record,candidate,image,message,savedRequests,url};
}

export type Sample=Awaited<ReturnType<typeof sample>>;
export async function preview(request:APIRequestContext,state:Sample,call=randomUUID()){
  const response=await request.get(`${state.taskRoot}/feedback-preview?${new URLSearchParams({message_id:state.message.id,call_id:call,model_id:state.model.id})}`);
  expect(response.ok(),await response.text()).toBe(true);
  return response.json();
}
export async function authorize(request:APIRequestContext,state:Sample){
  const value=await preview(request,state);
  const consent={...value.consent,allow_unknown_cost:true};
  const response=await request.post(`${state.taskRoot}/feedback-authorizations`,{data:consent});
  expect(response.ok(),await response.text()).toBe(true);
  return {value,consent,path:`${state.taskRoot}/feedback/${consent.call_id}`};
}
export async function calls(request:APIRequestContext,state:Sample){
  return (await (await request.get(`${state.taskRoot}/calls`)).json()).filter((call:any)=>call.evidence?.phase==="feedback_text");
}

export async function clarification(request:APIRequestContext,page:Page,bbox=false){
  const state=await sample(request,page,"clarify",bbox);
  if(bbox){
    // This isolated protocol test explicitly cancels the TEST sample's existing
    // safety request before starting another interpretation. Product code does
    // not bypass or silently cancel the pending-human gate.
    for(const waiting of state.savedRequests){
      expect((await request.post(`${state.taskRoot}/human-requests/${waiting.input.id}/cancel`,{data:{}})).ok()).toBe(true);
    }
  }
  const authorization=await authorize(request,state);
  const started=await request.post(`${authorization.path}/execute`,{data:{}});expect(started.ok(),await started.text()).toBe(true);
  await expect.poll(async()=> (await (await request.get(authorization.path)).json()).receipt?.status).toBe("completed");
  const status=await (await request.get(authorization.path)).json();
  expect(status.decision.Ok.decision).toBe("clarify_scope");
  expect(status.scope_context_digest).toMatch(/^[a-f0-9]{64}$/);expect(status.scope_answer).toBeNull();
  return {...state,...authorization,status};
}
