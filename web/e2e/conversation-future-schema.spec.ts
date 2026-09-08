import { isolatedEvidencePath } from "./evidence";
import {randomUUID} from "node:crypto";
import type {APIRequestContext,Page,Locator} from "@playwright/test";
import {test,expect as baseExpect,fetchWithinMutationLimit} from "./fixtures";
import {clarification} from "./conversation-feedback-helpers";

const expect=baseExpect.configure({timeout:75_000});
type State=Awaited<ReturnType<typeof clarification>>;

// These are explicit synthetic TEST model calls on 8796, against a disposable
// 8791 workspace. Saving a future intention or Schema must never call a model.
async function read(request:APIRequestContext,path:string){
  const response=await request.get(path);expect(response.ok(),await response.text()).toBe(true);return response.json();
}
async function scope(request:APIRequestContext,state:State,value="project_future_rule"){
  const input={command_id:randomUUID(),expected_context_digest:state.status.scope_context_digest,choice:{scope:value}};
  const response=await request.post(`${state.path}/scope-answer`,{data:input});expect(response.ok(),await response.text()).toBe(true);return input;
}
async function source(request:APIRequestContext,state:State){
  const value=await read(request,`${state.path}/future-schema`);
  expect(value.scope).toBe("future_tasks_only");expect(value.record).toBeNull();expect(value.schema).toBeNull();
  expect(value.base_schema.task_id).toBe(state.task);expect(value.base_schema.revision).toBe(1);
  return value;
}
function command(value:any,bbox:boolean){
  return {command_id:randomUUID(),expected_scope_answer_command_id:value.source.scope_answer_command_id,expected_context_digest:value.source.context_digest,base_schema_id:value.base_schema.id,base_schema_revision:value.base_schema.revision,goal:bbox?"TEST future images: boxes around cups, not bottles":"TEST future images: distinguish interior, exterior and uncertain scenes",decision:{decision:"draft",kind:bbox?"bounding_box":"classification",labels:bbox?["TEST future cup"]:["TEST interior","TEST exterior","TEST uncertain"],multi_label:false,attributes:{},boundary_rules:bbox?["TEST tightly include the whole cup; exclude its shadow"]:["TEST ambiguous scenes must use the uncertain category"],rationale:"TEST explicitly human-authored future rule; saved predictions stay unchanged"}};
}
function expectStoredFork(saved:any,input:any,state:State,base:any){
  const definition={goal:input.goal,task:{...base.definition.task,kind:input.decision.kind,labels:input.decision.labels,multi_label:input.decision.multi_label,attributes:input.decision.attributes},boundary_rules:input.decision.boundary_rules};
  expect(saved.record.input).toEqual({command_id:input.command_id,feedback_call_id:state.consent.call_id,scope_answer_command_id:input.expected_scope_answer_command_id,context_digest:input.expected_context_digest,base_schema_id:input.base_schema_id,base_schema_revision:input.base_schema_revision,definition});
  expect(saved.schema.definition).toEqual(definition);
}
async function preserved(request:APIRequestContext,state:State,base:any){
  const drafts=await read(request,`/api/workflow-drafts?project_id=${state.project}`);
  const draft=drafts.drafts.find((draft:any)=>draft.id===state.record.draft_id);
  expect(draft).toBeTruthy();
  return {
    schema:await read(request,`/api/projects/${state.project}/conversation-schema-drafts/${base.id}?revision=${base.revision}`),
    goal:await read(request,`/api/projects/${state.project}/goal`),
    sample:await read(request,`/api/workflow-drafts/${state.record.draft_id}/sample-test?test_id=${state.record.id}`),
    draft,
    feedback:await read(request,`/api/workflow-sample-tests/${state.record.id}/images/${state.image.image_id}/feedback`),
    exportReadiness:await read(request,`/api/projects/${state.project}/export-readiness`),
    runs:await read(request,`/api/runs?project_id=${state.project}`),
    calls:await read(request,`${state.taskRoot}/calls`),
    budget:await read(request,`${state.taskRoot}/budget`),
  };
}
async function wideEvidence(page:Page,card:Locator){
  // Real workspace controls only: widen its resizable conversation panel and
  // use a tall evidence viewport. No injected CSS, hidden risks or pixel edits.
  await page.setViewportSize({width:1280,height:1800});
  const divider=page.getByRole("separator",{name:"Resize conversation panel",exact:true});
  for(let index=0;index<13;index++)await divider.press("ArrowRight");
  await expect(divider).toHaveAttribute("aria-valuenow","50");
  await card.evaluate(element=>element.scrollIntoView({block:"start"}));
}

