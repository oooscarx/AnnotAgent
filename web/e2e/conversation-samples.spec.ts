import { resolve } from "node:path";
import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import { expect as baseExpect, test, fetchWithinMutationLimit } from "./fixtures";

// This complete workflow crosses the shared TEST server's real 60-second mutation
// window. The fixture retries only proven pre-execution 429s for up to 65 seconds;
// UI observations must not fail while that bounded transport pacing is still active.
const expect=baseExpect.configure({timeout:75_000});

for(const scenario of ["classification","bbox","classification-review","human-classification","human-bbox"] as const){
const humanSchema = scenario.startsWith("human-");
const transport = humanSchema ? scenario.slice(6) : scenario;
const kind = transport === "bbox" ? "bbox" : "classification";
const requiresReview = transport !== "classification";
test(`conversation ${scenario} authorizes HTTP fixture samples and restores editable terminal canvas`,async({page,request})=>{
  test.setTimeout(180_000);
  // Each scenario must bind its own TEST transport, not an earlier compatible registry model.
  const existingProfiles = (await (await request.get("/api/model-profiles")).json()).models;
  for (const profile of existingProfiles) {
    if (profile.display_name.startsWith("Conversation TEST ")) {
      expect((await request.patch(`/api/model-profiles/${profile.id}`, {data:{enabled:false}})).ok()).toBe(true);
    }
  }
  const provider=await (await request.post("/api/providers",{data:{display_name:"Conversation sample TEST transport",adapter:"open_ai_compatible",base_url:"http://127.0.0.1:8796/openai/v1"}})).json();
  expect((await request.post(`/api/providers/${provider.id}/credential`,{data:{source:"workspace_file",secret:"TEST-conversation-samples-only"}})).ok()).toBeTruthy();
  const model=await (await request.post("/api/model-profiles",{data:{provider_id:provider.id,display_name:`Conversation TEST ${scenario}`,remote_model_id:`e2e-conversation-${transport}`,input_modalities:["text","image"],task_capabilities:["text_generation","vision_language","image_classification"],protocol_features:{tool_calls:true,structured_output:true}}})).json();
  expect((await request.post(`/api/providers/${provider.id}/active-probe`,{data:{model_profile_id:model.id,confirmed_billable:true}})).ok()).toBeTruthy();
  const defaults=await (await request.get("/api/agent-model-bindings")).json();
  expect((await request.put("/api/agent-model-bindings",{data:{...defaults,pipeline_builder:model.id}})).ok()).toBeTruthy();
  const project=`conversation-samples-${Date.now()}`;
  expect((await request.post("/api/projects",{data:{id:project,yaml:"version: 1\nproject:\n  name: TEST conversation samples\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}})).ok()).toBeTruthy();
  expect((await request.put(`/api/projects/${project}/model-bindings`,{data:{bindings:[{capability:"image_classification",role:"classification",match_kind:"capability",model_profile_id:model.id,locked:false}]}})).ok()).toBeTruthy();
  await page.goto(`/projects/${project}/work`);
  if(scenario==="human-classification"){
    await page.getByText("Project call limit",{exact:true}).click();
    const ceiling=page.getByRole("region",{name:"Project call limit",exact:true});
    await ceiling.getByLabel("Cumulative maximum calls",{exact:true}).fill("64");
    await ceiling.getByRole("checkbox").check();
    await ceiling.getByRole("button",{name:"Save Project limit",exact:true}).click();
    await expect(ceiling).toContainText("64 cumulative maximum · Revision 1");
    await page.getByText("Project call limit",{exact:true}).click();
  }
  await page.getByLabel("Add images",{exact:true}).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await expect(page.getByText("Images saved on this server. This upload did not start inference.",{exact:true})).toBeVisible();
  await page.getByLabel("Your message",{exact:true}).fill(kind==="classification" ? "按室内和室外给图片分类" : "Find cups, not bottles. Draw a tight box around each cup.");
  await page.getByRole("button",{name:"Save message",exact:true}).click();
  if(humanSchema){
    await page.getByRole("button",{name:"Define labels myself · no LLM needed",exact:true}).click();
    await expect(page.getByText("No outstanding visual requests for this goal.",{exact:true})).toBeVisible({timeout:10_000});
    await expect(page.getByText("Select an annotation goal to see its requests.",{exact:true})).toHaveCount(0);
    await page.getByLabel("Output type",{exact:true}).selectOption(kind==="bbox"?"bounding_box":"classification");
    await page.getByLabel("Labels · one per line",{exact:true}).fill(kind==="bbox"?"cup":"室内\n室外");
    await page.getByRole("button",{name:"Save label draft without a model",exact:true}).click();
    await expect(page.getByRole("region",{name:"Saved label draft",exact:true})).toContainText("Revision 1");
    await page.reload();
  }else{
  await page.getByRole("button",{name:"Prepare label proposal",exact:true}).click();
  await expect(page.getByText("No outstanding visual requests for this goal.",{exact:true})).toBeVisible({timeout:10_000});
  await expect(page.getByText("Select an annotation goal to see its requests.",{exact:true})).toHaveCount(0);
  await page.getByRole("checkbox",{name:/Allow this text request/}).check();
  await page.getByRole("button",{name:"Generate label proposal",exact:true}).click();
  await expect(page.getByText("Schema Draft saved · Revision 1",{exact:true})).toBeVisible();
  await expect(page.getByRole("button",{name:"Save as editable Schema Draft",exact:true})).toHaveCount(0);
  if(scenario==="bbox")await page.screenshot({path:"../docs/execution/conversational-workspace/automatic-label-draft.png",fullPage:true,animations:"disabled"});
  }
  const builderPreviewPromise=page.waitForResponse(response=>response.url().includes("/builder-preview"));
  const changeCeiling=async(maximum:number)=>{
    const path=`/api/projects/${project}/conversation-call-limit`,current=await (await request.get(path)).json();
    expect((await request.post(path,{data:{id:randomUUID(),expected_revision:current.revision,maximum_calls:maximum}})).ok()).toBe(true);
  };
  if(scenario==="human-classification")await changeCeiling(0);
  await page.getByRole("button",{name:"Review Builder authorization",exact:true}).click();
  const builderPreviewResponse=await builderPreviewPromise;
  const builderPreview=await builderPreviewResponse.json();
  if(scenario==="bbox"){
    const jointRoot=new URL(builderPreviewResponse.url()).pathname.replace(/\/builder-preview$/, "");
    const before=await (await request.get(`${jointRoot}/calls`)).json();
    const query=new URLSearchParams({consent_id:randomUUID(),builder_operation_id:randomUUID(),sample_operation_id:randomUUID(),schema_id:builderPreview.selection.schema_id,schema_revision:String(builderPreview.selection.schema_revision),planner_model_id:model.id,allowed_models:JSON.stringify([`model-profile:${model.id}`])});
    const response=await request.get(`${jointRoot}/journey-preview?${query}`);expect(response.ok(),await response.text()).toBe(true);
    const joint=await response.json();
    expect(joint.estimated_cost).toBeNull();expect(joint.consent.allow_unknown_cost).toBe(false);
    expect(joint.consent.images).toHaveLength(1);expect(joint.data.models[0].destination).toContain("8796");
    expect((await request.post(`${jointRoot}/journey-consents`,{data:joint.consent})).status()).toBe(400);
    const accepted={...joint.consent,allow_unknown_cost:true};
    expect((await page.request.post(`${jointRoot}/journey-consents`,{data:accepted})).status()).toBe(403);
    expect((await request.post(`${jointRoot}/journey-consents`,{data:{...accepted,auto_publish:true}})).status()).toBe(422);
    const altered={...accepted,allowed_models:[{...accepted.allowed_models[0],binding_digest:"f".repeat(64)}]};
    expect((await request.post(`${jointRoot}/journey-consents`,{data:altered})).status()).toBe(400);
    const savedResponse=await request.post(`${jointRoot}/journey-consents`,{data:accepted});expect(savedResponse.ok(),await savedResponse.text()).toBe(true);
    const saved=await savedResponse.json();expect(saved.consent).toEqual(accepted);expect(saved.sample).toBeNull();
    expect(await (await request.post(`${jointRoot}/journey-consents`,{data:accepted})).json()).toEqual(saved);
    const savedPath=`${jointRoot}/journey-consents/${accepted.id}`;
    expect(await (await request.get(savedPath)).json()).toEqual(saved);
    expect((await request.post(`${jointRoot}/journey-consents`,{data:{...accepted,expires_at:new Date(Date.now()+30*60*1000).toISOString()}})).status()).toBe(400);
    expect((await request.get(`${jointRoot.replace(/\/tasks\/[^/]+$/,`/tasks/${randomUUID()}`)}/journey-consents/${accepted.id}`)).ok()).toBe(false);
    const revoked=await (await request.post(`${savedPath}/revoke`,{data:{}})).json();expect(revoked.revoked).toBe(true);
    expect(await (await request.post(`${jointRoot}/journey-consents`,{data:accepted})).json()).toEqual(revoked);
    expect(await (await request.get(`${jointRoot}/calls`)).json()).toEqual(before);
    expect((await (await request.get(`${jointRoot}/builder-operations`)).json()).items).toHaveLength(0);
    // Consent-only API tests never launch a planner or sample; this existing
    // workflow proceeds below under its separate explicit phase authorizations.
  }
  if(humanSchema){
    expect(builderPreview.previous_grant_id).toBeNull();expect(builderPreview.maximum_calls).toBe(8);expect(builderPreview.used_calls).toBe(0);
  }
  await page.getByRole("checkbox",{name:/Allow this bounded Builder request/}).check();
  if(scenario==="human-classification"){
    const authorization=page.getByLabel("Builder model authorization",{exact:true});
    await expect(authorization).toContainText("Project call limit exhausted");
    await expect(authorization.getByRole("button",{name:"Build Pipeline Draft",exact:true})).toBeDisabled();
    await authorization.getByRole("button",{name:"Review Project call limit",exact:true}).click();
    const ceiling=page.getByRole("region",{name:"Project call limit",exact:true});
    await expect(ceiling.getByLabel("Cumulative maximum calls",{exact:true})).toBeFocused();
    await ceiling.getByRole("button",{name:"Reload saved limit",exact:true}).click();
    await expect(ceiling).toContainText("0 cumulative maximum · Revision 2");
    await ceiling.getByLabel("Cumulative maximum calls",{exact:true}).fill("64");await ceiling.getByRole("checkbox").check();
    await ceiling.getByRole("button",{name:"Save Project limit",exact:true}).click();
    await expect(ceiling).toContainText("64 cumulative maximum · Revision 3");
    await page.getByText("Project call limit",{exact:true}).click();
    await authorization.getByRole("button",{name:"Refresh authorization and Project budget",exact:true}).click();
    await expect(authorization).toContainText("Project has 64 calls remaining");
    await page.getByRole("checkbox",{name:/Allow this bounded Builder request/}).check();
  }
  const builderReceiptPromise=page.waitForResponse(response=>response.url().endsWith("/builder-operations")&&response.request().method()==="POST");
  await page.getByRole("button",{name:"Build Pipeline Draft",exact:true}).click();
  const builderReceipt=await (await builderReceiptPromise).json();
  expect(builderReceipt.status).toBe("completed");
  const builtDraft=(await (await request.get(`/api/workflow-drafts?project_id=${project}`)).json()).drafts.find((draft:{id:string})=>draft.id===builderReceipt.evidence.draft_id);
  expect(builtDraft).toBeTruthy();
  expect(builderReceipt.evidence.draft_revision).toBe(builtDraft.revision);
  expect(builderReceipt.evidence.draft_content_hash).toBe(builtDraft.content_hash);
  if(scenario==="human-classification"){
    await expect(page.getByRole("button",{name:"Review sample authorization",exact:true})).toBeEnabled();
    const current=await (await request.get(`/api/projects/${project}/conversation-call-limit`)).json();
    await changeCeiling(current.reserved_calls);
  }
  await page.getByRole("button",{name:"Review sample authorization",exact:true}).click();
  const authorization=page.getByLabel("Sample model authorization",{exact:true});
  await expect(authorization).toContainText("Cost unknown");
  await page.getByRole("checkbox",{name:/Allow these sample images/}).check();
  const start=page.getByRole("button",{name:"Test these samples",exact:true});
  if(scenario==="human-classification"){
    await expect(authorization).toContainText("Project call limit exhausted");await expect(start).toBeDisabled();
    await authorization.getByLabel("Project budget before inference",{exact:true}).screenshot({path:"../docs/execution/conversational-workspace/sample-project-budget-exhausted.png",animations:"disabled"});
    await changeCeiling(64);
    await authorization.getByRole("button",{name:"Refresh authorization and Project budget",exact:true}).click();
    await expect(authorization).toContainText(/Project has \d+ calls remaining/);
    await page.getByRole("checkbox",{name:/Allow these sample images/}).check();
  }
  await expect(start).toBeEnabled();
  let envelope:any;
  // Lose both the acknowledged POST and its immediate receipt lookup. This is
  // genuinely unknown to the browser, not proof that execution failed.
  let loseReceipt = scenario === "bbox";
  await page.route(`**/api/projects/${project}/sample-operations/*`,async route=>{
    if(loseReceipt && route.request().method()==="GET")return route.fulfill({status:503,contentType:"application/json",body:JSON.stringify({error:"TEST receipt temporarily unavailable"})});
    return route.fallback();
  });
  await page.route(`**/api/projects/${project}/sample-operations`,async route=>{
    if(route.request().method()!=="POST")return route.continue();
    envelope=route.request().postDataJSON();
    const response=await fetchWithinMutationLimit(route);expect(response.ok(),await response.text()).toBe(true);await route.abort("failed");
  },{times:1});
  await start.click();
  if(scenario==="bbox"){
    await expect(page.getByRole("button",{name:"Retry the same sample request",exact:true})).toBeVisible();
    await expect(start).toBeDisabled({timeout:10_000});
    await page.getByLabel("Sample request outcome unknown",{exact:true}).screenshot({path:"../docs/execution/conversational-workspace/sample-outcome-unknown.png",animations:"disabled"});
    await expect.poll(async()=> (await (await request.get(`/api/projects/${project}/sample-operations/${envelope.request_id}`)).json()).status).toBe("succeeded");
    const pendingKey=`annotagent.conversation-sample:${project}:${envelope.conversation.conversation_id}:${envelope.conversation.task_id}:${envelope.draft_id}`;
    expect(await page.evaluate(key=>JSON.parse(sessionStorage.getItem(key)!),pendingKey)).toEqual(envelope);
    loseReceipt=false;
    let releaseHistory!:()=>void;
    const historyHeld=new Promise<void>(resolve=>{releaseHistory=resolve;});
    await page.route(`**/conversations/${envelope.conversation.conversation_id}/tasks/${envelope.conversation.task_id}/sample-operations`,async route=>{await historyHeld;await route.fallback();},{times:1});
    let recoveryWrites=0;
    const observeRecovery=(req:{method:()=>string;url:()=>string})=>{if(!["GET","HEAD","OPTIONS"].includes(req.method())&&req.url().includes("/api/"))recoveryWrites++;};
    page.on("request",observeRecovery);
    await page.reload();
    await expect(page.getByRole("button",{name:"Retry the same sample request",exact:true})).toBeDisabled();
    releaseHistory();
    await expect(page.getByRole("button",{name:"View sample results in canvas",exact:true})).toBeVisible();
    await expect(page.getByRole("button",{name:"Retry the same sample request",exact:true})).toHaveCount(0);
    expect(await page.evaluate(key=>sessionStorage.getItem(key),pendingKey)).toBeNull();
    expect(recoveryWrites).toBe(0);page.off("request",observeRecovery);
  }
  await expect(page.getByRole("button",{name:"View sample results in canvas",exact:true})).toBeVisible();
  expect(envelope.conversation.allow_unknown_cost).toBe(true);
  const taskRoot=`/api/projects/${project}/conversations/${envelope.conversation.conversation_id}/tasks/${envelope.conversation.task_id}`;
  const calls=await (await request.get(`${taskRoot}/calls`)).json();
  const initialReport = (await (await request.get(`/api/workflow-drafts/${envelope.draft_id}/sample-test?test_id=${envelope.request_id}`)).json()).sample_test.report;
  expect(initialReport.validation.valid).toBe(true);
  expect(initialReport.samples[0].nodes.some((node:any)=>node.metadata.model === `e2e-conversation-${transport}`)).toBe(true);
  if(humanSchema){
    const schemas=await (await request.get(`${taskRoot}/human-schema-drafts`)).json();
    expect(schemas).toHaveLength(1);expect(schemas[0].source_call_id).toBeNull();
    expect(calls.every((call:any)=>!call.evidence?.decision)).toBe(true);
    const before=await (await request.get(`${taskRoot}/budget`)).json();
    const resetAttempt=await request.post(`${taskRoot}/builder-operations`,{data:{selection:{...builderPreview.selection,operation_id:randomUUID()},previous_grant_id:null,scope_hash:builderPreview.scope_hash,expires_at:builderPreview.expires_at,allow_unknown_cost:true}});
    expect(resetAttempt.ok()).toBe(false);
    expect(await resetAttempt.text()).toContain("Task authorization changed");
    expect(await (await request.get(`${taskRoot}/budget`)).json()).toEqual(before);
    const sample=(await (await request.get(`/api/workflow-drafts/${envelope.draft_id}/sample-test?test_id=${envelope.request_id}`)).json()).sample_test;
    const candidate=sample.report.samples[0].projection.final_candidates[0] ?? sample.report.samples[0].projection.review_candidates[0].candidate;
    const tasks=await (await request.get(taskRoot.slice(0,taskRoot.lastIndexOf("/")))).json();
    const referenced={id:randomUUID(),text:"TEST 这个对象的边界需要检查，仅指当前候选",image:{image_id:sample.inputs[0].image_id,sha256:sample.inputs[0].content_hash},reference:{scope:"sample_candidate",task_id:envelope.conversation.task_id,project_schema_revision:tasks.find((task:any)=>task.input.id===envelope.conversation.task_id).input.schema_revision,draft_id:sample.draft_id,draft_revision:sample.draft_revision,sample_test_id:sample.id,candidate_id:candidate.outcome.id,source_artifact_id:candidate.source_artifact_id}};
    const messages=`/api/projects/${project}/conversations/${envelope.conversation.conversation_id}/messages`;
    const first=await request.post(messages,{data:referenced});expect(first.ok(),await first.text()).toBe(true);
    const saved=await first.json();
    expect(await (await request.post(messages,{data:referenced})).json()).toEqual(saved);
    for(const changed of [{candidate_id:"TEST-intermediate"},{source_artifact_id:randomUUID()},{draft_revision:sample.draft_revision+1},{task_id:randomUUID()},{project_schema_revision:"f".repeat(64)}]){
      expect((await request.post(messages,{data:{...referenced,id:randomUUID(),reference:{...referenced.reference,...changed}}})).ok()).toBe(false);
    }
    expect((await request.post(messages,{data:{...referenced,id:randomUUID(),image:null}})).ok()).toBe(false);
    expect((await request.post(messages,{data:{...referenced,reference:{...referenced.reference,candidate_id:"changed"}}})).ok()).toBe(false);
    expect((await (await request.get(messages)).json()).filter((message:any)=>message.input.id===referenced.id)).toEqual([saved]);
    expect(await (await request.get(`${taskRoot}/budget`)).json()).toEqual(before);
    const widened=await request.post(taskRoot.slice(0,taskRoot.lastIndexOf("/")),{data:{id:randomUUID(),source_message_id:referenced.id,schema_revision:referenced.reference.project_schema_revision}});
    expect(widened.ok()).toBe(false);expect(await widened.text()).toContain("candidate-scoped message");
  }
  const processingPreviewResponse = await request.get(`/api/projects/${project}/processing-preview?draft_id=${envelope.draft_id}&sample_test_id=${envelope.request_id}`);
  expect(processingPreviewResponse.ok(), await processingPreviewResponse.text()).toBe(true);
  const processingPreview = await processingPreviewResponse.json();
  expect(processingPreview.conversation.conversation_id).toBe(envelope.conversation.conversation_id);
  expect(processingPreview.conversation.task_id).toBe(envelope.conversation.task_id);
  expect(processingPreview.conversation.schema.definition.task.kind).toBe(kind === "bbox" ? "bounding_box" : "classification");
  expect(processingPreview.goal.goal).toBe(processingPreview.conversation.schema.definition.goal);
  expect(await (await request.get(`${taskRoot}/processing-operations`)).json()).toEqual([]);
  expect((await request.get(taskRoot.replace(envelope.conversation.task_id, randomUUID()) + "/processing-operations")).ok()).toBe(false);
  expect(envelope.conversation.human_review).toBe(true);
  if(requiresReview){
  await expect.poll(async()=> (await (await request.get(`${taskRoot}/human-requests`)).json()).length).toBe(1);
  const initialAutomatic=(await (await request.get(`${taskRoot}/human-requests`)).json())[0];
  expect(initialAutomatic.input.reason_code).toBe("terminal_result_requires_review");
  expect(initialAutomatic.status).toBe("pending");
  await expect(page.getByText(initialAutomatic.input.question,{exact:true})).toBeVisible();
  // Retain explicit manual-request API coverage below without leaving a task budget blocker.
  expect((await request.post(`${taskRoot}/human-requests/${initialAutomatic.input.id}/cancel`)).ok()).toBe(true);
  } else {
    await expect.poll(async()=> (await (await request.get(`/api/projects/${project}/sample-operations/${envelope.request_id}`)).json()).assistance?.status).toBe("completed");
    expect(await (await request.get(`${taskRoot}/human-requests`)).json()).toEqual([]);
  }
  expect(calls.some((call:any)=>call.evidence?.phase==="sample_inference" && call.status==="completed")).toBe(true);
  if (requiresReview) {
    const saved = await (await request.get(`/api/workflow-drafts/${envelope.draft_id}/sample-test?test_id=${envelope.request_id}`)).json();
    const projection = saved.sample_test.report.samples[0].projection;
    expect(projection.final_candidates).toHaveLength(0);
    expect(projection.review_candidates).toHaveLength(1);
    expect(calls.filter((call:any) => call.evidence?.phase === "sample_inference")).toHaveLength(1);
  }
  const starts:string[]=[];
  page.on("request",request=>{if(request.method()==="POST")starts.push(request.url());});
  await page.reload();
  await page.getByRole("button",{name:"View sample results in canvas",exact:true}).click();
  await expect(page.getByLabel("Saved sample results",{exact:true})).toBeVisible();
  await expect(page.locator(".conversation-sample-canvas .annotation-canvas image")).toBeVisible();
  expect(new URL(page.url()).searchParams.get("test")).toBe(envelope.request_id);
  await page.reload();
  await expect(page.getByLabel("Saved sample results",{exact:true})).toBeVisible();
  expect(starts).toEqual([]);
  await page.getByRole("button",{name:"Annotation list · 1",exact:true}).click();
  await page.getByLabel("Annotations on canvas",{exact:true}).getByRole("button").first().click();
  if(kind==="classification")await page.getByLabel("Correct label",{exact:true}).fill("室外");
  else{await page.getByText("Result needs attention",{exact:true}).click();await page.getByRole("spinbutton",{name:"width",exact:true}).fill("0.15");}
  page.once("dialog",dialog=>dialog.dismiss());
  await page.getByRole("button",{name:"Back to project",exact:true}).click();
  expect(new URL(page.url()).searchParams.get("test")).toBe(envelope.request_id);
  if(kind==="classification")await expect(page.getByLabel("Correct label",{exact:true})).toHaveValue("室外");
  else await expect(page.getByRole("spinbutton",{name:"width",exact:true})).toHaveValue("0.15");
  await page.getByRole("button",{name:"Save sample feedback",exact:true}).click();
  await expect(page.getByText("Sample feedback saved",{exact:true})).toBeVisible();
  await page.reload();
  if(humanSchema){
    await expect(page.getByRole("list",{name:"Saved messages",exact:true})).toContainText("This is not a project-wide goal.");
    await expect(page.getByRole("button",{name:"Use message 2 as annotation goal",exact:true})).toHaveCount(0);
  }
  if(kind==="classification")await expect(page.getByLabel("Image classification results",{exact:true})).toContainText("室外");
  else {
    const box = page.locator(".conversation-sample-canvas .annotation-shape rect.aa-annotation-shape");
    await expect(box).toHaveCount(1);
    // Normalized geometry round-trips through Rust f32 before becoming SVG pixels.
    await expect.poll(async () => Number(await box.getAttribute("width"))).toBeCloseTo(96, 3);
  }
  expect(starts.filter(url=>!url.endsWith("/feedback"))).toEqual([]);
  expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual(calls);
  await page.locator(".conversation-image-panel").evaluate(element=>element.scrollTop=0);
  await page.screenshot({path:`../docs/execution/conversational-workspace/sample-${scenario}.png`,fullPage:true});
  await page.setViewportSize({width:390,height:844});
  await page.getByRole("button",{name:/Images \(/}).click();
  await expect(page.getByLabel("Saved sample results",{exact:true})).toBeVisible();
  await page.screenshot({path:`../docs/execution/conversational-workspace/sample-${kind}-390.png`,fullPage:true});
  // A persisted request opens the existing canvas and saves through its atomic answer API.
  const imageId=new URL(page.url()).searchParams.get("image")!;
  const feedbackPath=`/api/workflow-sample-tests/${envelope.request_id}/images/${imageId}/feedback`;
  const revisions=(await (await request.get(feedbackPath)).json()).revisions;
  const previous=revisions.at(-1);
  const savedTest=(await (await request.get(`/api/workflow-drafts/${envelope.draft_id}/sample-test?test_id=${envelope.request_id}`)).json()).sample_test;
  const human={id:randomUUID(),task_id:envelope.conversation.task_id,conversation_id:envelope.conversation.conversation_id,sample_test_id:envelope.request_id,image_id:imageId,content_hash:savedTest.inputs.find((input:any)=>input.image_id===imageId).content_hash,outcome_id:previous.outcome_id,expected_feedback_sequence:previous.sequence,reason_code:"poor_boundary",question:"TEST: confirm this correction",resume_checkpoint_ref:randomUUID()};
  const humanRoot=`${taskRoot}/human-requests`;
  expect((await request.post(humanRoot,{data:{...human,task_id:randomUUID()}})).ok()).toBe(false);
  expect((await request.post(humanRoot,{data:human,headers:{Origin:"https://foreign.invalid"}})).status()).toBe(403);
  const created=await request.post(humanRoot,{data:human});
  expect(created.ok(),await created.text()).toBe(true);
  expect((await created.json()).status).toBe("pending");
  expect((await request.post(humanRoot,{data:human})).ok()).toBe(true);
  await page.setViewportSize({width:1280,height:800});
  await page.getByRole("button",{name:"Refresh requests",exact:true}).click();
  if(scenario==="bbox"){
    const originalTask=new URL(page.url()).searchParams.get("task");
    await page.getByLabel("Your message",{exact:true}).fill("TEST independent goal without changing the original correction");
    await page.getByRole("button",{name:"Save message",exact:true}).click();
    const newGoal=page.getByRole("list",{name:"Saved messages",exact:true}).locator("li").filter({hasText:"TEST independent goal without changing the original correction"});
    await newGoal.getByRole("button",{name:/Use message .* as annotation goal/}).click();
    await expect.poll(()=>new URL(page.url()).searchParams.get("task")).not.toBe(originalTask);
    const help=page.getByRole("region",{name:"Human requests",exact:true});
    await expect(help.getByText("No outstanding visual requests for this goal.",{exact:true})).toBeVisible();
    await expect(help.getByText(human.question,{exact:true})).not.toBeVisible();
    const history=help.locator("details");
    await expect(history.locator("summary")).toContainText("awaiting help in other goals");
    await history.locator("summary").focus();await page.keyboard.press("Enter");
    await expect(help.getByText(human.question,{exact:true})).toBeVisible();
    await help.evaluate(element=>element.scrollIntoView({block:"start"}));
    await page.screenshot({path:"../docs/execution/conversational-workspace/request-task-history.png",fullPage:true});
    const operationPath=`/api/projects/${project}/sample-operations/${human.sample_test_id}`;
    const operationSnapshot=await (await request.get(operationPath)).json();
    const independentTask=new URL(page.url()).searchParams.get("task")!;
    for(const lateError of [false,true]){
    let release!:()=>void;
    const gate=new Promise<void>(resolve=>{release=resolve;});
    let intercepted=false;
    await page.route(`**${operationPath}`,async route=>{intercepted=true;await gate;await route.fulfill(lateError ? {status:503,json:{error:"TEST stale request lookup failed"}} : {json:operationSnapshot});},{times:1});
    await help.locator("article").filter({hasText:human.question}).getByRole("button",{name:"Open requested result",exact:true}).click();
    await expect.poll(()=>intercepted).toBe(true);
    const taskOnly=new URL(page.url());taskOnly.searchParams.set("task",lateError ? independentTask : human.task_id);
    await page.evaluate(url=>{history.pushState({},"",url);window.dispatchEvent(new PopStateEvent("popstate"));},taskOnly.toString());
    await expect(page.getByRole("button",{name:lateError ? /Use message 2 as annotation goal/ : /Use message 1 as annotation goal/})).toHaveAttribute("aria-pressed","true");
    const response=page.waitForResponse(value=>value.url().endsWith(operationPath));release();await response;
    await page.evaluate(()=>new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve()))));
    expect(page.url()).toBe(taskOnly.toString());
    await expect(page.getByRole("alert").filter({hasText:"TEST stale request lookup failed"})).toHaveCount(0);
    }
    const restoredHistory=help.locator("details");
    if(!await restoredHistory.evaluate(element=>(element as HTMLDetailsElement).open))await restoredHistory.locator("summary").click();
  }
  await page.locator("article").filter({hasText:human.question}).getByRole("button",{name:"Open requested result",exact:true}).click();
  await expect(page.getByRole("button",{name:"Submit correction",exact:true})).toBeVisible();
  expect(new URL(page.url()).searchParams.get("request")).toBe(human.id);
  expect(new URL(page.url()).searchParams.get("task")).toBe(human.task_id);
  if(scenario==="bbox"){
    const deferralPath=`${humanRoot}/${human.id}/deferral`;
    const beforeDeferral=await (await request.get(`${taskRoot}/budget`)).json();
    await page.route(`**${deferralPath}`,async route=>{const response=await fetchWithinMutationLimit(route);expect(response.ok(),await response.text()).toBe(true);await route.abort("failed");},{times:1});
    const help=page.getByRole("region",{name:"Human requests",exact:true}).locator("article").filter({hasText:human.question});
    await help.getByRole("button",{name:"Do this later",exact:true}).click();
    await expect(page.getByRole("alert").last()).toBeVisible();
    const afterLost=(await (await request.get(humanRoot)).json()).find((value:any)=>value.input.id===human.id);
    expect(afterLost.deferred).toBe(true);expect(afterLost.deferral_revision).toBe(1);
    await help.getByRole("button",{name:"Do this later",exact:true}).click();
    await expect(page.getByRole("region",{name:"Deferred sample request",exact:true})).toBeVisible();
    await page.reload();
    await expect(page.getByRole("region",{name:"Deferred sample request",exact:true})).toBeVisible();
    await expect(page.getByRole("button",{name:"Submit correction",exact:true})).toHaveCount(0);
    await expect.poll(async()=>page.getByRole("region",{name:"Deferred sample request",exact:true}).locator("rect.aa-annotation-shape").evaluate(rect=>Number(rect.getAttribute("width"))/(rect.ownerSVGElement?.viewBox.baseVal.width??1))).toBeCloseTo(0.15,5);
    const lateAnswer=await request.post(`${humanRoot}/${human.id}/answer`,{data:{answer:{...previous,revision_id:randomUUID(),sequence:human.expected_feedback_sequence+1}}});
    expect(lateAnswer.ok()).toBe(false);expect(await lateAnswer.text()).toContain("deferred");
    await page.screenshot({path:"../docs/execution/conversational-workspace/deferred-request.png",fullPage:true});
    await help.getByRole("button",{name:"Reopen request",exact:true}).click();
    await expect(page.getByRole("button",{name:"Submit correction",exact:true})).toBeVisible();
    expect(await (await request.get(`${taskRoot}/budget`)).json()).toEqual(beforeDeferral);
    const resumed=(await (await request.get(humanRoot)).json()).find((value:any)=>value.input.id===human.id);
    expect(resumed.deferred).toBe(false);expect(resumed.deferral_revision).toBe(2);
  }
  await page.reload();
  await expect(page.getByRole("button",{name:"Submit correction",exact:true})).toBeVisible();
  await page.getByLabel("Correct label",{exact:true}).fill(kind==="classification" ? "室内" : "cup");
  if(kind==="bbox")await page.getByRole("spinbutton",{name:"width",exact:true}).fill("0.12");
  await page.locator(".conversation-panel").evaluate(element=>{const card=element.querySelector<HTMLElement>('[aria-label="Human requests"]');if(card)element.scrollTop=card.offsetTop-element.getBoundingClientRect().top;});
  await page.locator(".conversation-image-panel").evaluate(element=>element.scrollTop=0);
  await page.screenshot({path:`../docs/execution/conversational-workspace/human-request-${kind}.png`,fullPage:true});
  await page.setViewportSize({width:390,height:844});
  await page.getByRole("button",{name:/Images \(/}).click();
  await expect(page.getByRole("button",{name:"Submit correction",exact:true})).toBeVisible();
  await page.screenshot({path:`../docs/execution/conversational-workspace/human-request-${kind}-390.png`,fullPage:true});
  await page.setViewportSize({width:1280,height:800});
  let answer:any;
  await page.route(`**${humanRoot}/${human.id}/answer`,async route=>{answer=route.request().postDataJSON().answer;const response=await fetchWithinMutationLimit(route);expect(response.ok(),await response.text()).toBe(true);await route.abort("failed");},{times:1});
  await page.getByRole("button",{name:"Submit correction",exact:true}).click();
  await expect(page.locator(".sample-confirm-action [role=alert]")).toBeVisible();
  if(kind==="bbox")await expect(page.getByRole("spinbutton",{name:"width",exact:true})).toHaveValue("0.12");
  else await expect(page.getByLabel("Correct label",{exact:true})).toHaveValue("室内");
  await page.getByRole("button",{name:"Submit correction",exact:true}).click();
  await expect(page.getByText("Correction saved and revision Draft prepared without model calls. Later repairs and tests have separate operation records.",{exact:true})).toBeVisible();
  expect(answer.outcome_id).toBe(human.outcome_id);
  expect(answer.sequence).toBe(previous.sequence+1);
  expect((await request.post(`${humanRoot}/${human.id}/answer`,{data:{answer}})).ok()).toBe(true);
  expect((await request.post(`${humanRoot}/${human.id}/answer`,{data:{answer:{...answer,note:"changed retry"}}})).ok()).toBe(false);
  expect((await (await request.get(feedbackPath)).json()).revisions).toHaveLength(revisions.length+1);
  await page.reload();
  const restored=(await (await request.get(humanRoot)).json()).filter((value:any)=>value.input.id===human.id);
  expect(restored).toHaveLength(1);
  expect(restored[0].answer.revision_id).toBe(answer.revision_id);
  expect(restored[0].status).toBe("applied");
  expect(restored[0].resume_draft_id).toBe(human.resume_checkpoint_ref);
  expect((await request.post(`${humanRoot}/${human.id}/resume`)).ok()).toBe(true);
  const evidence=await (await request.get(`/api/projects/${project}/sample-plan-copies/${human.resume_checkpoint_ref}`)).json();
  expect(evidence.feedback).toHaveLength(1);
  expect(evidence.feedback[0].revision_id).toBe(answer.revision_id);
  const cancelRequest={...human,id:randomUUID(),expected_feedback_sequence:answer.sequence};
  expect((await request.post(humanRoot,{data:cancelRequest})).ok()).toBe(true);
  const cancelled=await request.post(`${humanRoot}/${cancelRequest.id}/cancel`);
  expect(cancelled.ok()).toBe(true);
  expect((await cancelled.json()).status).toBe("cancelled");
  expect((await request.post(`${humanRoot}/${cancelRequest.id}/cancel`)).ok()).toBe(true);
  expect((await request.post(`${humanRoot}/${cancelRequest.id}/answer`,{data:{answer:{...answer,revision_id:randomUUID(),sequence:answer.sequence+1}}})).ok()).toBe(false);
  expect((await (await request.get(feedbackPath)).json()).revisions).toHaveLength(revisions.length+1);
  expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual(calls);
  const repairCard=page.getByRole("region",{name:"Repair annotation pipeline",exact:true});
  await expect(repairCard).toBeVisible();
  await repairCard.getByRole("button",{name:"Review Builder authorization",exact:true}).click();
  await expect(repairCard).toContainText("scoped human feedback and terminal result metadata");
  await expect(repairCard).toContainText("No image pixels");
  await repairCard.getByRole("checkbox",{name:/Allow this bounded Builder request/}).check();
  let repairConsent:any;
  page.on("request",req=>{if(req.method()==="POST" && req.url().endsWith(`${taskRoot}/builder-operations`))repairConsent=req.postDataJSON();});
  await repairCard.getByRole("button",{name:"Build Pipeline Draft",exact:true}).click();
  await expect(repairCard.getByText("Builder outcome saved",{exact:true})).toBeVisible({timeout:30_000});
  expect(repairConsent.selection.repair_request_id).toBe(human.id);
  expect(repairConsent.repair.draft_id).toBe(human.resume_checkpoint_ref);
  const repairCalls=await (await request.get(`${taskRoot}/calls`)).json();
  expect(repairCalls.length).toBeGreaterThan(calls.length);
  expect(repairCalls.length-calls.length).toBeLessThanOrEqual(8);
  const history=await (await request.get(`${taskRoot}/builder-operations`)).json();
  const repaired=history.items.find((item:any)=>item.operation.id===repairConsent.selection.operation_id);
  expect(repaired.session.working_draft.draft_id).toBe(human.resume_checkpoint_ref);
  expect(repaired.session.working_draft.build_mode.kind).toBe("repair_draft");
  expect((await request.post(`${taskRoot}/builder-operations`,{data:repairConsent})).ok()).toBe(true);
  await page.reload();
  await expect(repairCard.getByText("Builder outcome saved",{exact:true})).toBeVisible();
  expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual(repairCalls);
  const savedRepair=repairCard.locator(".conversation-completed-stage > summary");
  if(await savedRepair.isVisible())await savedRepair.click();
  await repairCard.getByRole("heading",{name:"Revise the plan from your correction",exact:true}).evaluate(element=>element.scrollIntoView({block:"start"}));
  await page.screenshot({path:`../docs/execution/conversational-workspace/repair-${kind}.png`,fullPage:true});
  await repairCard.getByRole("button",{name:"Review sample authorization",exact:true}).click();
  await repairCard.getByRole("checkbox",{name:/Allow these sample images/}).check();
  await repairCard.getByRole("button",{name:"Test these samples",exact:true}).click();
  await expect(repairCard.getByRole("button",{name:"View sample results in canvas",exact:true})).toBeVisible();
  await repairCard.getByRole("button",{name:"View sample results in canvas",exact:true}).click();
  await expect(page).toHaveURL(new RegExp(`draft=${human.resume_checkpoint_ref}`));
  const comparisonUrl=page.url();
  expect(new URL(comparisonUrl).searchParams.get("test")).not.toBe(human.sample_test_id);
  expect(new URL(comparisonUrl).searchParams.get("request")).toBe(human.id);
  expect(new URL(comparisonUrl).searchParams.get("task")).toBe(human.task_id);
  const afterComparison=await (await request.get(`${taskRoot}/calls`)).json();
  expect(afterComparison.length).toBeGreaterThan(repairCalls.length);
  await page.reload();
  await expect(page).toHaveURL(comparisonUrl);
  await expect(page.getByRole("heading",{name:"synthetic-robocup.png",exact:true})).toBeVisible();
  expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual(afterComparison);
  const origin=page.getByRole("complementary",{name:"Sample origin",exact:true});
  await expect(origin).toContainText("improvement has not been established");
  // A later whole-image note must not replace the request's selected subject on return.
  expect((await request.post(feedbackPath,{data:{...answer,revision_id:randomUUID(),sequence:answer.sequence+1,outcome_id:null,corrected_value:null,corrected_label:null,reason:"cannot_judge",note:"TEST unrelated whole-image note"}})).ok()).toBe(true);
  await origin.getByRole("button",{name:"Return to original correction",exact:true}).click();
  await expect(page).toHaveURL(new RegExp(`test=${human.sample_test_id}`));
  expect(new URL(page.url()).searchParams.get("image")).toBe(human.image_id);
  await expect(page.getByText("Correction saved and revision Draft prepared without model calls. Later repairs and tests have separate operation records.",{exact:true})).toBeVisible();
  await expect(page.getByLabel("Result to inspect",{exact:true})).toHaveValue(human.outcome_id);
  await page.goBack();
  await expect(page).toHaveURL(comparisonUrl);
  await expect(origin).toBeVisible();
  await origin.evaluate(element=>element.scrollIntoView({block:"start"}));
  await page.screenshot({path:`../docs/execution/conversational-workspace/comparison-origin-${kind}.png`,fullPage:true});
  expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual(afterComparison);
  if(kind==="bbox"){
  const comparisonId=new URL(comparisonUrl).searchParams.get("test");
  await expect.poll(async()=> (await (await request.get(humanRoot)).json()).filter((value:any)=>value.input.sample_test_id===comparisonId).length).toBe(1);
  const automatic=(await (await request.get(humanRoot)).json()).find((value:any)=>value.input.sample_test_id===comparisonId);
  expect(automatic.status).toBe("pending");
  const automaticCard=page.locator("article").filter({hasText:automatic.input.question}).filter({has:page.getByText("pending",{exact:true})});
  await expect(automaticCard).toBeVisible();
  await automaticCard.getByRole("button",{name:"Open requested result",exact:true}).click();
  await expect(page).toHaveURL(new RegExp(`request=${automatic.input.id}`));
  await expect(page.getByRole("button",{name:"Submit correction",exact:true})).toBeVisible();
  await page.getByRole("spinbutton",{name:"width",exact:true}).fill("0.14");
  await automaticCard.evaluate(element=>element.scrollIntoView({block:"start"}));
  await page.screenshot({path:"../docs/execution/conversational-workspace/automatic-human-request.png",fullPage:true});
  await page.getByRole("button",{name:"Submit correction",exact:true}).click();
  await expect.poll(async()=> (await (await request.get(humanRoot)).json()).find((value:any)=>value.input.id===automatic.input.id)?.status).toBe("applied");
  expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual(afterComparison);
  await page.reload();
  await expect(page.getByText("Correction saved and revision Draft prepared without model calls. Later repairs and tests have separate operation records.",{exact:true})).toBeVisible();
  expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual(afterComparison);
  }
  // Exercise the existing formal service using only this isolated HTTP TEST transport.
  const selection={draft_id:human.resume_checkpoint_ref,sample_test_id:new URL(comparisonUrl).searchParams.get("test"),limit:1};
  const approvalResponse=await request.get(`/api/projects/${project}/processing-preview?${new URLSearchParams(selection as any)}`);
  expect(approvalResponse.ok(),await approvalResponse.text()).toBe(true);
  const approval=await approvalResponse.json();
  const budgetBefore=await (await request.get(`${taskRoot}/budget`)).json();
  expect(budgetBefore.total_reserved_calls).toBe(afterComparison.length);
  await page.getByRole("button",{name:"Review dataset processing",exact:true}).click();
  const confirmCard=page.getByRole("region",{name:"Processing confirmation",exact:true});
  await expect(confirmCard).toContainText("prior usage is not reset");
  await expect(confirmCard).toContainText("Cost is unknown");
  await expect(confirmCard.getByRole("button",{name:"Confirm and start processing",exact:true})).toBeDisabled();
  await expect(confirmCard).toContainText(`Project conversation history: ${approval.task_budget.project_reserved_calls} calls reserved`);
  await confirmCard.getByRole("checkbox",{name:"I authorize this image, model and call-budget scope",exact:true}).check();
  if(kind==="bbox"){
    await page.setViewportSize({width:390,height:844});
    await page.getByRole("button",{name:"Conversation",exact:true}).click();
    const authorizationCheckbox=confirmCard.getByRole("checkbox",{name:"I authorize this image, model and call-budget scope",exact:true});
    await authorizationCheckbox.scrollIntoViewIfNeeded();
    await expect(authorizationCheckbox).toBeChecked();
    await expect.poll(()=>page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1)).toBe(true);
    await page.screenshot({path:"../docs/execution/conversational-workspace/processing-confirm-390.png",fullPage:true,animations:"disabled"});
    await page.setViewportSize({width:1280,height:800});
  }
  await confirmCard.evaluate(element=>element.scrollIntoView({block:"start"}));
  await page.screenshot({path:`../docs/execution/conversational-workspace/processing-confirm-${scenario}.png`,fullPage:true,animations:"disabled"});
  let confirmation:any;
  await page.route(`**/api/projects/${project}/processing-operations`,async route=>{
    confirmation=route.request().postDataJSON();
    const response=await fetchWithinMutationLimit(route);
    expect(response.ok(),await response.text()).toBe(true);
    await route.abort("failed");
  },{times:1});
  await confirmCard.getByRole("button",{name:"Confirm and start processing",exact:true}).click();
  await expect.poll(()=>confirmation?.request_id).toBeTruthy();
  await expect(page).toHaveURL(new RegExp(`processing=${confirmation.request_id}`));
  await expect(confirmCard.getByRole("button",{name:"Retry this confirmed action",exact:true})).toBeEnabled({timeout:50_000});
  await page.reload();
  await expect(confirmCard.getByRole("heading",{name:"Processing started",exact:true})).toBeVisible();
  const started=await (await request.get(`/api/projects/${project}/processing-operations/${confirmation.request_id}`)).json();
  expect(started.phase,JSON.stringify(started)).toBe("started");
  const duplicate=await (await request.post(`/api/projects/${project}/processing-operations`,{data:confirmation})).json();
  expect(duplicate.batch_id).toBe(started.batch_id);
  const operations=await (await request.get(`${taskRoot}/processing-operations`)).json();
  expect(operations).toHaveLength(1);
  expect(operations[0].authorization.conversation.schema.id).toBe(approval.conversation.schema.id);
  await expect.poll(async()=> (await (await request.get(`/api/batches/${started.batch_id}`)).json()).batch.images[0].execution_status).toBe(requiresReview ? "awaiting_review" : "completed");
  const frozenBatch=(await (await request.get(`/api/batches/${started.batch_id}`)).json()).checkpoint.batch;
  expect(frozenBatch.workflow_snapshot.settings.provider.max_retries).toBe(0);
  const budgetAfter=await (await request.get(`${taskRoot}/budget`)).json();
  expect(budgetAfter.planning_reserved_calls).toBe(budgetBefore.planning_reserved_calls);
  expect(budgetAfter.processing_authorized_calls).toBe(approval.maximum_model_calls);
  expect(budgetAfter.processing_reserved_calls).toBe(1);
  expect(budgetAfter.project_reserved_calls).toBe(budgetAfter.total_reserved_calls);
  expect(budgetAfter.project_authorized_calls).toBe(budgetAfter.total_authorized_calls);
  if(scenario==="human-classification"){
    const ceiling=await (await request.get(`/api/projects/${project}/conversation-call-limit`)).json();
    expect(ceiling.maximum_calls).toBe(64);expect(ceiling.reserved_calls).toBe(budgetAfter.project_reserved_calls);
  }
  expect(budgetAfter.total_reserved_calls).toBe(budgetBefore.total_reserved_calls+1);
  expect(budgetAfter.total_authorized_calls).toBe(budgetBefore.total_authorized_calls+approval.maximum_model_calls);
  await page.reload();
  const processingCard=page.getByRole("region",{name:"Saved processing tasks",exact:true});
  const liveStatus=page.getByRole("region",{name:"Current processing status",exact:true});
  await expect(liveStatus.getByText(`Current status: ${requiresReview ? "awaiting review" : "completed"}`,{exact:true})).toBeVisible();
  await expect(liveStatus.getByRole("button",{name:"Resume",exact:true})).toHaveCount(0);
  await expect(liveStatus.getByRole("button",{name:"Cancel processing",exact:true})).toHaveCount(0);
  if(scenario==="bbox") await liveStatus.screenshot({path:"../docs/execution/conversational-workspace/processing-current-status.png",animations:"disabled"});
  await expect(processingCard.getByRole("button",{name:"Open processing results",exact:true})).toBeVisible();
  expect(new URL(page.url()).searchParams.get("test")).toBe(selection.sample_test_id);
  await processingCard.evaluate(element=>element.scrollIntoView({block:"start"}));
  await page.screenshot({path:`../docs/execution/conversational-workspace/processing-linked-${scenario}.png`,fullPage:true});
  const savedWorkspaceUrl=page.url();
  await processingCard.getByRole("button",{name:"Open processing results",exact:true}).click();
  await expect(page).toHaveURL(new RegExp(`batch=${started.batch_id}`));
  await expect(page.getByRole("region",{name:"Processing results",exact:true})).toBeVisible();
  expect(new URL(page.url()).pathname).toBe(`/projects/${project}/work`);
  expect(new URL(page.url()).searchParams.get("test")).toBe(selection.sample_test_id);
  const formalResults=page.getByRole("region",{name:"Processing results",exact:true});
  await formalResults.getByRole("button",{name:"Show original",exact:true}).click();
  await expect(page).toHaveURL(/result_view=original/);
  await page.reload();
  await expect(formalResults.getByRole("button",{name:"Show results",exact:true})).toBeVisible();
  expect(new URL(page.url()).searchParams.get("result_image")).toBeTruthy();
  await formalResults.getByRole("button",{name:"Show results",exact:true}).click();
  const formalUrl=page.url();
  if(requiresReview) {
    await formalResults.getByRole("button",{name:"Review this image",exact:true}).click();
    await expect(page).toHaveURL(new RegExp(`/projects/${project}/review/`));
    if(requiresReview) {
      const reviewUrl=page.url();
      const reviewId=new URL(reviewUrl).pathname.split("/").pop()!;
      await page.route(`**/api/projects/${project}/reviews/${reviewId}/accept-and-next`,route=>route.fulfill({status:503,json:{error:"TEST decision save unavailable"}}),{times:1});
      await page.getByRole("button",{name:"Accept and next",exact:true}).click();
      await expect(page.getByRole("alert").filter({hasText:"TEST decision save unavailable"})).toBeVisible();
      expect(page.url()).toBe(reviewUrl);
      expect((await (await request.get(`/api/projects/${project}/reviews/${reviewId}`)).json()).annotation.review_status).not.toBe("human_accepted");
      const beforeEdit=await (await request.get(`/api/projects/${project}/reviews/${reviewId}`)).json();
      if(kind==="bbox") await page.getByRole("button",{name:"Move box with arrow keys",exact:true}).press("ArrowRight");
      else {
        await page.getByRole("button",{name:/^Edit E$/}).click();
        await page.getByLabel("Categories (comma-separated)",{exact:true}).fill(beforeEdit.annotation.value.labels[0]==="室内" ? "室外" : "室内");
        await page.getByRole("combobox",{name:"Correction reason",exact:true}).selectOption({label:"Wrong label"});
      }
      await page.route(`**/api/annotations/${reviewId}`,route=>route.fulfill({status:503,json:{error:"TEST revision save unavailable"}}),{times:1});
      await page.getByRole("button",{name:"Save changes",exact:true}).click();
      await expect(page.getByRole("alert").filter({hasText:"TEST revision save unavailable"})).toBeVisible();
      await expect(page.getByRole("button",{name:"Save changes",exact:true})).toBeVisible();
      expect((await (await request.get(`/api/projects/${project}/reviews/${reviewId}`)).json()).annotation.value).toEqual(beforeEdit.annotation.value);
      await page.getByRole("button",{name:"Accept and next",exact:true}).click();
      await expect(page.getByRole("heading",{name:"Review complete",exact:true})).toBeVisible();
      const accepted=await (await request.get(`/api/projects/${project}/reviews/${reviewId}`)).json();
      expect(accepted.annotation?.review_status,JSON.stringify(accepted)).toBe("human_accepted");
      expect(accepted.annotation.value).not.toEqual(beforeEdit.annotation.value);
      await page.reload();
      await expect(page.getByRole("heading",{name:"Review complete",exact:true})).toBeVisible();
      const readiness=await (await request.get(`/api/projects/${project}/export-readiness`)).json();
      expect(readiness.accepted_annotations).toBe(1);
      expect(readiness.unresolved_reviews).toBe(0);
      expect(readiness.ready,JSON.stringify(readiness)).toBe(true);
      await page.getByRole("button",{name:"Continue to export",exact:true}).click();
      const exportResponse=page.waitForResponse(response=>response.url().includes(`/api/projects/${project}/export`) && response.request().method()==="POST");
      await page.getByRole("button",{name:/^Export .+ dataset$/}).click();
      const delivered=await (await exportResponse).json();
      expect(delivered.report.exported_count,JSON.stringify(delivered)).toBe(1);
      expect(delivered.report.output_files.length).toBeGreaterThan(0);
      // Real export file in the isolated server workspace, not a mocked download/report.
      const exported=JSON.parse(readFileSync(delivered.report.output_files.find((path:string)=>path.endsWith("annotagent-native.json")),"utf8"));
      expect(exported.project.annotations).toHaveLength(1);
      expect(exported.project.annotations[0].id).toBe(reviewId);
      expect(exported.project.annotations[0].value).toEqual(accepted.annotation.value);
      expect(exported.project.annotations[0].review_status).toBe("human_accepted");
      await expect(page.getByRole("heading",{name:"Dataset exported successfully",exact:true})).toBeVisible();
      await page.screenshot({path:`../docs/execution/conversational-workspace/formal-export-${kind}.png`,fullPage:true,animations:"disabled"});
      await page.goBack();
      await expect(page).toHaveURL(reviewUrl);
    }
  } else {
    await formalResults.getByRole("button",{name:"Export confirmed results",exact:true}).click();
    await expect(page).toHaveURL(new RegExp(`/projects/${project}/export`));
  }
  await page.reload();
  await page.getByRole("button",{name:"Back to annotation workspace",exact:true}).click();
  await expect(page).toHaveURL(formalUrl);
  await expect(formalResults).toBeVisible();
  await page.screenshot({path:`../docs/execution/conversational-workspace/processing-results-${scenario}.png`,fullPage:true,animations:"disabled"});
  const foreignUrl=new URL(page.url());foreignUrl.searchParams.set("batch",crypto.randomUUID());
  await page.goto(foreignUrl.toString());
  await expect(page.getByRole("alert").filter({hasText:"This Batch is not linked to this conversation"})).toBeVisible();
  await expect(page.getByRole("region",{name:"Processing results",exact:true})).toHaveCount(0);
  await page.goto(formalUrl);
  await page.getByRole("button",{name:"Return to sample canvas",exact:true}).click();
  await expect(page).toHaveURL(savedWorkspaceUrl);
  await expect(page.getByRole("region",{name:"Saved processing tasks",exact:true})).toBeVisible();
  expect(await (await request.get(`${taskRoot}/processing-operations`)).json()).toHaveLength(1);
  if(scenario==="classification") {
    // Browser-only transport fault/state harness. The real TEST Batch above is complete;
    // no mutations are sent to it and this is not executor-control evidence.
    const batchResponse=await (await request.get(`/api/batches/${started.batch_id}`)).json();
    let presentedStatus="running";
    let failPause=true;
    const controlRequests:string[]=[];
    await page.route(`**/api/batches/${started.batch_id}`,route=>route.fulfill({json:{...batchResponse,batch:{...batchResponse.batch,status:presentedStatus}}}));
    await page.route(`**/api/batches/${started.batch_id}/*`,async route=>{
      const action=route.request().url().split("/").pop()!;
      controlRequests.push(action);
      if(action==="pause" && failPause){failPause=false;await route.fulfill({status:503,json:{error:"TEST pause unavailable"}});return;}
      presentedStatus=action==="pause" ? "paused" : action==="resume" ? "running" : "cancelled";
      await route.fulfill({json:{status:presentedStatus}});
    });
    await page.reload();
    await expect(liveStatus.getByRole("button",{name:"Pause",exact:true})).toBeVisible();
    expect(controlRequests).toEqual([]);
    await liveStatus.getByRole("button",{name:"Pause",exact:true}).click();
    await expect(liveStatus.getByRole("alert")).toContainText("TEST pause unavailable");
    await expect(liveStatus.getByText("Current status: running",{exact:true})).toBeVisible();
    await liveStatus.getByRole("button",{name:"Pause",exact:true}).click();
    await expect(liveStatus.getByRole("button",{name:"Resume",exact:true})).toBeVisible();
    await liveStatus.getByRole("button",{name:"Resume",exact:true}).click();
    await expect(liveStatus.getByRole("button",{name:"Pause",exact:true})).toBeVisible();
    await liveStatus.getByRole("button",{name:"Cancel processing",exact:true}).click();
    await expect(liveStatus.getByText("Current status: cancelled",{exact:true})).toBeVisible();
    await page.reload();
    await expect(liveStatus.getByText("Current status: cancelled",{exact:true})).toBeVisible();
    expect(controlRequests).toEqual(["pause","pause","resume","cancel"]);
    expect(page.url()).toBe(savedWorkspaceUrl);
  }
  if(humanSchema){
    await page.goto(savedWorkspaceUrl);
    await expect(page.getByLabel("Saved sample results",{exact:true})).toBeVisible();
    await expect(page.getByText("No valid Schema proposal was produced. Saved evidence is retained; no automatic retry.",{exact:true})).toHaveCount(0);
    const originalImage=new URL(page.url()).searchParams.get("image");
    const originalTest=new URL(page.url()).searchParams.get("test");
    await page.getByRole("button",{name:"Annotation list · 1",exact:true}).click();
    await page.getByLabel("Annotations on canvas",{exact:true}).getByRole("button").first().click();
    await page.getByRole("button",{name:"Reference saved candidate in message",exact:true}).click();
    const reference=page.getByLabel("Message candidate reference",{exact:true});
    await expect(reference).toContainText("Only this saved candidate");
    await expect(page.getByLabel("Your message",{exact:true})).toBeFocused();
    await page.getByLabel("Your message",{exact:true}).fill("TEST UI 只讨论刚才选中的原始候选");
    const beforeMessage=await (await request.get(`${taskRoot}/budget`)).json();
    await page.getByLabel("Add images",{exact:true}).setInputFiles({name:"TEST-second-image.png",mimeType:"image/png",buffer:readFileSync(resolve("public/brand/core/pwa-192.png"))});
    await page.getByRole("navigation",{name:"Select image",exact:true}).getByRole("button",{name:"TEST-second-image.png",exact:true}).click();
    expect(new URL(page.url()).searchParams.get("image")).not.toBe(originalImage);
    await expect(reference).toContainText("synthetic-robocup.png");
    await page.setViewportSize({width:390,height:844});
    await expect(reference).toBeVisible();
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
    await page.screenshot({path:`../docs/execution/conversational-workspace/candidate-message-${kind}-390.png`,fullPage:true,animations:"disabled"});
    await reference.screenshot({path:`../docs/execution/conversational-workspace/candidate-reference-chip-${kind}.png`,animations:"disabled"});
    await page.setViewportSize({width:1280,height:800});
    let lost=false;let sent:any;
    await page.route("**/conversations/*/messages",async route=>{
      if(route.request().method()==="POST"&&!lost){lost=true;sent=route.request().postDataJSON();const response=await fetchWithinMutationLimit(route);expect(response.ok(),await response.text()).toBe(true);await route.abort("failed");}else await route.fallback();
    });
    await page.getByRole("button",{name:"Save message",exact:true}).click();
    await expect(page.getByRole("button",{name:"Retry saving message",exact:true})).toBeVisible();
    await expect(reference.getByRole("button",{name:"Remove candidate reference",exact:true})).toBeDisabled();
    await page.getByRole("button",{name:"Retry saving message",exact:true}).click();
    await expect(page.getByRole("list",{name:"Saved messages",exact:true})).toContainText("TEST UI 只讨论刚才选中的原始候选");
    expect(sent.image.image_id).toBe(originalImage);expect(sent.reference.sample_test_id).toBe(originalTest);
    expect(sent.reference.scope).toBe("sample_candidate");
    await page.reload();
    const messages=await (await request.get(`/api/projects/${project}/conversations/${envelope.conversation.conversation_id}/messages`)).json();
    expect(messages.filter((message:any)=>message.input.id===sent.id).map((message:any)=>message.input)).toEqual([sent]);
    expect(await (await request.get(`${taskRoot}/budget`)).json()).toEqual(beforeMessage);
    const mutationCount=starts.length;
    const item=page.getByRole("list",{name:"Saved messages",exact:true}).locator("li").filter({hasText:"TEST UI 只讨论刚才选中的原始候选"});
    await item.getByRole("button",{name:"Open referenced candidate",exact:true}).click();
    await expect(page).toHaveURL(new RegExp(`message=${sent.id}`));
    const referenceUrl=page.url();
    const historical=page.getByRole("region",{name:"Referenced sample candidate",exact:true});
    await expect(historical).toContainText("This historical prediction is read-only");
    await expect(historical.getByRole("button",{name:"Save sample feedback",exact:true})).toHaveCount(0);
    const evidence=(await (await request.get(`/api/workflow-drafts/${sent.reference.draft_id}/sample-test?test_id=${sent.reference.sample_test_id}`)).json()).sample_test;
    const projection=evidence.report.samples[0].projection;
    const original=[...projection.final_candidates,...projection.review_candidates.map((value:any)=>value.candidate)].find((value:any)=>value.outcome.id===sent.reference.candidate_id).outcome;
    if(kind==="bbox")await expect.poll(async()=>Number(await historical.locator("rect.aa-annotation-shape").getAttribute("width"))).toBeCloseTo(original.value.rect[2]*640,3);
    else await expect(historical.getByLabel("Image classification results",{exact:true})).toContainText(original.value.labels.join(", "));
    await page.reload();
    await expect(historical).toContainText("Original saved candidate");
    await historical.screenshot({path:`../docs/execution/conversational-workspace/message-reference-reopen-${kind}.png`,animations:"disabled"});
    const forged=new URL(referenceUrl);forged.searchParams.set("message",randomUUID());
    await page.goto(forged.toString());
    await expect(page.getByRole("alert").filter({hasText:"The message reference does not match"})).toBeVisible();
    await expect(historical).toHaveCount(0);
    const wrongImage=new URL(referenceUrl);wrongImage.searchParams.set("image",randomUUID());
    await page.goto(wrongImage.toString());
    await expect(page.getByRole("alert").filter({hasText:"The message reference does not match"})).toBeVisible();
    await expect(historical).toHaveCount(0);
    await page.goto(referenceUrl);
    await page.getByRole("button",{name:"Review model setup",exact:true}).click();
    await page.reload();
    await page.getByRole("button",{name:"Return to annotation task",exact:true}).click();
    await expect(page).toHaveURL(referenceUrl);
    await expect(historical).toContainText("Original saved candidate");
    await historical.getByRole("button",{name:"View current sample corrections",exact:true}).click();
    await expect(page.getByLabel("Saved sample results",{exact:true})).toBeVisible();
    expect(new URL(page.url()).searchParams.has("message")).toBe(false);
    expect(starts.length).toBe(mutationCount);
  }
});
}
