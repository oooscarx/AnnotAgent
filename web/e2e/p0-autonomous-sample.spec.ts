import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect as baseExpect, test } from "./fixtures";

const expect = baseExpect.configure({ timeout: 75_000 });

test("one bounded approval continues six newly uploaded images to three real Sample reviews", async ({ browser, page, request }, testInfo) => {
  test.skip(!process.env.AGENT_UI_TEST_MANIFEST, "Requires the marked isolated Agent UI fixture");
  test.setTimeout(360_000);
  const manifest = JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!, "utf8"));
  expect(manifest.fixture).toBe("external-model-only");
  const health = await request.get("/api/health");
  expect(health.headers()["x-annotagent-fixture"]).toBe("external-model-only");

  const project = `TEST-p0-autonomy-${randomUUID()}`;
  const yaml = `version: 1
project:
  name: TEST P0 autonomous sample
dataset:
  root: images
runtime: {}
tasks: []
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native]
`;
  const created = await request.post("/api/projects", { data: { id: project, yaml } });
  expect(created.ok(), await created.text()).toBe(true);
  const bound = await request.put(`/api/projects/${project}/model-bindings`, {
    data: {
      bindings: [{
        capability: "vision_language",
        role: "primary_inference",
        match_kind: "capability",
        model_profile_id: manifest.model_profile_id,
        locked: false,
      }],
    },
  });
  expect(bound.ok(), await bound.text()).toBe(true);

  const writes: string[] = [];
  const mark = async (name: string) => page.screenshot({ path: testInfo.outputPath(`${name}.png`), fullPage: true, animations: "disabled" });
  page.on("request", (event) => {
    if (!["GET", "HEAD"].includes(event.method())) writes.push(`${event.method()} ${new URL(event.url()).pathname}`);
  });
  await page.goto(`/projects/${project}/work`);
  const files = [1, 2, 3, 4, 5, 6].map((index) => resolve(`../examples/demo-packs/object-detection-review/1.0.0/images/desk_0${index}.png`));
  await page.locator('input[type="file"]').setInputFiles(files);
  await expect(page.locator("svg image")).toHaveAttribute("href", /^\/api\//);
  await page.getByRole("textbox", { name: "给 AnnotAgent 的需求" }).fill("标注这些图片中的杯子和瓶子，框住完整可见物体，用于 Ultralytics YOLO 目标检测。先给我看三张样例。");
  await page.getByRole("button", { name: "发送", exact: true }).click();
  await expect.poll(() => new URL(page.url()).searchParams.get("task")).not.toBeNull();
  const task = new URL(page.url()).searchParams.get("task");
  expect(task).toBeTruthy();

  const navigation = await (await request.get("/api/navigation?limit=100")).json();
  const owner = navigation.items.find((item: { project_id: string }) => item.project_id === project);
  expect(owner?.conversation_id).toBeTruthy();
  const root = `/api/projects/${project}/conversations/${owner.conversation_id}/tasks/${task}`;
  const before = await (await request.get(`${root}/workspace`)).json();
  expect(before.mainline.intake.dataset_scope).toHaveLength(6);
  expect(before.mainline.available_actions.filter((action: { id: string }) => action.id === "build_and_test_pipeline")).toHaveLength(1);
  await expect(page.getByRole("region", { name: "当前任务状态", exact: true })).toContainText("图片和要求已记录，可以准备样例");
  await mark("01-ready-to-start");

  await page.getByRole("button", { name: "开始标注样例", exact: true }).click();
  const approval = page.getByRole("dialog");
  await expect(approval).toContainText("3 张图片已冻结");
  await mark("02-bounded-approval");
  const consentResponse = page.waitForResponse((response) => response.request().method() === "POST" && new URL(response.url()).pathname === `${root}/journey-consents`);
  const consentStarted = Date.now();
  await approval.getByRole("button", { name: "接受未知费用并执行此范围", exact: true }).click();
  const accepted = await consentResponse;
  expect(Date.now() - consentStarted).toBeLessThan(3_000);
  expect(accepted.ok(), await accepted.text()).toBe(true);
  const acceptedBody = await accepted.json();
  expect(acceptedBody.consent.images).toHaveLength(3);
  expect(writes.filter((write) => write === `POST ${root}/journey-consents`)).toHaveLength(1);
  expect(writes.some((write) => write.endsWith("/execution"))).toBe(false);
  const executionPath = `${root}/journey-consents/${acceptedBody.consent.id}/execution`;
  const firstObservationResponse = await request.get(executionPath);
  expect(firstObservationResponse.ok(), await firstObservationResponse.text()).toBe(true);
  const firstObservation = await firstObservationResponse.json();
  expect(["queued", "running"]).toContain(firstObservation.dispatch.status);
  expect(firstObservation.sample).toBeNull();
  await expect(page.getByRole("region", { name: "当前任务状态", exact: true })).toContainText(/正在|已排队/);
  await mark("03-server-running");

  await page.close();
  await expect.poll(async () => {
    const workspace = await (await request.get(`${root}/workspace`)).json();
    const sample = workspace.sample_operations?.find((operation: { id: string }) => operation.id === acceptedBody.consent.sample_operation_id);
    return {
      sampleStatus: sample?.status ?? null,
      sampleId: sample?.id ?? null,
      humanRequests: workspace.human_requests?.length ?? 0,
    };
  }, { timeout: 90_000 }).toMatchObject({
    sampleStatus: "succeeded",
    sampleId: acceptedBody.consent.sample_operation_id,
    humanRequests: 3,
  });
  const completedWorkspace = await (await request.get(`${root}/workspace`)).json();
  const completedSample = completedWorkspace.sample_operations.find((operation: { id: string }) => operation.id === acceptedBody.consent.sample_operation_id);
  expect(completedSample).toMatchObject({ id: acceptedBody.consent.sample_operation_id, status: "succeeded" });
  expect(completedWorkspace.human_requests).toHaveLength(3);
  expect(completedWorkspace.human_requests.every((item: { input: { sample_test_id: string; task_id: string }; status: string }) =>
    item.input.sample_test_id === acceptedBody.consent.sample_operation_id &&
    item.input.task_id === task &&
    item.status === "pending",
  )).toBe(true);
  expect(completedWorkspace.mainline.available_actions).toHaveLength(1);
  expect(completedWorkspace.mainline.available_actions[0]).toMatchObject({
    id: "review_sample_results",
    method: "GET",
    requires_confirmation: false,
    scope: {
      sample_test_id: acceptedBody.consent.sample_operation_id,
      pending_count: 3,
    },
  });

  const sampleRecordResponse = await request.get(`/api/workflow-drafts/${completedSample.draft_id}/sample-test?test_id=${acceptedBody.consent.sample_operation_id}`);
  expect(sampleRecordResponse.ok(), await sampleRecordResponse.text()).toBe(true);
  const sampleRecord = await sampleRecordResponse.json();
  expect(sampleRecord.sample_test.inputs).toHaveLength(3);

  const reopened = await browser.newPage();
  reopened.on("request",(event)=>{
    if(!["GET","HEAD"].includes(event.method()))writes.push(`${event.method()} ${new URL(event.url()).pathname}`);
  });
  await reopened.goto(`/projects/${project}/work?task=${task}`);
  await expect(reopened).toHaveURL(new RegExp(`task=${task}`));
  await expect(reopened.getByRole("region", { name: "当前任务状态", exact: true })).toContainText("需要你的判断");
  const result = reopened.getByRole("region", { name: "当前任务图片结果", exact: true });
  await expect(result).toBeVisible();
  await expect(result.getByRole("heading", { name: "检查样例结果 · 3 张", exact: true })).toBeVisible();
  await expect(reopened.getByRole("region", { name: "当前任务状态", exact: true })).toContainText("3 个结果需要人工判断");
  await reopened.screenshot({ path: testInfo.outputPath("04-sample-review.png"), fullPage: true, animations: "disabled" });

  await reopened.emulateMedia({colorScheme:"dark"});
  await reopened.screenshot({path:testInfo.outputPath("05-sample-review-dark.png"),fullPage:true,animations:"disabled"});
  await reopened.setViewportSize({width:390,height:844});
  await expect(reopened.getByRole("region",{name:"当前任务图片结果",exact:true})).toBeVisible();
  await reopened.screenshot({path:testInfo.outputPath("06-sample-review-mobile.png"),fullPage:true,animations:"disabled"});
  await reopened.reload();
  await expect(reopened).toHaveURL(new RegExp(`task=${task}`));
  await expect(reopened.getByRole("region", { name: "当前任务图片结果", exact: true })).toBeVisible();

  const processingSelection={draft_id:completedSample.draft_id,sample_test_id:completedSample.id};
  const processingQuery=new URLSearchParams(processingSelection).toString();
  const blockedPreview=await request.get(`/api/projects/${project}/processing-preview?${processingQuery}`);
  expect(blockedPreview.status()).toBe(409);
  expect(await blockedPreview.json()).toMatchObject({code:"sample_reviews_pending",admitted:false,suggested_action:"review_sample_results",sample_review:{sample_test_id:completedSample.id,ready:false,unresolved:expect.any(Array)}});
  const blockedConfirm=await request.post(`/api/projects/${project}/processing-operations`,{data:{request_id:randomUUID(),selection:processingSelection,expected_revision:sampleRecord.sample_test.draft_revision,authorization_fingerprint:"0".repeat(64)}});
  expect(blockedConfirm.status()).toBe(409);
  expect(await blockedConfirm.json()).toMatchObject({code:"sample_reviews_pending",admitted:false});
  expect(await (await request.get(`${root}/processing-operations`)).json()).toEqual([]);

  await reopened.setViewportSize({width:1440,height:900});
  await reopened.emulateMedia({colorScheme:"light"});
  for(let reviewed=1;reviewed<=3;reviewed++){
    const confirmSample=reopened.getByRole("button",{name:"这个样例结果正确",exact:true});
    await expect(confirmSample).toBeEnabled();
    await confirmSample.click();
    await expect.poll(async()=>{
      const workspace=await(await request.get(`${root}/workspace`)).json();
      return workspace.human_requests.filter((item:{status:string;input:{sample_test_id:string}})=>item.status==="applied"&&item.input.sample_test_id===completedSample.id).length;
    }).toBe(reviewed);
  }
  await expect(reopened.getByRole("region",{name:"当前任务状态",exact:true})).toContainText("样例已经确认，可以处理剩余图片");
  const readyPreview=await request.get(`/api/projects/${project}/processing-preview?${processingQuery}`);
  expect(readyPreview.ok(),await readyPreview.text()).toBe(true);
  expect(await readyPreview.json()).toMatchObject({sample_review:{sample_test_id:completedSample.id,ready:true,unresolved:[],applied_request_ids:expect.any(Array)}});
  await reopened.screenshot({path:testInfo.outputPath("07-samples-confirmed.png"),fullPage:true,animations:"disabled"});

  await reopened.getByRole("button",{name:"确认范围并处理剩余图片",exact:true}).click();
  const processingApproval=reopened.getByRole("dialog");
  await expect(processingApproval).toContainText("6 张图片");
  const processingResponse=reopened.waitForResponse(response=>response.request().method()==="POST"&&new URL(response.url()).pathname===`/api/projects/${project}/processing-operations`);
  await processingApproval.getByRole("button",{name:"接受未知费用并执行此范围",exact:true}).click();
  const processingStarted=await processingResponse;
  expect(processingStarted.ok(),await processingStarted.text()).toBe(true);
  const processingReceipt=await processingStarted.json();
  expect(processingReceipt.batch_id).toBeTruthy();
  await expect.poll(async()=>{
    const batch=await(await request.get(`/api/batches/${processingReceipt.batch_id}`)).json();
    return batch.batch.status;
  },{timeout:90_000}).toMatch(/completed|awaiting_review/);
  await reopened.reload();
  await expect(reopened.getByRole("region",{name:"当前任务图片结果",exact:true})).toBeVisible();
  await expect(reopened.getByRole("heading",{name:"检查正式结果",exact:true})).toBeVisible();
  expect((await(await request.get(`${root}/processing-operations`)).json())).toHaveLength(1);
  await reopened.screenshot({path:testInfo.outputPath("08-formal-review.png"),fullPage:true,animations:"disabled"});

  const formalResponse=await request.get(`${root}/formal-result`);
  expect(formalResponse.ok(),await formalResponse.text()).toBe(true);
  const formal=await formalResponse.json();
  expect(formal.images).toHaveLength(6);
  const review=reopened.getByRole("region",{name:"当前任务图片结果",exact:true});
  for(const image of formal.images as {image_id:string;child_run_id:string|null}[]){
    await review.getByLabel("图片",{exact:true}).selectOption(image.image_id);
    await expect(review.getByLabel("图片",{exact:true})).toHaveValue(image.image_id);
    const stateResponse=await request.get(`${root}/delivery-images/${image.image_id}${image.child_run_id?`?source_run_id=${image.child_run_id}`:""}`);
    expect(stateResponse.ok(),await stateResponse.text()).toBe(true);
    const state=await stateResponse.json();
    if(state.confirmation_current)continue;
    if(!image.child_run_id){
      await review.getByLabel("检查备注／排除原因").fill("TEST explicit exclusion: the bound Batch did not produce an auditable child Run for this image");
      const saved=reopened.waitForResponse(response=>response.request().method()==="POST"&&new URL(response.url()).pathname.endsWith(`/delivery-images/${image.image_id}`));
      await review.getByRole("button",{name:"明确排除此图并继续",exact:true}).click();
      expect((await saved).ok()).toBe(true);
      continue;
    }
    const activeCount=state.snapshot.annotations.filter((annotation:{review_status:string})=>annotation.review_status!=="rejected").length;
    await expect(review.locator("rect.aa-annotation-shape")).toHaveCount(activeCount);
    await expect(review.getByRole("button",{name:"重新读取服务器状态",exact:true})).toBeEnabled();
    await expect(review).toContainText(`未解决对象 ${state.unresolved_objects}`);
    const listToggle=review.getByRole("button",{name:new RegExp(`^Annotation list · ${activeCount}$`)});
    if(await listToggle.getAttribute("aria-expanded")!=="true")await listToggle.click();
    const objectRows=review.getByRole("list",{name:"Annotations on canvas",exact:true}).getByRole("button");
    await expect(objectRows).toHaveCount(activeCount);
    let unresolved=state.unresolved_objects;
    for(let index=0;index<activeCount;index++){
      await objectRows.nth(index).click();
      const acceptObject=review.getByRole("button",{name:"接受这个对象",exact:true});
      await expect(acceptObject).toBeEnabled();
      await acceptObject.click();
      unresolved--;
      await expect(review).toContainText(`未解决对象 ${unresolved}`);
    }
    const saved=reopened.waitForResponse(response=>response.request().method()==="POST"&&new URL(response.url()).pathname.endsWith(`/delivery-images/${image.image_id}`));
    await review.getByRole("button",{name:"确认整张图标注完整并继续",exact:true}).click();
    expect((await saved).ok()).toBe(true);
  }

  await reopened.reload();
  await expect(reopened.getByRole("region",{name:"当前任务状态",exact:true})).toContainText("可以生成训练数据包");
  await reopened.getByRole("button",{name:"生成训练数据包",exact:true}).click();
  const delivery=reopened.getByRole("region",{name:"训练数据包交付",exact:true});
  await expect(delivery).toBeVisible();
  await delivery.getByRole("button",{name:"刷新审核与打包状态",exact:true}).click();
  await expect(delivery).toContainText("正式审核齐全");
  await delivery.getByRole("button",{name:"允许审核齐全后自动打包",exact:true}).click();
  await expect(delivery.getByRole("link",{name:"下载数据集 ZIP",exact:true})).toBeVisible({timeout:30_000});
  expect(writes.filter(write=>write.endsWith("/delivery-package-consents"))).toHaveLength(1);
  expect(writes.filter(write=>write.endsWith("/delivery-packages"))).toHaveLength(0);
  const writesBeforeReload=writes.length;
  await reopened.reload();
  await expect(reopened.getByRole("link",{name:"下载数据集 ZIP",exact:true})).toBeVisible();
  expect(writes).toHaveLength(writesBeforeReload);
  const downloadStarted=reopened.waitForEvent("download");
  await reopened.getByRole("link",{name:"下载数据集 ZIP",exact:true}).click();
  const download=await downloadStarted;
  await download.saveAs(testInfo.outputPath("p0-autonomous-delivery-TEST.zip"));
  expect(await download.failure()).toBeNull();
  await reopened.screenshot({path:testInfo.outputPath("09-package-ready.png"),fullPage:true,animations:"disabled"});
  await reopened.close();
});