for(const bbox of [false,true]){
test(`future ${bbox?"bbox":"classification"} Schema forks explicitly and needs a fresh exact Builder authorization`,async({page,request})=>{
  test.setTimeout(180_000);
  const state=await clarification(request,page,bbox);
  const answer=await scope(request,state);
  const value=await source(request,state), endpoint=`${state.path}/future-schema`;
  expect(value.source.scope_answer_command_id).toBe(answer.command_id);
  const input=command(value,bbox),before=await preserved(request,state,value.base_schema);
  expect((await page.request.post(endpoint,{data:input})).status()).toBe(403);
  expect((await request.post(endpoint,{data:input,headers:{Origin:"https://foreign.invalid"}})).status()).toBe(403);
  for(const invalid of [
    {...input,expected_context_digest:"f".repeat(64)},
    {...input,expected_scope_answer_command_id:randomUUID()},
    {...input,base_schema_id:randomUUID()},
    {...input,base_schema_revision:input.base_schema_revision+1},
  ])expect((await request.post(endpoint,{data:invalid})).ok()).toBe(false);
  for(const invalid of [{...input,apply_to_existing:true},{...input,decision:{...input.decision,geometry:[0,0,1,1]}},{...input,decision:{...input.decision,publish:true}}])expect((await request.post(endpoint,{data:invalid})).status()).toBe(422);
  const foreignMessage=randomUUID(),foreignTask=randomUUID();
  expect((await request.post(`${state.root}/messages`,{data:{id:foreignMessage,text:"TEST separate task owns no future-rule feedback",image:null}})).ok()).toBe(true);
  expect((await request.post(`${state.root}/tasks`,{data:{id:foreignTask,source_message_id:foreignMessage,schema_revision:before.goal.revision}})).ok()).toBe(true);
  expect((await request.get(endpoint.replace(state.task,foreignTask))).ok()).toBe(false);
  expect((await request.post(endpoint.replace(state.task,foreignTask),{data:input})).ok()).toBe(false);
  expect((await read(request,endpoint)).record).toBeNull();
  expect(await preserved(request,state,value.base_schema)).toEqual(before);
  const savedResponse=await request.post(endpoint,{data:input});expect(savedResponse.ok(),await savedResponse.text()).toBe(true);
  const saved=await read(request,endpoint);
  expect(await savedResponse.json()).toEqual(saved);
  expectStoredFork(saved,input,state,value.base_schema);expect(saved.record.schema_id).toBe(saved.schema.id);
  expect(saved.schema.id).not.toBe(value.base_schema.id);expect(saved.schema.revision).toBe(1);
  expect(saved.schema.definition.goal).toBe(input.goal);expect(saved.schema.definition.task.kind).toBe(input.decision.kind);
  expect(saved.schema.definition.task.labels).toEqual(input.decision.labels);expect(saved.schema.definition.boundary_rules).toEqual(input.decision.boundary_rules);
  expect(saved.base_schema).toEqual(value.base_schema);
  expect((await request.post(endpoint,{data:input})).ok()).toBe(true);expect(await read(request,endpoint)).toEqual(saved);
  expect((await request.post(endpoint,{data:{...input,command_id:randomUUID()}})).ok()).toBe(false);
  expect((await request.post(endpoint,{data:{...input,goal:"TEST changed retry"}})).ok()).toBe(false);
  expect(await preserved(request,state,value.base_schema)).toEqual(before);
  const selection={operation_id:randomUUID(),schema_id:saved.schema.id,schema_revision:1,model_id:state.model.id};
  const preview=await read(request,`${state.taskRoot}/builder-preview?${new URLSearchParams(Object.entries(selection).map(([key,value])=>[key,String(value)]))}`);
  expect(preview.selection.schema_id).toBe(saved.schema.id);expect(preview.selection.schema_revision).toBe(1);
  const authorization={selection:preview.selection,previous_grant_id:preview.previous_grant_id,scope_hash:preview.scope_hash,expires_at:preview.expires_at,allow_unknown_cost:true};
  expect((await request.post(`${state.taskRoot}/builder-operations`,{data:{...authorization,allow_unknown_cost:false}})).ok()).toBe(false);
  expect((await request.post(`${state.taskRoot}/builder-operations`,{data:{...authorization,selection:{...selection,schema_id:value.base_schema.id}}})).ok()).toBe(false);
  expect(await preserved(request,state,value.base_schema)).toEqual(before);
  // Only this explicit, bounded TEST authorization is permitted to call Builder.
  const executed=await request.post(`${state.taskRoot}/builder-operations`,{data:authorization});expect(executed.ok(),await executed.text()).toBe(true);
  const operation=await executed.json();expect(operation.status).toBe("completed");
  expect(operation.evidence.schema_id).toBe(saved.schema.id);expect(operation.evidence.schema_revision).toBe(1);
  expect(operation.evidence.draft_id).not.toBe(state.record.draft_id);
  expect(operation.evidence.samples_tested).toBe(false);expect(operation.evidence.published).toBe(false);
  const after=await preserved(request,state,value.base_schema);
  expect({...after,calls:before.calls,budget:before.budget}).toEqual(before);
  expect(after.calls.length).toBeGreaterThan(before.calls.length);
  expect((await request.post(`${state.taskRoot}/builder-operations`,{data:authorization})).ok()).toBe(true);
  expect(await preserved(request,state,value.base_schema)).toEqual(after);
  // Separately authorize exactly one sample on the newly bound plan. The TEST
  // fixture reads the actual requested labels; this checks wiring, not accuracy.
  const sampleId=randomUUID(),newDraft=operation.evidence.draft_id;
  const samplePreview=await read(request,`${state.taskRoot}/sample-preview?${new URLSearchParams({draft_id:newDraft,request_id:sampleId})}`);
  expect(samplePreview.supported).toBe(true);expect(samplePreview.image_count).toBe(1);
  expect(samplePreview.estimated_cost).toBeNull();expect(samplePreview.request_limit).toBeGreaterThan(0);
  expect(samplePreview.models.length).toBeGreaterThan(0);
  for(const model of samplePreview.models)expect(model.destination).toContain("127.0.0.1:8796");
  const sampleRequest={request_id:sampleId,draft_id:newDraft,expected_revision:samplePreview.revision,image_indices:[0],authorization_fingerprint:samplePreview.authorization_fingerprint,conversation:{conversation_id:state.conversation,task_id:state.task,previous_grant_id:samplePreview.conversation_budget.previous_grant_id,scope_hash:samplePreview.conversation_budget.scope_hash,expires_at:samplePreview.conversation_budget.expires_at,allow_unknown_cost:true,human_review:true}};
  const sampleRoot=`/api/projects/${state.project}/sample-operations`;
  expect((await request.post(sampleRoot,{data:{...sampleRequest,conversation:{...sampleRequest.conversation,allow_unknown_cost:false}}})).ok()).toBe(false);
  expect(await preserved(request,state,value.base_schema)).toEqual(after);
  const started=await request.post(sampleRoot,{data:sampleRequest});expect(started.ok(),await started.text()).toBe(true);
  await expect.poll(async()=> (await read(request,`${sampleRoot}/${sampleId}`)).status).toBe("succeeded");
  await expect.poll(async()=> (await read(request,`${sampleRoot}/${sampleId}`)).assistance?.status).toBe("completed");
  const sampleView=await read(request,`/api/workflow-drafts/${newDraft}/sample-test?test_id=${sampleId}`);
  expect(sampleView.annotation_schema.schema_draft_id).toBe(saved.schema.id);expect(sampleView.annotation_schema.revision).toBe(1);
  expect(sampleView.annotation_schema.goal).toBe(input.goal);expect(sampleView.annotation_schema.task).toEqual(saved.schema.definition.task);
  const newSample=sampleView.sample_test;expect(newSample.id).toBe(sampleId);expect(newSample.draft_id).toBe(newDraft);
  expect(newSample.inputs).toEqual(state.record.inputs);expect(newSample.report.samples).toHaveLength(1);
  const result=newSample.report.samples[0];expect(result.failed).toBe(false);
  const terminal=[...result.projection.final_candidates,...result.projection.review_candidates.map((item:any)=>item.candidate)];
  expect(terminal.length).toBeGreaterThan(0);
  for(const candidate of terminal)expect(input.decision.labels).toContain(candidate.outcome.label);
  const afterSample=await preserved(request,state,value.base_schema);
  expect({...afterSample,calls:before.calls,budget:before.budget}).toEqual(before);
  expect(afterSample.calls.length).toBeGreaterThan(after.calls.length);expect(afterSample.budget).not.toEqual(after.budget);
  expect((await request.post(sampleRoot,{data:sampleRequest})).ok()).toBe(true);
  expect(await preserved(request,state,value.base_schema)).toEqual(afterSample);
});
}

