import {test,expect} from "@playwright/test";
import {readFileSync,writeFileSync,existsSync} from "node:fs";
import {join} from "node:path";

test("LIVE recording: original uploads, delivery goal and authorized VLM plus local refinement",async({page,request},info)=>{
  test.skip(process.env.LIVE_VIDEO_OPENING!=="1","Explicit opt-in paid LIVE recording only");
  const root=process.env.LIVE_RECORDING_WORKSPACE!;expect(root).toMatch(/^\/tmp\/AnnotAgent-LIVE-recording-/);
  expect(JSON.parse(readFileSync(join(root,"LIVE_RECORDING.json"),"utf8")).synthetic_model).toBe(false);
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBeUndefined();
  const record=join(root,"video-journey.json");if(existsSync(record))expect(JSON.parse(readFileSync(record,"utf8")).trace.some((t:any)=>t.path)).toBe(false);
  const scene:any={live:true,project:"live-robocup-video",started_at:new Date().toISOString(),trace:[],source_sha:process.env.LIVE_SOURCE_SHA};
  const save=()=>writeFileSync(record,JSON.stringify(scene,null,2),{mode:0o600});save();
  const mark=async(label:string)=>{scene.trace.push({at:Date.now(),label,url:page.url()});save();await page.screenshot({path:info.outputPath(label+".png")});};
  page.on("response",async response=>{
    const method=response.request().method(),path=new URL(response.url()).pathname;
    if(method!=="POST"||!path.startsWith("/api/")||path.endsWith("/session"))return;
    let value;try{value=await response.json();}catch{return;}
    scene.trace.push({at:Date.now(),path,status:response.status(),response:value});
    if(path.endsWith("/send")){scene.task_id=value.task_id;scene.conversation_id=path.split("/")[5];scene.task_root=path.replace(/\/send$/,"/tasks/"+value.task_id);}
    if(path.endsWith("/delivery-intent"))scene.delivery=value.saved;
    save();
  });
  await page.goto(`/projects/${scene.project}/work`);
  await expect(page.getByRole("textbox",{name:"给 AnnotAgent 的需求"})).toBeVisible();await mark("01-empty-project");
  const source="/Users/oscar/Documents/my_workspace/AnnotAgent/workspace/robocup-ball/images";
  await page.locator(".composer input[type=file]").setInputFiles([join(source,"color_289771.png"),join(source,"color_548575.png")]);
  await expect(page.getByText("color_289771.png",{exact:true})).toBeVisible();await page.waitForTimeout(1300);await mark("02-original-images");
  await page.getByRole("textbox",{name:"给 AnnotAgent 的需求"}).pressSequentially("标注足球和机器人，用来训练 YOLO 目标检测。排除人和场线；先试跑样例，再交付原图、标签和 data.yaml。VLM 只作候选定位，请使用已安装的 SAM 检查边界，不确定时交我修正。",{delay:25});
  await page.getByRole("button",{name:"发送",exact:true}).click();
  const intake=page.getByRole("region",{name:"训练数据交付信息",exact:true});await expect(intake).toBeVisible();await mark("03-saved-goal");
  await intake.getByRole("button",{name:"定义交付目标",exact:true}).click();
  await intake.getByRole("button",{name:"选择当前 2 张图片",exact:true}).click();
  await intake.getByLabel("类别名称（每行一个）",{exact:true}).fill("足球\n机器人");
  await intake.getByRole("combobox",{name:"训练什么任务？",exact:true}).selectOption("ultralytics_yolo_detection");
  await intake.getByText("已有数据划分与来源组",{exact:true}).click();
  await intake.getByLabel("已有划分 1",{exact:true}).selectOption("val");await intake.getByLabel("已有划分 2",{exact:true}).selectOption("train");
  await intake.getByRole("button",{name:"保存交付信息",exact:true}).click();await expect(intake.getByText("已保存到服务器",{exact:true})).toBeVisible();await mark("04-persisted-delivery-goal");
  await intake.getByRole("button",{name:"收起信息",exact:true}).click();await intake.getByRole("button",{name:"确认目标并准备方案",exact:true}).click();
  await expect(intake).toContainText("目标规范已准备");
  // Read-only preview first. Only the exact visible confirmation sends paid work.
  let sample=page.getByRole("button",{name:"构建方案并测试样例…",exact:true});
  if(!await sample.isVisible())await page.getByText("其他操作",{exact:true}).click();
  await sample.click();await page.getByRole("button",{name:"查看并确认授权",exact:true}).click();
  const dialog=page.getByRole("dialog");await expect(dialog).toContainText("efficientsam");await expect(dialog).toContainText("dashscope");await mark("05-real-model-authorization");await page.waitForTimeout(1800);
  await dialog.getByRole("button",{name:"接受未知费用并执行此范围",exact:true}).click();await mark("06-authorized-model-execution");
  // Wait on authoritative persisted receipt, not on an invented countdown or token stream.
  await expect.poll(async()=>{if(!scene.task_root)return "not-sent";const r=await request.get(scene.task_root+"/workspace");if(!r.ok())return "unavailable";const ws=await r.json();scene.workspace=ws;save();return ws.sample_operations?.at(-1)?.status||ws.builder_operations?.items?.at(-1)?.status||"waiting";},{timeout:480_000,intervals:[2500]}).toBe("succeeded");
  scene.images=(await(await request.get(`/api/projects/${scene.project}/images`)).json()).images;
  scene.sample=scene.workspace.sample_operations.at(-1);const r=await request.get(`/api/workflow-drafts/${scene.sample.draft_id}/sample-test?test_id=${scene.sample.id}`);scene.sample_result=await r.json();save();
  expect(scene.sample_result.sample_test.report.samples.some((s:any)=>s.nodes.some((n:any)=>n.node_id.includes("segment")&&n.status==="succeeded"))).toBe(true);
  await page.reload();await page.getByRole("button",{name:"定位需要修正的目标 →",exact:true}).click();await mark("07-real-sample-and-human-request");await page.waitForTimeout(2500);
});