test("current task keeps an exact three-image scope inside a ten-image project",async({page,request},testInfo)=>{
  test.skip(!process.env.AGENT_UI_TEST_MANIFEST,"Requires the marked isolated Agent UI fixture");
  test.setTimeout(180_000);
  const manifest=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8"));
  const project=`TEST-p0-exact-scope-${randomUUID()}`;
  const yaml=`version: 1
project:
  name: TEST P0 exact task scope
dataset:
  root: images
runtime: {}
tasks: []
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native]
`;
  expect((await request.post("/api/projects",{data:{id:project,yaml}})).ok()).toBe(true);
  const binding=await request.put(`/api/projects/${project}/model-bindings`,{data:{bindings:[{capability:"vision_language",role:"primary_inference",match_kind:"capability",model_profile_id:manifest.model_profile_id,locked:false}]}});
  expect(binding.ok(),await binding.text()).toBe(true);
  const existingFiles=[
    "../examples/robocup/images/synthetic-robocup.png",
    "public/brand/core/pwa-192.png",
    "public/brand/core/pwa-512.png",
    "public/evidence/visual-finish/reference/01-new-task-light.png",
    "public/brand/core/apple-touch-icon.png",
    "public/brand/core/og-card.png",
    "../examples/demo-packs/object-detection-review/1.0.0/thumbnail.png",
  ];
  for(let index=0;index<existingFiles.length;index++){
    const bytes=readFileSync(resolve(existingFiles[index]));
    const uploaded=await request.post(`/api/projects/${project}/image-upload?name=existing_${index+1}.png`,{data:bytes,headers:{"Content-Type":"image/png"}});
    expect(uploaded.ok(),await uploaded.text()).toBe(true);
  }
  const beforeImages=(await(await request.get(`/api/projects/${project}/images`)).json()).images;
  expect(beforeImages).toHaveLength(7);
  const existingIds=new Set(beforeImages.map((image:{image_id:string})=>image.image_id));

  await page.goto(`/projects/${project}/work`);
  const currentFiles=[1,2,3].map(index=>resolve(`../examples/demo-packs/object-detection-review/1.0.0/images/desk_0${index}.png`));
  await page.locator('input[type="file"]').setInputFiles(currentFiles);
  await page.getByRole("textbox",{name:"给 AnnotAgent 的需求"}).fill("标注这三张新图片中的杯子和瓶子，框住完整可见物体，用于 Ultralytics YOLO 目标检测。先给我看三张样例。");
  await page.getByRole("button",{name:"发送",exact:true}).click();
  await expect.poll(()=>new URL(page.url()).searchParams.get("task")).not.toBeNull();
  const task=new URL(page.url()).searchParams.get("task")!;
  const navigation=await(await request.get("/api/navigation?limit=100")).json();
  const owner=navigation.items.find((item:{project_id:string})=>item.project_id===project);
  const root=`/api/projects/${project}/conversations/${owner.conversation_id}/tasks/${task}`;
  const initial=await(await request.get(`${root}/workspace`)).json();
  const taskIds=initial.mainline.intake.dataset_scope.map((image:{image_id:string})=>image.image_id);
  expect(taskIds).toHaveLength(3);
  expect(taskIds.every((id:string)=>!existingIds.has(id))).toBe(true);
  expect((await(await request.get(`/api/projects/${project}/images`)).json()).images).toHaveLength(10);

  await page.getByRole("button",{name:"开始标注样例",exact:true}).click();
  const approval=page.getByRole("dialog");
  await expect(approval).toContainText("3 张图片已冻结");
  await page.screenshot({path:testInfo.outputPath("01-exact-three-of-ten-approval.png"),fullPage:true,animations:"disabled"});
  await approval.getByRole("button",{name:"接受未知费用并执行此范围",exact:true}).click();
  await expect.poll(async()=>{
    const workspace=await(await request.get(`${root}/workspace`)).json();
    return workspace.human_requests?.filter((item:{status:string})=>item.status==="pending").length||0;
  },{timeout:90_000}).toBe(3);
  const afterSample=await(await request.get(`${root}/workspace`)).json();
  const sample=afterSample.sample_operations.find((item:{status:string})=>item.status==="succeeded");
  const sampleRecord=await(await request.get(`/api/workflow-drafts/${sample.draft_id}/sample-test?test_id=${sample.id}`)).json();
  expect(new Set(sampleRecord.sample_test.inputs.map((input:{image_id:string})=>input.image_id))).toEqual(new Set(taskIds));

  await page.reload();
  for(let count=1;count<=3;count++){
    await page.getByRole("button",{name:"这个样例结果正确",exact:true}).click();
    await expect.poll(async()=>{
      const workspace=await(await request.get(`${root}/workspace`)).json();
      return workspace.human_requests.filter((item:{status:string})=>item.status==="applied").length;
    }).toBe(count);
  }
  await page.getByRole("button",{name:"确认范围并处理剩余图片",exact:true}).click();
  const processingDialog=page.getByRole("dialog");
  await expect(processingDialog).toContainText("3 张图片");
  const started=page.waitForResponse(response=>response.request().method()==="POST"&&new URL(response.url()).pathname===`/api/projects/${project}/processing-operations`);
  await processingDialog.getByRole("button",{name:"接受未知费用并执行此范围",exact:true}).click();
  const receipt=await(await started).json();
  expect(receipt.authorization.available_images).toBe(10);
  expect(receipt.authorization.images.map((image:{image_id:string})=>image.image_id)).toEqual(taskIds);
  await page.screenshot({path:testInfo.outputPath("02-exact-three-of-ten-processing.png"),fullPage:true,animations:"disabled"});
});