test("a current-image intention and a cancelled future intention cannot create a future Schema",async({page,request})=>{
  test.setTimeout(180_000);
  const image=await clarification(request,page);await scope(request,image,"current_image_class");
  expect((await request.get(`${image.path}/future-schema`)).ok()).toBe(false);
  const state=await clarification(request,page);await scope(request,state);
  const value=await source(request,state),input=command(value,false);
  expect((await request.post(`${image.path}/future-schema`,{data:input})).ok()).toBe(false);
  expect((await request.post(`${state.taskRoot}/calls/${state.consent.call_id}/cancel`,{data:{}})).ok()).toBe(true);
  const before=await preserved(request,state,value.base_schema);
  expect((await request.post(`${state.path}/future-schema`,{data:input})).ok()).toBe(false);
  const cancelled=await read(request,state.path);expect(cancelled.cancelled).toBe(true);
  expect(await preserved(request,state,value.base_schema)).toEqual(before);
});

for(const bbox of [false,true]){
test(`future ${bbox?"bbox":"classification"} Schema UI restores the explicit fork and never mistakes the old revision-one plan for the new one`,async({page,request})=>{
  test.setTimeout(180_000);
  const state=await clarification(request,page,bbox);await scope(request,state);
  const value=await source(request,state), before=await preserved(request,state,value.base_schema);
  await page.goto(state.url);
  const card=page.getByRole("region",{name:"Future rule draft",exact:true});
  const mutations:{url:string;input:any}[]=[];
  page.on("request",request=>{if(request.method()==="POST"&&request.url().includes(state.taskRoot))mutations.push({url:request.url(),input:request.postDataJSON()});});
  await card.getByRole("button",{name:"Edit future rule draft",exact:true}).click();
  const input=command(value,bbox);
  await expect(card.getByLabel("Output type",{exact:true})).toHaveValue(input.decision.kind);
  await card.getByLabel("Future task goal",{exact:true}).fill(input.goal);
  await card.getByLabel("Labels · one per line",{exact:true}).fill(input.decision.labels.join("\n"));
  await card.getByLabel("Boundary rules · one per line",{exact:true}).fill(input.decision.boundary_rules.join("\n"));
  await expect(card.getByRole("region",{name:"Future rule changes",exact:true})).toBeVisible();
  expect(mutations).toEqual([]);expect(await preserved(request,state,value.base_schema)).toEqual(before);
  if(bbox){
    await page.setViewportSize({width:390,height:844});await page.getByRole("button",{name:"Conversation",exact:true}).click();
    await card.evaluate(element=>element.scrollIntoView({block:"center"}));
    await card.screenshot({path:isolatedEvidencePath("../docs/execution/conversational-workspace/future-schema-bbox-390-form.png"),animations:"disabled"});
  }else{
    await wideEvidence(page,card);
    await expect(card.getByLabel("Future task goal",{exact:true})).toBeInViewport();
    await expect(card.getByRole("button",{name:"Save future rule draft",exact:true})).toBeInViewport();
    await page.screenshot({path:isolatedEvidencePath("../docs/execution/conversational-workspace/future-schema-classification-form.png"),animations:"disabled"});
    await page.setViewportSize({width:1280,height:800});
  }
  if(!bbox){
    let aborted=false;
    await page.route(`**${state.path}/future-schema`,async route=>{if(route.request().method()!=="POST")return route.fallback();await route.abort("failed");aborted=true;},{times:1});
    await card.getByRole("button",{name:"Save future rule draft",exact:true}).click();
    await expect.poll(()=>aborted).toBe(true);
    await expect(card.getByRole("button",{name:"Retry same future rule save",exact:true})).toBeEnabled();
    expect((await read(request,`${state.path}/future-schema`)).record).toBeNull();
    await page.reload();
    await expect(card.getByRole("button",{name:"Retry same future rule save",exact:true})).toBeEnabled();
    await page.route(`**${state.path}/future-schema`,async route=>{if(route.request().method()!=="POST")return route.fallback();const response=await fetchWithinMutationLimit(route);expect(response.ok(),await response.text()).toBe(true);await route.abort("failed");},{times:1});
    await card.getByRole("button",{name:"Retry same future rule save",exact:true}).click();
  }else await card.getByRole("button",{name:"Save future rule draft",exact:true}).click();
  await expect.poll(async()=> (await read(request,`${state.path}/future-schema`)).record?.schema_id).toBeTruthy();
  const saved=await read(request,`${state.path}/future-schema`);
  const submitted=mutations.filter(item=>item.url.endsWith("/future-schema"));
  expect(submitted).toHaveLength(bbox?1:2);
  if(!bbox)expect(submitted[1].input).toEqual(submitted[0].input);
  expectStoredFork(saved,submitted[0].input,state,value.base_schema);
  expect(saved.schema.id).not.toBe(value.base_schema.id);expect(saved.schema.revision).toBe(value.base_schema.revision);
  const count=mutations.length;
  await page.reload();
  await expect(card.getByText("Future rule draft saved",{exact:true})).toBeVisible();
  const originalGoal=page.getByRole("region",{name:"Annotation Schema proposal",exact:true});
  const originalLabels=originalGoal.getByRole("region",{name:"Saved label draft",exact:true});
  await expect(originalLabels).toBeVisible();
  for(const label of value.base_schema.definition.task.labels)await expect(originalLabels.getByText(label,{exact:true}).first()).toBeVisible();
  for(const label of input.decision.labels)await expect(originalLabels.getByText(label,{exact:true})).toHaveCount(0);
  expect((await read(request,`${state.taskRoot}/human-schema-drafts`)).some((draft:any)=>draft.id===saved.schema.id)).toBe(false);
  expect(mutations).toHaveLength(count);expect(await preserved(request,state,value.base_schema)).toEqual(before);
  const builder=card.getByRole("region",{name:"Build and test annotation plan",exact:true});
  await expect(builder.getByText("Sample results saved",{exact:true})).toHaveCount(0);
  await expect(builder.getByRole("button",{name:"View sample results in canvas",exact:true})).toHaveCount(0);
  await expect(builder.getByRole("button",{name:"Review build and sample authorization",exact:true})).toBeVisible();
  // A deliberately large browser-only registry must require an explicit selection,
  // not send every binding (or silently truncate). The chosen binding is real.
  const registry=await read(request,"/api/model-profiles");
  const realModel=registry.models.find((model:any)=>model.id===state.model.id);
  expect(realModel).toBeTruthy();
  const modelRegistryUrl=(url:URL)=>url.pathname==="/api/model-profiles";
  await page.route(modelRegistryUrl,route=>route.fulfill({json:{...registry,models:[realModel,...Array.from({length:33},(_,i)=>({...realModel,id:`TEST-extra-${i}`,display_name:`TEST extra choice ${i}`}))]}}));
  let previews=0;
  page.on("request",req=>{if(req.method()==="GET"&&req.url().includes(`${state.taskRoot}/journey-preview?`))previews++;});
  await builder.getByRole("button",{name:"Review build and sample authorization",exact:true}).click();
  await expect(builder.getByText(/No arbitrary subset is selected automatically/)).toBeVisible();
  await expect(builder.locator(".journey-model-choices input:checked")).toHaveCount(0);
  expect(previews).toBe(0);
  await builder.locator(".journey-model-choices input").first().check();
  const previewRequest=page.waitForResponse(response=>response.request().method()==="GET"&&response.url().includes(`${state.taskRoot}/journey-preview?`));
  await builder.getByRole("button",{name:"Review build and sample authorization",exact:true}).click();
  const response=await previewRequest;expect(response.ok(),await response.text()).toBe(true);
  const preview=await response.json();
  expect(JSON.parse(new URL(response.url()).searchParams.get("allowed_models")!)).toEqual([`model-profile:${state.model.id}`]);
  await page.unroute(modelRegistryUrl);
  expect(preview.consent.schema_id).toBe(saved.schema.id);expect(preview.consent.schema_revision).toBe(1);
  await expect(builder.getByRole("button",{name:"Build plan and test samples",exact:true})).toBeDisabled();
  expect(mutations).toHaveLength(count);expect(await preserved(request,state,value.base_schema)).toEqual(before);
  await builder.getByRole("button",{name:"Back",exact:true}).click();
  await wideEvidence(page,card);
  await expect(card.getByText("Future rule draft saved",{exact:true})).toBeInViewport();
  await expect(builder.getByRole("button",{name:"Review build and sample authorization",exact:true})).toBeInViewport();
  await expect(page.getByRole("region",{name:"Saved sample results",exact:true}).getByRole("heading",{name:"synthetic-robocup.png",exact:true})).toBeInViewport();
  await page.screenshot({path:isolatedEvidencePath(`../docs/execution/conversational-workspace/future-schema-${bbox?"bbox":"classification"}-saved.png`),animations:"disabled"});
});
}
