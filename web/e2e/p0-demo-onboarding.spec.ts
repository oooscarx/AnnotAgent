import { expect, test } from "@playwright/test";
import { readFileSync, statSync } from "node:fs";
import { randomUUID } from "node:crypto";

type FixtureManifest = {
  fixture:string;
  project:string;
  model_profile_id:string;
};

test("explicit preset Demo creates one isolated task and opens real review with zero model attempts",async({page},testInfo)=>{
  const manifest=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8")) as FixtureManifest;
  expect(manifest.fixture).toBe("external-model-only");
  expect(manifest.project).toMatch(/^TEST-/);

  const startPosts:string[]=[];
  const allWrites:string[]=[];
  page.on("request",request=>{
    if(request.method()!=="GET")allWrites.push(`${request.method()} ${new URL(request.url()).pathname}`);
    if(request.method()==="POST"&&new URL(request.url()).pathname==="/api/demos/start")startPosts.push(request.postData()||"");
  });

  await page.goto(`/projects/${encodeURIComponent(manifest.project)}/work?task=${encodeURIComponent(`new:${manifest.project}`)}`);
  const onboarding=page.getByRole("region",{name:"用示例试试看",exact:true});
  await expect(onboarding).toBeVisible();
  await expect(onboarding).toContainText("不调用模型 · 结果仍需逐图人工审核");
  await expect(onboarding).toContainText("不代表实时模型准确率");
  await page.reload();
  await expect(onboarding).toBeVisible();
  expect(startPosts).toEqual([]);

  const startResponsePromise=page.waitForResponse(response=>response.request().method()==="POST"&&new URL(response.url()).pathname==="/api/demos/start");
  await onboarding.getByRole("button",{name:"体验预置候选",exact:true}).dblclick();
  const startResponse=await startResponsePromise;
  expect(startResponse.status()).toBe(201);
  const receipt=await startResponse.json() as {
    source_mode:string;project_id:string;project_owner_id:string;conversation_id:string;task_id:string;work_route:string;
    source_provenance:{live_inference_occurred:boolean;review_status:string|null};
  };
  expect(receipt.source_mode).toBe("preset_candidates");
  expect(receipt.source_provenance).toMatchObject({live_inference_occurred:false,review_status:"needs_review"});
  await expect(page).toHaveURL(new RegExp(`/projects/${receipt.project_id}/work\\?.*task=${receipt.task_id}`));
  expect(startPosts).toHaveLength(1);
  expect(JSON.parse(startPosts[0])).toEqual({
    command_id:expect.any(String),demo_id:"object-detection-review",demo_version:"1.0.0",
    source_mode:"preset_candidates",model_profile_id:null,
  });

  const taskRoot=`/api/projects/${receipt.project_id}/conversations/${receipt.conversation_id}/tasks/${receipt.task_id}`;
  const workspace=await (await page.request.get(`${taskRoot}/workspace`)).json();
  expect(workspace.mainline.available_actions.map((action:{id:string})=>action.id)).toEqual(["review_delivery_images"]);
  expect(workspace.mainline.formal_source).toMatchObject({kind:"preset_candidate_import",live_inference_occurred:false,model_run_id:null});
  const usage=await (await page.request.get(`${taskRoot}/model-usage`)).json();
  expect(usage.state).toBe("no_model_requests");
  expect(usage.attempts.items).toEqual([]);
  const reviewPage=await (await page.request.get(`${taskRoot}/delivery-review-items?limit=50`)).json();
  expect(reviewPage.items).toHaveLength(6);
  expect(reviewPage.items.flatMap((item:{annotations:unknown[]})=>item.annotations)).toHaveLength(9);
  expect(reviewPage.items.every((item:{child_run_id:string|null})=>item.child_run_id===null)).toBe(true);

  const status=page.getByRole("region",{name:"当前任务状态",exact:true});
  await expect(status).toContainText("6 个结果需要人工判断");
  await status.getByRole("button",{name:"重新定位当前结果",exact:true}).click();
  const review=page.getByRole("region",{name:"当前任务图片结果",exact:true});
  await expect(review).toBeVisible();
  await expect(review).toContainText("预置候选 · 本次没有调用模型 · 必须人工审核");
  await expect(review.getByRole("button",{name:"新增漏标目标框",exact:true})).toHaveCount(0);
  await review.getByRole("button",{name:/Annotation list/}).click();
  const candidate=review.getByRole("list",{name:"Annotations on canvas"}).getByRole("button").first();
  await candidate.click();
  const accept=review.getByRole("button",{name:"接受这个对象",exact:true});
  await expect(accept).toBeEnabled();
  const saveResponsePromise=page.waitForResponse(response=>response.request().method()==="POST"&&new URL(response.url()).pathname.endsWith("/preset-objects"));
  await accept.click();
  const saveResponse=await saveResponsePromise;
  expect(saveResponse.ok(),await saveResponse.text()).toBeTruthy();
  const presetWrites=allWrites.filter(value=>value.endsWith("/preset-objects"));
  expect(presetWrites).toHaveLength(1);
  expect(allWrites.some(value=>value.endsWith("/objects"))).toBe(false);
  await expect(review).toContainText("对象修改已保存");

  await page.reload();
  const restoredReview=page.getByRole("region",{name:"当前任务图片结果",exact:true});
  await expect(restoredReview).toContainText("预置候选 · 本次没有调用模型");
  await expect(restoredReview.getByRole("button",{name:/Annotation list · [1-9]/})).toBeVisible();
  const persisted=await (await page.request.get(`${taskRoot}/delivery-images/${reviewPage.items[0].image_id}`)).json();
  expect(persisted.snapshot.annotations.some((annotation:{review_status:string})=>annotation.review_status==="human_accepted")).toBe(true);
  expect(startPosts).toHaveLength(1);
  await page.screenshot({path:testInfo.outputPath("preset-demo-real-review.png"),fullPage:true});
});

