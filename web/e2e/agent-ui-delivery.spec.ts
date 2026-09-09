import {test,expect} from "@playwright/test";
import {readFileSync} from "node:fs";

test("delivery intake and formal review restore through real HttpAdapter without model or package writes",async({page,request},info)=>{
  test.skip(!process.env.DELIVERY_SCENE_MANIFEST,"Requires an explicitly seeded isolated delivery scene");
  const health=await request.get("/api/health");expect(health.headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const scene=JSON.parse(readFileSync(process.env.DELIVERY_SCENE_MANIFEST!,"utf8"));
  expect(scene.project).toMatch(/^TEST-delivery-/);
  const image=scene.images[0];
  const source=scene.batch.batch.images.find((i:{image_id:string})=>i.image_id===image.image_id).child_run_id;
  expect(source).toBeTruthy();
  const state=await (await request.get(`${scene.task_root}/delivery-images/${image.image_id}?source_run_id=${source}`)).json();
  let missing=0;
  for(const item of scene.images){let value=await (await request.get(`${scene.task_root}/delivery-images/${item.image_id}`)).json();if(value.review?.input.source_run_id)value=await (await request.get(`${scene.task_root}/delivery-images/${item.image_id}?source_run_id=${value.review.input.source_run_id}`)).json();if(!value.confirmation_current)missing++;}
  const writes:string[]=[];page.on("request",r=>{if(!["GET","HEAD"].includes(r.method()))writes.push(`${r.method()} ${new URL(r.url()).pathname}`);});
  await page.goto(`/projects/${encodeURIComponent(scene.project)}/work?task=${scene.task_id}`);
  await expect(page.getByRole("region",{name:"训练数据交付信息",exact:true})).toContainText("12 张图片");
  await page.getByText("检查正式训练图片（整图审核）",{exact:true}).click();
  const review=page.getByRole("region",{name:"训练图片整图审核",exact:true});
  await review.getByLabel("图片",{exact:true}).selectOption(image.image_id);
  await review.getByLabel("正式标注来源",{exact:true}).selectOption(source);
  await expect(review).toContainText(`未解决对象 ${state.unresolved_objects}`);
  await review.locator("svg").scrollIntoViewIfNeeded();
  await page.screenshot({path:info.outputPath("delivery-formal-review-TEST.png")});
  if(state.unresolved_objects)await expect(review.getByRole("button",{name:"确认整张图标注完整",exact:true})).toBeDisabled();
  await expect(review.getByRole("button",{name:"确认整张图没有目标",exact:true})).toBeDisabled();
  await expect(review.locator("svg image")).toHaveAttribute("href",image.url);
  await page.reload();
  await expect(review.getByLabel("正式标注来源",{exact:true})).toHaveValue(source);
  await expect(review).toContainText(`未解决对象 ${state.unresolved_objects}`);
  await page.getByRole("button",{name:"检查当前打包范围",exact:true}).click();
  await expect(page.getByRole("region",{name:"训练数据包交付",exact:true})).toContainText(missing?`还有 ${missing} 张未完成当前整图确认`:"整图决定齐全");
  await expect(page.getByRole("link",{name:"下载数据集 ZIP",exact:true})).toHaveCount(0);
  expect(writes).toEqual([]);
  await page.screenshot({path:info.outputPath("delivery-unresolved-package-TEST.png")});
});

test("explicit TEST classroom review delivers a real ZIP through the conversation",async({page,request},info)=>{
  test.skip(process.env.DELIVERY_FINISH_SCENE!=="1","Explicit opt-in: changes only the marked TEST scene review state");
  test.setTimeout(120_000);
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const scene=JSON.parse(readFileSync(process.env.DELIVERY_SCENE_MANIFEST!,"utf8"));
  expect(scene.project).toMatch(/^TEST-delivery-/);
  const writes:string[]=[];
  page.on("request",r=>{if(!["GET","HEAD"].includes(r.method()))writes.push(new URL(r.url()).pathname);});
  await page.goto(`/projects/${scene.project}/work?task=${scene.task_id}`);
  await page.getByText("检查正式训练图片（整图审核）",{exact:true}).click();
  const review=page.getByRole("region",{name:"训练图片整图审核",exact:true});
  for(let i=0;i<scene.images.length;i++){
    const image=scene.images[i];
    await review.getByLabel("图片",{exact:true}).selectOption(image.image_id);
    const source=scene.batch.batch.images.find((v:{image_id:string})=>v.image_id===image.image_id).child_run_id;
    const state=await (await request.get(`${scene.task_root}/delivery-images/${image.image_id}${source?`?source_run_id=${source}`:""}`)).json();
    if(state.confirmation_current)continue;
    if(!source){
      await review.getByLabel("检查备注／排除原因").fill("TEST explicit exclusion: formal request budget exhausted before this image; no completeness claim");
      await review.getByRole("button",{name:"明确排除此图",exact:true}).click();
      await expect(review).toContainText("此快照已有整图决定：excluded");
      continue;
    }
    await review.getByLabel("正式标注来源",{exact:true}).selectOption(source);
    await expect(review.locator("rect.aa-annotation-shape")).toHaveCount(state.snapshot.annotations.filter((a:{review_status:string})=>a.review_status!=="rejected").length);
    // Synthetic scene: image 10 is blank; its scripted false positives are explicitly rejected.
    const blank=i===10;
    let unresolved=state.unresolved_objects;
    for(let j=0;j<(blank?state.snapshot.annotations.filter((a:{review_status:string})=>a.review_status!=="rejected").length:3);j++){
      await review.locator("rect.aa-annotation-shape").nth(blank?0:j).click();
      const action=review.getByRole("button",{name:blank?"拒绝这个对象":"接受这个对象",exact:true});
      if(await action.isEnabled()){
        await action.click();
        unresolved--;await expect(review).toContainText(`未解决对象 ${unresolved}`);
      }
    }
    await review.getByRole("button",{name:blank?"确认整张图没有目标":"确认整张图标注完整",exact:true}).click();
    await expect(review).toContainText(`此快照已有整图决定：${blank?"negative_confirmed":"positive_complete"}`);
  }
  const delivery=page.getByRole("region",{name:"训练数据包交付",exact:true});
  await delivery.getByRole("button",{name:"检查当前打包范围",exact:true}).click();
  await expect(delivery).toContainText("整图决定齐全");
  await delivery.getByRole("button",{name:"确认并生成训练数据包",exact:true}).click();
  await expect(delivery.getByRole("link",{name:"下载数据集 ZIP",exact:true})).toBeVisible({timeout:30_000});
  await expect(delivery).toContainText("已纳入 11 张原图、30 个正式对象");
  await expect(delivery).toContainText("确认负样本 1 · 排除 1");
  const url=page.url();
  const admissions=writes.filter(p=>p.endsWith("/delivery-packages"));expect(admissions).toHaveLength(1);
  const before=writes.length;
  await page.reload();
  await expect(delivery.getByRole("link",{name:"下载数据集 ZIP",exact:true})).toBeVisible();
  expect(page.url()).toBe(url);expect(writes).toHaveLength(before);
  const downloaded=page.waitForEvent("download");
  await delivery.getByRole("link",{name:"下载数据集 ZIP",exact:true}).click();
  const download=await downloaded;
  await download.saveAs(info.outputPath("classroom-delivery-TEST.zip"));
  expect(await download.failure()).toBeNull();
  await delivery.scrollIntoViewIfNeeded();
  await page.screenshot({path:info.outputPath("classroom-delivery-ready-TEST.png")});
  await info.attach("delivery-evidence",{body:JSON.stringify({url,download:download.suggestedFilename(),writes},null,2),contentType:"application/json"});
});

test("formal object correction survives HTTP save and refresh without confirming the whole image",async({page,request},info)=>{
  test.skip(!process.env.DELIVERY_SCENE_MANIFEST||process.env.DELIVERY_EDIT_SCENE!=="1","Explicit opt-in: changes one TEST formal object and invalidates its whole-image receipt");
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const scene=JSON.parse(readFileSync(process.env.DELIVERY_SCENE_MANIFEST!,"utf8"));
  expect(scene.project).toMatch(/^TEST-delivery-/);
  const image=scene.images[0];
  const source=scene.batch.batch.images.find((i:{image_id:string})=>i.image_id===image.image_id).child_run_id;
  const endpoint=`/api/projects/${scene.project}/conversations/${scene.conversation_id}/tasks/${scene.task_id}/delivery-images/${image.image_id}`;
  const beforeState=await (await request.get(`${endpoint}?source_run_id=${source}`)).json();
  const writes:string[]=[];
  page.on("request",r=>{if(!["GET","HEAD"].includes(r.method()))writes.push(new URL(r.url()).pathname);});
  await page.goto(`/projects/${scene.project}/work?task=${scene.task_id}&delivery_image=${image.image_id}&delivery_run=${source}`);
  const review=page.getByRole("region",{name:"训练图片整图审核",exact:true});
  await expect(review).toContainText(`未解决对象 ${beforeState.unresolved_objects}`);
  await review.locator("rect.aa-annotation-shape").first().click();
  const move=review.getByRole("button",{name:/Move box with arrow keys|方向键移动/});
  await move.focus();await move.press("ArrowRight");
  await expect(review).toContainText("尚未保存");
  await page.getByText("检查正式训练图片（整图审核）",{exact:true}).click();
  await page.getByText("检查正式训练图片（整图审核）",{exact:true}).click();
  await expect(review).toContainText("尚未保存");
  const responsePromise=page.waitForResponse(r=>r.url().includes("/objects")&&r.request().method()==="POST");
  await review.getByRole("button",{name:"保存对象修改",exact:true}).click();
  const response=await responsePromise;expect(response.ok()).toBeTruthy();
  const revision=await response.json();
  await expect(review).toContainText("对象决定已保存");
  const afterState=await (await request.get(`${endpoint}?source_run_id=${source}`)).json();
  await expect(review).toContainText(`未解决对象 ${afterState.unresolved_objects}`);
  await page.reload();
  await expect(review).toContainText(`未解决对象 ${afterState.unresolved_objects}`);
  await expect(review).toContainText("尚未整图确认");
  expect(writes).toHaveLength(1);expect(writes[0]).toBe(`${endpoint}/objects`);
  expect(revision.revision_id).toBeTruthy();
  expect(revision.after.value.rect[0]).toBeCloseTo(revision.before.value.rect[0]+0.001,6);
  const persisted=await request.get(`${endpoint}?source_run_id=${source}`);
  expect(persisted.ok()).toBeTruthy();
  const stored=(await persisted.json()).snapshot.annotations.find((a:{id:string})=>a.id===revision.annotation_id);
  expect(stored.value).toEqual(revision.after.value);
  await review.locator("svg").scrollIntoViewIfNeeded();
  await page.screenshot({path:info.outputPath("object-correction-HTTP-TEST.png")});
});
