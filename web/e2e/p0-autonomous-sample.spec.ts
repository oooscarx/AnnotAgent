import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect as baseExpect, test } from "./fixtures";

const expect = baseExpect.configure({ timeout: 75_000 });

test("one bounded approval continues six newly uploaded images to three real Sample reviews", async ({ browser, page, request }, testInfo) => {
  test.skip(!process.env.AGENT_UI_TEST_MANIFEST, "Requires the marked isolated Agent UI fixture");
  test.setTimeout(240_000);
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
  await reopened.close();
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