test("an unknown Demo start response recovers by GET without a second POST",async({page})=>{
  const manifest=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8")) as FixtureManifest;
  let posts=0;
  let commandId="";
  page.on("request",request=>{
    if(request.method()==="POST"&&new URL(request.url()).pathname==="/api/demos/start"){
      posts+=1;
      commandId=JSON.parse(request.postData()||"{}").command_id||"";
    }
  });
  let lose=true;
  await page.route("**/api/demos/start",async route=>{
    if(!lose){await route.continue();return;}
    lose=false;
    const response=await route.fetch();
    expect(response.status()).toBe(201);
    await route.abort("connectionfailed");
  });
  await page.goto(`/projects/${encodeURIComponent(manifest.project)}/work?task=${encodeURIComponent(`new:${manifest.project}`)}`);
  const onboarding=page.getByRole("region",{name:"用示例试试看",exact:true});
  await onboarding.getByRole("button",{name:"体验预置候选",exact:true}).click();
  await expect(onboarding.getByRole("alert")).toContainText("刷新只会查询同一命令");
  expect(posts).toBe(1);
  expect(commandId).not.toBe("");

  await page.unroute("**/api/demos/start");
  await page.reload();
  await expect(page).toHaveURL(/\/projects\/demo-object-detection-review-.*\/work\?.*task=/);
  expect(posts).toBe(1);
  const receiptResponse=await page.request.get(`/api/demos/start/${commandId}`);
  expect(receiptResponse.ok(),await receiptResponse.text()).toBeTruthy();
  const receipt=await receiptResponse.json();
  expect(receipt.replayed).toBe(true);
  expect(receipt.source_provenance.live_inference_occurred).toBe(false);
});