test("record the actual extracted YAML and validation report",async({page},info)=>{
  test.skip(process.env.LIVE_VIDEO_FILES!=="1","Explicit read-only local extracted file capture");test.setTimeout(60_000);
  const base=process.env.LIVE_EVIDENCE_URL!;expect(base).toMatch(/^http:\/\/127\.0\.0\.1:\d+$/);
  const root=process.env.LIVE_DELIVERY_DIRECTORY!;expect(root).toMatch(/^\/Users\/oscar\/Downloads\/AnnotAgent-Live-Demo-20260910-[^/]+\/dataset$/);
  await page.goto(base+"/");await expect(page.getByRole("heading",{name:"下载后解压的真实文件"})).toBeVisible();await page.screenshot({path:info.outputPath("18-extracted-files.png")});await page.waitForTimeout(2000);
  await page.getByRole("link",{name:"data.yaml",exact:true}).click();await expect(page.locator("pre")).toHaveText(readFileSync(join(root,"data.yaml"),"utf8"));await page.screenshot({path:info.outputPath("19-portable-yaml.png")});await page.waitForTimeout(4500);
  await page.goto(base+"/annotagent/validation-report.json");await expect(page.locator("pre")).toHaveText(readFileSync(join(root,"annotagent/validation-report.json"),"utf8"));await page.screenshot({path:info.outputPath("20-actual-validation-report.png")});await page.waitForTimeout(4500);
});

test("LIVE recording: inspect persisted detection and SAM masks without rerunning",async({page,request},info)=>{
  test.setTimeout(60_000);
  test.skip(process.env.LIVE_VIDEO_EVIDENCE!=="1","Read-only LIVE evidence capture");
  const root=process.env.LIVE_RECORDING_WORKSPACE!;expect(root).toMatch(/^\/tmp\/AnnotAgent-LIVE-recording-/);
  const scene=JSON.parse(readFileSync(join(root,"video-journey.json"),"utf8"));expect(scene.project).toBe("live-robocup-video");
  const image=scene.images.find((i:any)=>i.name==="color_289771.png");
  const run=scene.batch.batch.images.find((i:any)=>i.image_id===image.image_id).child_run_id;
  const writes:string[]=[];page.on("request",r=>{if(!["GET","HEAD"].includes(r.method()))writes.push(r.url());});
  await page.goto(`/projects/${scene.project}/manage/runs/${run}?view=debug`);
  const inspector=page.getByRole("region",{name:"节点与 Artifact 检查",exact:true});
  const select=inspector.getByLabel(/选择实际执行节点/);await select.selectOption("shared.detector");
  await inspector.getByRole("button",{name:/查看输出 1 · detection_set/}).click();
  const preview=page.getByRole("region",{name:"中间产物预览",exact:true});await expect(preview.getByRole("img",{name:"此产物的原始输入图片"})).toBeVisible();await preview.scrollIntoViewIfNeeded();
  await page.screenshot({path:info.outputPath("15-original-vlm-candidate.png")});await page.waitForTimeout(3500);
  const options=await select.locator("option").evaluateAll(elements=>elements.map(e=>({value:(e as HTMLOptionElement).value,text:e.textContent})));
  const segment=options.find(o=>o.value.endsWith(".geometry_refine.segment")&&o.value.includes(scene.delivery.intent.label_spec.find((l:any)=>l.display_name==="足球").stable_id));expect(segment).toBeTruthy();
  await select.selectOption(segment!.value);await inspector.getByRole("button",{name:/查看输出 1 · mask_set/}).click();
  await expect(preview).toContainText("可显示 Mask 1");await expect.poll(()=>preview.locator("canvas.artifact-mask-layer").evaluate(canvas=>{const c=canvas as HTMLCanvasElement;const bytes=c.getContext("2d")!.getImageData(0,0,c.width,c.height).data;let active=0;for(let i=3;i<bytes.length;i+=4)if(bytes[i])active++;return active;})).toBeGreaterThan(100);
  await preview.scrollIntoViewIfNeeded();await page.screenshot({path:info.outputPath("16-real-sam-mask.png")});await page.waitForTimeout(4500);
  await page.goto(scene.final_package_url);const delivery=page.getByRole("region",{name:"训练数据包交付",exact:true});
  await expect(delivery.getByRole("link",{name:"下载数据集 ZIP",exact:true})).toBeVisible();await delivery.scrollIntoViewIfNeeded();await page.screenshot({path:info.outputPath("17-restored-package.png")});await page.waitForTimeout(3000);
  expect(writes).toEqual([]);
});