test("description first then later upload keeps one Task and exact six-image scope",async({page,request},testInfo)=>{
  test.skip(!process.env.AGENT_UI_TEST_MANIFEST,"Requires the marked isolated Agent UI fixture");
  test.setTimeout(180_000);
  const manifest=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8"));
  expect(manifest.fixture).toBe("external-model-only");
  const project=`TEST-p0-description-first-${randomUUID()}`;
  const yaml=`version: 1
project:
  name: TEST P0 description first
dataset:
  root: images
runtime: {}
tasks: []
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native]
`;
  expect((await request.post("/api/projects",{data:{id:project,yaml}})).ok()).toBe(true);
  const binding=await request.put(`/api/projects/${project}/model-bindings`,{data:{bindings:[{capability:"vision_language",role:"primary_inference",match_kind:"capability",model_profile_id:manifest.model_profile_id,locked:false}]}});
  expect(binding.ok(),await binding.text()).toBe(true);
  const writes:string[]=[];
  page.on("request",event=>{if(!["GET","HEAD"].includes(event.method()))writes.push(`${event.method()} ${new URL(event.url()).pathname}`);});
  await page.goto(`/projects/${project}/work`);
  await page.getByRole("textbox",{name:"给 AnnotAgent 的需求"}).fill("标注这些图片中的杯子和瓶子，框住完整可见物体，用于 Ultralytics YOLO 目标检测。先给我看三张样例。");
  await page.getByRole("button",{name:"发送",exact:true}).click();
  await expect.poll(()=>new URL(page.url()).searchParams.get("task")).not.toBeNull();
  const task=new URL(page.url()).searchParams.get("task")!;
  const navigation=await(await request.get("/api/navigation?limit=100")).json();
  const owner=navigation.items.find((item:{project_id:string})=>item.project_id===project);
  const root=`/api/projects/${project}/conversations/${owner.conversation_id}/tasks/${task}`;
  const missing=await(await request.get(`${root}/workspace`)).json();
  expect(missing.mainline.intake.missing_slots).toContain("dataset_scope");
  expect(missing.sample_operations||[]).toHaveLength(0);
  await expect(page.getByRole("region",{name:"当前任务状态",exact:true})).toContainText("请上传或选择这次要处理的图片");
  await page.screenshot({path:testInfo.outputPath("00-missing-images.png"),fullPage:true,animations:"disabled"});

  const files=[1,2,3,4,5,6].map(index=>resolve(`../examples/demo-packs/object-detection-review/1.0.0/images/desk_0${index}.png`));
  await page.locator('input[type="file"]').setInputFiles(files);
  await expect.poll(async()=>{
    const workspace=await(await request.get(`${root}/workspace`)).json();
    return {images:workspace.mainline.intake.dataset_scope?.length||0,actions:workspace.mainline.available_actions.filter((action:{id:string})=>action.id==="build_and_test_pipeline").length};
  }).toEqual({images:6,actions:1});
  const ready=await(await request.get(`${root}/workspace`)).json();
  expect(ready.task.input.id).toBe(task);
  expect(ready.sample_operations||[]).toHaveLength(0);
  expect(ready.mainline.intake.dataset_scope.map((image:{image_id:string;content_sha256:string})=>[image.image_id,image.content_sha256])).toHaveLength(6);
  expect(new Set(ready.mainline.intake.dataset_scope.map((image:{image_id:string})=>image.image_id)).size).toBe(6);
  expect(writes.filter(write=>write===`POST ${root}/delivery-intent`)).toHaveLength(1);
  expect(writes.some(write=>write.includes("journey-consents")||write.endsWith("/execution"))).toBe(false);
  await expect(page.getByRole("region",{name:"当前任务状态",exact:true})).toContainText("图片和要求已记录，可以准备样例");
  await page.screenshot({path:testInfo.outputPath("01-description-first-ready.png"),fullPage:true,animations:"disabled"});
});

