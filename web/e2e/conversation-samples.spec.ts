import { resolve } from "node:path";
import { expect, test } from "./fixtures";

for(const kind of ["classification","bbox"] as const){
test(`conversation ${kind} authorizes HTTP fixture samples and restores editable terminal canvas`,async({page,request})=>{
  test.setTimeout(120_000);
  const provider=await (await request.post("/api/providers",{data:{display_name:"Conversation sample TEST transport",adapter:"open_ai_compatible",base_url:"http://127.0.0.1:8796/openai/v1"}})).json();
  expect((await request.post(`/api/providers/${provider.id}/credential`,{data:{source:"workspace_file",secret:"TEST-conversation-samples-only"}})).ok()).toBeTruthy();
  const model=await (await request.post("/api/model-profiles",{data:{provider_id:provider.id,display_name:`Conversation TEST ${kind}`,remote_model_id:`e2e-conversation-${kind}`,input_modalities:["text","image"],task_capabilities:["text_generation","vision_language","image_classification"],protocol_features:{tool_calls:true,structured_output:true}}})).json();
  expect((await request.post(`/api/providers/${provider.id}/active-probe`,{data:{model_profile_id:model.id,confirmed_billable:true}})).ok()).toBeTruthy();
  const defaults=await (await request.get("/api/agent-model-bindings")).json();
  expect((await request.put("/api/agent-model-bindings",{data:{...defaults,pipeline_builder:model.id}})).ok()).toBeTruthy();
  const project=`conversation-samples-${Date.now()}`;
  expect((await request.post("/api/projects",{data:{id:project,yaml:"version: 1\nproject:\n  name: TEST conversation samples\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}})).ok()).toBeTruthy();
  expect((await request.put(`/api/projects/${project}/model-bindings`,{data:{bindings:[{capability:"image_classification",role:"classification",match_kind:"capability",model_profile_id:model.id,locked:false}]}})).ok()).toBeTruthy();
  await page.goto(`/projects/${project}/work`);
  await page.getByLabel("Add images",{exact:true}).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await expect(page.getByText("Images saved on this server. No model has been called.",{exact:true})).toBeVisible();
  await page.getByLabel("Your message",{exact:true}).fill(kind==="classification" ? "按室内和室外给图片分类" : "Find cups, not bottles. Draw a tight box around each cup.");
  await page.getByRole("button",{name:"Save message",exact:true}).click();
  await page.getByRole("button",{name:"Prepare label proposal",exact:true}).click();
  await page.getByRole("checkbox",{name:/Allow this text request/}).check();
  await page.getByRole("button",{name:"Generate label proposal",exact:true}).click();
  await page.getByRole("button",{name:"Save as editable Schema Draft",exact:true}).click();
  await page.getByRole("button",{name:"Review Builder authorization",exact:true}).click();
  await page.getByRole("checkbox",{name:/Allow this bounded Builder request/}).check();
  await page.getByRole("button",{name:"Build Pipeline Draft",exact:true}).click();
  await page.getByRole("button",{name:"Review sample authorization",exact:true}).click();
  const authorization=page.getByLabel("Sample model authorization",{exact:true});
  await expect(authorization).toContainText("Cost unknown");
  await page.getByRole("checkbox",{name:/Allow these sample images/}).check();
  const start=page.getByRole("button",{name:"Test these samples",exact:true});
  await expect(start).toBeEnabled();
  let envelope:any;
  await page.route(`**/api/projects/${project}/sample-operations`,async route=>{
    if(route.request().method()!=="POST")return route.continue();
    envelope=route.request().postDataJSON();
    await route.fetch();await route.abort("failed");
  },{times:1});
  await start.click();
  await expect(page.getByRole("button",{name:"View sample results in canvas",exact:true})).toBeVisible();
  expect(envelope.conversation.allow_unknown_cost).toBe(true);
  const taskRoot=`/api/projects/${project}/conversations/${envelope.conversation.conversation_id}/tasks/${envelope.conversation.task_id}`;
  const calls=await (await request.get(`${taskRoot}/calls`)).json();
  expect(calls.some((call:any)=>call.evidence?.phase==="sample_inference" && call.status==="completed")).toBe(true);
  if (kind === "bbox") {
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
  await page.screenshot({path:`../docs/execution/conversational-workspace/sample-${kind}.png`,fullPage:true});
  await page.setViewportSize({width:390,height:844});
  await page.getByRole("button",{name:/Images \(/}).click();
  await expect(page.getByLabel("Saved sample results",{exact:true})).toBeVisible();
  await page.screenshot({path:`../docs/execution/conversational-workspace/sample-${kind}-390.png`,fullPage:true});
});
}