test("LIVE recording: saved human boundary correction and explicit formal processing",async({page,request},info)=>{
  test.skip(process.env.LIVE_VIDEO_PROCESS!=="1","Explicit opt-in to continue the same recorded LIVE task");
  const root=process.env.LIVE_RECORDING_WORKSPACE!;expect(root).toMatch(/^\/tmp\/AnnotAgent-LIVE-recording-/);
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBeUndefined();
  const record=join(root,"video-journey.json"),scene=JSON.parse(readFileSync(record,"utf8"));expect(scene.project).toBe("live-robocup-video");
  const save=()=>writeFileSync(record,JSON.stringify(scene,null,2),{mode:0o600});
  const mark=async(label:string)=>{scene.trace.push({at:Date.now(),label,url:page.url()});save();await page.screenshot({path:info.outputPath(label+".png")});};
  page.on("response",async r=>{if(r.request().method()==="POST"&&new URL(r.url()).pathname.startsWith("/api/")){let response;try{response=await r.json();}catch{return;}scene.trace.push({at:Date.now(),path:new URL(r.url()).pathname,status:r.status(),response});if(r.url().endsWith("/processing-operations"))scene.processing=response;save();}});
  const first=scene.images.find((i:any)=>i.name==="color_289771.png");
  await page.goto(`/projects/${scene.project}/work?task=${scene.task_id}&pane=image&image=${first.image_id}`);
  const pane=page.getByRole("complementary",{name:"图片与标注"});
  await expect(pane.locator("svg .bbox-label")).toContainText(["足球"]);await mark("08-named-real-sample");
  const plan=page.getByText(/查看计划 · .*个步骤/);await plan.click();
  await expect(page.locator(".plan-block")).toContainText("efficientsam");await page.locator(".plan-block").scrollIntoViewIfNeeded();await mark("09-actual-plan-models");await page.waitForTimeout(2000);await plan.click();
  await pane.getByText(/标注列表与精确编辑/).click();
  const current=await(await request.get(scene.task_root+"/workspace")).json();
  const human=current.human_requests.find((h:any)=>h.status==="pending"&&h.input.image_id===first.image_id);expect(human).toBeTruthy();
  for(const [axis,value] of Object.entries({x:232,y:237,w:54,h:55}))await pane.getByRole("spinbutton",{name:`${human.input.outcome_id} ${axis}`,exact:true}).fill(String(value));
  await mark("10-human-boundary-edit");await page.waitForTimeout(1400);
  const answer=page.waitForResponse(r=>r.request().method()==="POST"&&r.url().includes("/human-requests/"));
  await pane.getByRole("button",{name:"保存当前样例修正",exact:true}).click();const response=await answer;expect(response.ok()).toBe(true);
  await expect.poll(async()=>{const ws=await(await request.get(scene.task_root+"/workspace")).json();return ws.human_requests.find((h:any)=>h.input.id===human.input.id)?.status;}).toBe("applied");
  await page.reload();await expect(pane.locator("svg .bbox-label")).toContainText(["足球"]);await mark("11-correction-restored-after-refresh");
  expect(scene.processing).toBeUndefined();
  await page.getByRole("button",{name:"确认方案并开始处理…",exact:true}).click();await page.getByRole("button",{name:"查看并确认授权",exact:true}).click();
  const dialog=page.getByRole("dialog");await expect(dialog).toContainText("2 张图片");await expect(dialog).toContainText("不可变版本");await mark("12-formal-range-confirmation");await page.waitForTimeout(1700);
  await dialog.getByRole("button",{name:"接受未知费用并执行此范围",exact:true}).click();await mark("13-formal-processing-started");
  await expect.poll(async()=>{if(!scene.processing?.batch_id)return "not-admitted";scene.batch=await(await request.get("/api/batches/"+scene.processing.batch_id)).json();save();return scene.batch.batch.status;},{timeout:180_000,intervals:[2000]}).toBe("awaiting_review");
  for(const image of scene.images){const run=scene.batch.batch.images.find((i:any)=>i.image_id===image.image_id).child_run_id;const state=await(await request.get(`${scene.task_root}/delivery-images/${image.image_id}?source_run_id=${run}`)).json();scene.trace.push({at:Date.now(),label:"Formal annotations before operator review",image:image.name,state});}save();
  await page.reload();await mark("14-formal-images-await-review");
});