test("ambiguous YOLO goal asks one output question and resumes the same Journey",async({page,request},testInfo)=>{
  test.skip(!process.env.AGENT_UI_TEST_MANIFEST,"Requires the marked isolated Agent UI fixture");
  test.setTimeout(180_000);
  const manifest=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8"));
  expect(manifest.fixture).toBe("external-model-only");
  const project=`TEST-p0-clarification-${randomUUID()}`;
  const yaml=`version: 1
project:
  name: TEST P0 clarification
dataset:
  root: images
runtime: {}
tasks: []
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native]
`;
  expect((await request.post("/api/projects",{data:{id:project,yaml}})).ok()).toBe(true);
  const binding=await request.put(`/api/projects/${project}/model-bindings`,{data:{bindings:[{capability:"vision_language",role:"primary_inference",match_kind:"capability",model_profile_id:manifest.model_profile_id,locked:false}]}});
  expect(binding.ok(),await binding.text()).toBe(true);
  const profiles=await(await request.get("/api/model-profiles")).json();
  const planner=profiles.models.find((model:{remote_model_id:string})=>model.remote_model_id==="e2e-conversation-clarify");
  expect(planner?.id).toBeTruthy();
  const conversation=(await(await request.post(`/api/projects/${project}/conversations`)).json()).conversation_id;
  const preference=await(await request.get(`/api/projects/${project}/conversations/${conversation}/agent-model`)).json();
  const selected=await request.post(`/api/projects/${project}/conversations/${conversation}/agent-model`,{data:{request_id:randomUUID(),expected_revision:preference.revision,model_profile_id:planner.id}});
  expect(selected.ok(),await selected.text()).toBe(true);

  const writes:string[]=[];
  page.on("request",event=>{if(!["GET","HEAD"].includes(event.method()))writes.push(`${event.method()} ${new URL(event.url()).pathname}`);});
  await page.goto(`/projects/${project}/work`);
  const files=[1,2,3,4,5,6].map(index=>resolve(`../examples/demo-packs/object-detection-review/1.0.0/images/desk_0${index}.png`));
  await page.locator('input[type="file"]').setInputFiles(files);
  await page.getByRole("textbox",{name:"给 AnnotAgent 的需求"}).fill("标注这些图片中的杯子，训练 YOLO。先给我看三张样例。");
  await page.getByRole("button",{name:"发送",exact:true}).click();
  await expect.poll(()=>new URL(page.url()).searchParams.get("task")).not.toBeNull();
  const task=new URL(page.url()).searchParams.get("task")!;
  const root=`/api/projects/${project}/conversations/${conversation}/tasks/${task}`;
  await expect(page.getByRole("button",{name:"开始标注样例",exact:true})).toBeVisible();
  await page.getByRole("button",{name:"开始标注样例",exact:true}).click();
  const approval=page.getByRole("dialog");
  const consentResponse=page.waitForResponse(response=>response.request().method()==="POST"&&new URL(response.url()).pathname===`${root}/journey-consents`);
  await approval.getByRole("button",{name:"接受未知费用并执行此范围",exact:true}).click();
  const accepted=await consentResponse;expect(accepted.ok(),await accepted.text()).toBe(true);
  const consent=(await accepted.json()).consent;

  const status=page.getByRole("region",{name:"当前任务状态",exact:true});
  await expect(status).toContainText("只需要确认输出类型");
  await expect(page.getByText("需要框出目标、描出轮廓，还是做整图分类？",{exact:true})).toBeVisible();
  await expect(status.getByRole("button",{name:"框住目标",exact:true})).toBeEnabled();
  await expect(status.getByRole("button",{name:"描出轮廓",exact:true})).toBeDisabled();
  await expect(status.getByRole("button",{name:"整图分类",exact:true})).toBeDisabled();
  await page.screenshot({path:testInfo.outputPath("01-output-question.png"),fullPage:true,animations:"disabled"});

  const answerResponse=page.waitForResponse(response=>response.request().method()==="POST"&&new URL(response.url()).pathname.endsWith("/clarification/answer"));
  await status.getByRole("button",{name:"框住目标",exact:true}).click();
  const answered=await answerResponse;expect(answered.ok(),await answered.text()).toBe(true);
  const answer=await answered.json();
  expect(answer.journey_resume.consent_id).toBe(consent.id);
  expect(answer.selected_choice).toBe("bounding_box");
  await expect(status).toContainText("需要你的判断",{timeout:90_000});
  await expect(page.getByRole("region",{name:"当前任务图片结果",exact:true})).toBeVisible();
  const workspace=await(await request.get(`${root}/workspace`)).json();
  expect(workspace.calls.filter((call:{evidence?:{decision?:unknown}})=>call.evidence?.decision)).toHaveLength(1);
  expect(workspace.sample_operations).toHaveLength(1);
  expect(workspace.sample_operations[0].id).toBe(consent.sample_operation_id);
  expect(writes.filter(write=>write===`POST ${root}/journey-consents`)).toHaveLength(1);
  expect(writes.filter(write=>write.endsWith("/clarification/answer"))).toHaveLength(1);
  expect(writes.some(write=>write.endsWith("/human-schema-drafts")||write.endsWith("/execution"))).toBe(false);
  await page.screenshot({path:testInfo.outputPath("02-same-journey-review.png"),fullPage:true,animations:"disabled"});
});