test("live Demo creates the bounded mainline task without inference or preset fallback",async({page})=>{
  const manifest=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8")) as FixtureManifest;
  const posts:string[]=[];
  page.on("request",request=>{
    if(request.method()==="POST"&&new URL(request.url()).pathname==="/api/demos/start")posts.push(request.postData()||"");
  });
  await page.goto(`/projects/${encodeURIComponent(manifest.project)}/work?task=${encodeURIComponent(`new:${manifest.project}`)}`);
  const onboarding=page.getByRole("region",{name:"用示例试试看",exact:true});
  const model=onboarding.getByRole("combobox",{name:"示例视觉模型",exact:true});
  await expect(model).toBeVisible();
  await model.selectOption(manifest.model_profile_id);

  const responsePromise=page.waitForResponse(response=>response.request().method()==="POST"&&new URL(response.url()).pathname==="/api/demos/start");
  await onboarding.getByRole("button",{name:"用所选模型创建任务",exact:true}).click();
  const response=await responsePromise;
  expect(response.status()).toBe(201);
  const receipt=await response.json() as {source_mode:string;project_id:string;conversation_id:string;task_id:string;source_provenance:{live_inference_occurred:boolean}};
  expect(receipt.source_mode).toBe("live_model");
  expect(receipt.source_provenance.live_inference_occurred).toBe(false);
  expect(JSON.parse(posts[0])).toMatchObject({source_mode:"live_model",model_profile_id:manifest.model_profile_id});
  expect(posts).toHaveLength(1);

  const taskRoot=`/api/projects/${receipt.project_id}/conversations/${receipt.conversation_id}/tasks/${receipt.task_id}`;
  const workspace=await (await page.request.get(`${taskRoot}/workspace`)).json();
  expect(workspace.mainline.available_actions.map((action:{id:string})=>action.id)).toEqual(["prepare_delivery_schema"]);
  expect(workspace.mainline.formal_source).toBeNull();
  const usage=await (await page.request.get(`${taskRoot}/model-usage`)).json();
  expect(usage.state).toBe("no_model_requests");
  expect(usage.attempts.items).toEqual([]);
  const review=await page.request.get(`${taskRoot}/delivery-review-items?limit=50`);
  const reviewBody=await review.text();
  expect(review.ok(),reviewBody).toBeTruthy();
  const liveReview=JSON.parse(reviewBody);
  expect(liveReview.items).toHaveLength(6);
  expect(liveReview.items.every((item:{annotations:unknown[];child_run_id:string|null})=>item.annotations.length===0&&item.child_run_id===null)).toBe(true);
  await expect(page.getByRole("region",{name:"当前任务图片结果",exact:true})).toHaveCount(0);
  await expect(page.getByText("预置候选 · 本次没有调用模型",{exact:false})).toHaveCount(0);
});

test("a rejected live Demo remains failed and never changes to preset candidates",async({page})=>{
  const manifest=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8")) as FixtureManifest;
  let commandId="";
  await page.route("**/api/demos/start",async route=>{
    const request=route.request();
    const body=JSON.parse(request.postData()||"{}");
    commandId=body.command_id;
    await route.continue({postData:JSON.stringify({...body,model_profile_id:"00000000-0000-4000-8000-000000000001"})});
  });
  await page.goto(`/projects/${encodeURIComponent(manifest.project)}/work?task=${encodeURIComponent(`new:${manifest.project}`)}`);
  const onboarding=page.getByRole("region",{name:"用示例试试看",exact:true});
  await onboarding.getByRole("combobox",{name:"示例视觉模型",exact:true}).selectOption(manifest.model_profile_id);
  const actualResponsePromise=page.waitForResponse(response=>response.request().method()==="POST"&&new URL(response.url()).pathname==="/api/demos/start");
  await onboarding.getByRole("button",{name:"用所选模型创建任务",exact:true}).click();
  const response=await actualResponsePromise;
  expect(response.status()).toBe(422);
  await expect(onboarding.getByRole("alert")).toContainText("没有创建任务、调用模型或改用预置候选");
  expect(await page.evaluate(()=>Object.keys(localStorage).filter(key=>key.startsWith("annotagent.demo.pending.v1.")))).toEqual([]);
  const recovery=await page.request.get(`/api/demos/start/${commandId}`);
  expect(recovery.status()).toBe(404);
  await expect(page).toHaveURL(new RegExp(`/projects/${manifest.project}/work`));
  await expect(page.getByText("预置候选 · 本次没有调用模型",{exact:false})).toHaveCount(0);
});

test("preset Demo reaches a real ZIP only after every image is human reviewed",async({page},testInfo)=>{
  test.setTimeout(60_000);
  const manifest=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8")) as FixtureManifest;
  await page.goto(`/projects/${encodeURIComponent(manifest.project)}/work?task=${encodeURIComponent(`new:${manifest.project}`)}`);
  const onboarding=page.getByRole("region",{name:"用示例试试看",exact:true});
  const startResponsePromise=page.waitForResponse(response=>response.request().method()==="POST"&&new URL(response.url()).pathname==="/api/demos/start");
  await onboarding.getByRole("button",{name:"体验预置候选",exact:true}).click();
  const receipt=await (await startResponsePromise).json() as {source_mode:string;project_id:string;conversation_id:string;task_id:string};
  expect(receipt.source_mode).toBe("preset_candidates");
  await expect(page).toHaveURL(new RegExp(`/projects/${receipt.project_id}/work\\?.*task=${receipt.task_id}`));
  const taskRoot=`/api/projects/${receipt.project_id}/conversations/${receipt.conversation_id}/tasks/${receipt.task_id}`;
  const session=await (await page.request.get("/api/session")).json();
  const headers={"x-annotagent-csrf":session.csrf_token as string};
  const reviewPage=await (await page.request.get(`${taskRoot}/delivery-review-items?limit=50`)).json();

  for(const item of reviewPage.items as Array<{image_id:string;annotations:Array<{annotation_id:string;label:string;value:unknown;review_status:string}>}>){
    let current=await (await page.request.get(`${taskRoot}/delivery-images/${item.image_id}`)).json();
    for(const annotation of item.annotations.filter(candidate=>candidate.review_status==="needs_review")){
      const response=await page.request.post(`${taskRoot}/delivery-images/${item.image_id}/preset-objects`,{headers,data:{
        command_id:randomUUID(),intent_revision:reviewPage.intent_revision,intent_sha256:reviewPage.intent_sha256,
        annotation_id:annotation.annotation_id,expected_snapshot_sha256:current.snapshot.sha256,label:annotation.label,
        value:annotation.value,review_status:"human_accepted",reason:"TEST human inspected preset candidate",
      }});
      expect(response.ok(),await response.text()).toBeTruthy();
      current=await (await page.request.get(`${taskRoot}/delivery-images/${item.image_id}`)).json();
    }
    const confirm=await page.request.post(`${taskRoot}/delivery-images/${item.image_id}`,{headers,data:{
      command_id:randomUUID(),intent_revision:reviewPage.intent_revision,intent_sha256:reviewPage.intent_sha256,
      image_id:item.image_id,source_run_id:null,expected_snapshot_sha256:current.snapshot.sha256,
      expected_review_revision:current.review?.revision||0,
      decision:current.snapshot.annotations.length?"positive_complete":"negative_confirmed",
      reason:"TEST whole image inspected",confirmed:true,
    }});
    expect(confirm.ok(),`${item.image_id}: ${await confirm.text()} snapshot=${JSON.stringify(current.snapshot.annotations)}`).toBeTruthy();
  }

  const completedWorkspace=await (await page.request.get(`${taskRoot}/workspace`)).json();
  expect(completedWorkspace.mainline.available_actions.map((action:{id:string})=>action.id)).toEqual(["authorize_training_package"]);
  await page.reload();
  await expect(page).toHaveURL(new RegExp(`/projects/${receipt.project_id}/work\\?.*task=${receipt.task_id}`));
  const status=page.getByRole("region",{name:"当前任务状态",exact:true});
  await expect(status).toContainText("可以生成训练数据包");
  await status.getByRole("button",{name:"生成训练数据包",exact:true}).click();
  const delivery=page.getByRole("region",{name:"训练数据包交付",exact:true});
  await expect(delivery).toBeVisible();
  await delivery.getByRole("button",{name:"刷新审核与打包状态",exact:true}).click();
  await expect(delivery).toContainText("正式审核齐全");
  await delivery.getByRole("button",{name:"允许审核齐全后自动打包",exact:true}).click();
  const downloadLink=delivery.getByRole("link",{name:"下载数据集 ZIP",exact:true});
  await expect(downloadLink).toBeVisible({timeout:30_000});
  const downloadStarted=page.waitForEvent("download");
  await downloadLink.click();
  const download=await downloadStarted;
  const zipPath=testInfo.outputPath("preset-demo-human-reviewed.zip");
  await download.saveAs(zipPath);
  expect(await download.failure()).toBeNull();
  expect(statSync(zipPath).size).toBeGreaterThan(100);
  const usage=await (await page.request.get(`${taskRoot}/model-usage`)).json();
  expect(usage.state).toBe("no_model_requests");
  expect(usage.attempts.items).toEqual([]);
  await page.reload();
  await expect(page.getByRole("region",{name:"当前任务状态",exact:true})).toContainText("训练数据包已准备好");
  await expect(page.getByRole("link",{name:"下载数据集 ZIP",exact:true})).toBeVisible();
  await page.screenshot({path:testInfo.outputPath("preset-demo-package-ready.png"),fullPage:true});
});
