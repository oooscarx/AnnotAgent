import {test,expect} from "@playwright/test";
import {readFileSync,writeFileSync} from "node:fs";
import {join} from "node:path";

// Opt-in LIVE browser rehearsal. All commands go through visible product controls.
// Adjudications below are explicit operator decisions from inspected originals,
// not model ground truth, a fixture, or a measured detection accuracy claim.
test("LIVE formal correction, scoped exclusions and portable training ZIP",async({page,request},info)=>{
  test.skip(process.env.LIVE_DELIVERY_COMPLETE!=="1","Requires explicit permission and isolated LIVE scene");
  const root=process.env.LIVE_RECORDING_WORKSPACE!;
  expect(root).toMatch(/^\/tmp\/AnnotAgent-LIVE-recording-/);
  expect(JSON.parse(readFileSync(join(root,"LIVE_RECORDING.json"),"utf8")).synthetic_model).toBe(false);
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBeUndefined();
  const scene=JSON.parse(readFileSync(join(root,"journey.json"),"utf8"));
  expect(scene.project).toBe("live-robocup-delivery");
  const audit:{at:number;action:string;details?:unknown}[]=[];const started=Date.now();
  const mark=(action:string,details?:unknown)=>{audit.push({at:Date.now()-started,action,details});writeFileSync(info.outputPath("actions.json"),JSON.stringify(audit,null,2));};
  page.on("response",async response=>{if(response.request().method()==="POST"){const path=new URL(response.url()).pathname;if(path.startsWith("/api/")&&!path.endsWith("/session"))mark("HTTP receipt",{path,status:response.status()});}});
  await page.goto(`/projects/${scene.project}/work?task=${scene.task_id}`);
  if(process.env.LIVE_REVISE_SPLIT==="1"){
    const intake=page.getByRole("region",{name:"训练数据交付信息",exact:true});
    await intake.getByRole("button",{name:"查看交付信息",exact:true}).click();
    await intake.getByText("已有数据划分与来源组",{exact:true}).click();
    for(let i=0;i<scene.images.length;i++)if(["color_548575.png","color_289771.png"].includes(scene.images[i].name))await intake.getByLabel(`已有划分 ${i+1}`,{exact:true}).selectOption(scene.images[i].name==="color_548575.png"?"train":"val");
    await intake.getByRole("button",{name:"保存交付信息",exact:true}).click();await expect(intake.getByText("已保存到服务器",{exact:true})).toBeVisible();
    await intake.getByRole("button",{name:"收起信息",exact:true}).click();mark("Explicit split revision: training includes both classes; old failed package preserved");
  }
  await page.getByText("检查正式训练图片（整图审核）",{exact:true}).click();
  const review=page.getByRole("region",{name:"训练图片整图审核",exact:true});
  const stateFor=async(image:string,run:string)=>(await request.get(`${scene.task_root}/delivery-images/${image}?source_run_id=${run}`)).json();
  for(const item of scene.images){
    const name=item.name;const source=scene.batch.batch.images.find((i:any)=>i.image_id===item.image_id).child_run_id;
    await review.getByLabel("图片",{exact:true}).selectOption(item.image_id);
    await review.getByLabel("正式标注来源",{exact:true}).selectOption(source);
    let state=await stateFor(item.image_id,source);
    if(state.confirmation_current){mark("Already saved image receipt; no repeated mutation",{name});continue;}
    await expect(review.locator("rect.aa-annotation-shape")).toHaveCount(state.snapshot.annotations.filter((a:any)=>a.review_status!=="rejected").length);
    await review.locator("svg.annotation-canvas").scrollIntoViewIfNeeded();mark("Inspect original and formal candidates",{name});
    await page.waitForTimeout(1200);
    if(!["color_548575.png","color_289771.png"].includes(name)){
      await review.getByLabel("检查备注／排除原因").fill("人工检查发现机器人漏检或边界仍不可靠；本次演示不将未完成的图片纳入训练包。不是确认负样本。");
      await review.getByRole("button",{name:"明确排除此图",exact:true}).click();
      await expect(review).toContainText("此快照已有整图决定：excluded");mark("Explicitly excluded unresolved image",{name});continue;
    }
    if(name==="color_548575.png"){
      const additions=[[61,185,51,71],[106,181,44,68]];
      for(const pixels of additions){
        // Do not duplicate an already-saved human correction when recovering this rehearsal.
        state=await stateFor(item.image_id,source);
        const manual=state.snapshot.annotations.filter((a:any)=>a.source==="human");
        if(manual.length>additions.indexOf(pixels))continue;
        await review.getByRole("button",{name:"新增漏标目标框",exact:true}).click();
        await review.getByLabel("对象类别",{exact:true}).selectOption("robot");
        const svg=review.locator("svg.annotation-canvas");await svg.scrollIntoViewIfNeeded();
        const rect=review.locator(".annotation-shape.selected rect.aa-annotation-shape");
        const start=await rect.boundingBox();const frame=await svg.boundingBox();expect(start&&frame).toBeTruthy();
        await page.mouse.move(start!.x+start!.width/2,start!.y+start!.height/2);await page.mouse.down();
        await page.mouse.move(frame!.x+(pixels[0]/544+.075)*frame!.width,frame!.y+(pixels[1]/448+.075)*frame!.height,{steps:18});await page.mouse.up();
        const handle=review.getByRole("button",{name:"Resize bounding box from se corner",exact:true});const h=await handle.boundingBox();
        await page.mouse.move(h!.x+h!.width/2,h!.y+h!.height/2);await page.mouse.down();
        await page.mouse.move(frame!.x+(pixels[0]+pixels[2])/544*frame!.width,frame!.y+(pixels[1]+pixels[3])/448*frame!.height,{steps:18});await page.mouse.up();
        await review.getByLabel("检查备注／排除原因").fill("人工观察原图，补充场边可见机器人；模型没有产生此框。尚需对象审核。");
        const receipt=page.waitForResponse(r=>r.url().endsWith("/missing-objects")&&r.request().method()==="POST");
        await review.getByRole("button",{name:"保存新增目标框",exact:true}).click();const saved=await receipt;expect(await saved.text()).not.toContain('"error"');expect(saved.ok()).toBe(true);
        await expect(review).toContainText("新增目标框已保存为待审核");mark("Human missing object saved",{name,pixels});
      }
    }
    state=await stateFor(item.image_id,source);
    const listToggle=review.getByRole("button",{name:/Annotation list|标注列表/});if(await listToggle.getAttribute("aria-expanded")==="false")await listToggle.click();
    for(let i=0;i<state.snapshot.annotations.length;i++){
      if(state.snapshot.annotations[i].review_status==="human_accepted")continue;
      await review.locator(".canvas-annotation-list li button").nth(i).click();
      await review.getByRole("button",{name:"接受这个对象",exact:true}).click();
      await expect(review).toContainText("对象决定已保存");
      await expect(review.getByRole("button",{name:"接受这个对象",exact:true})).toBeDisabled();
    }
    await expect(review).toContainText("未解决对象 0");
    await review.getByLabel("检查备注／排除原因").fill("已逐个检查可见目标并处理漏标；操作者审核，不代表自动模型准确率。");
    await review.getByRole("button",{name:"确认整张图标注完整",exact:true}).click();
    await expect(review).toContainText("此快照已有整图决定：positive_complete");mark("Whole image confirmed separately",{name});
    await review.locator("svg.annotation-canvas").scrollIntoViewIfNeeded();await page.screenshot({path:info.outputPath(name)});
  }
  await page.getByText("检查正式训练图片（整图审核）",{exact:true}).click();
  const delivery=page.getByRole("region",{name:"训练数据包交付",exact:true});
  await delivery.getByRole("button",{name:"检查当前打包范围",exact:true}).click();await expect(delivery).toContainText("整图决定齐全");
  mark("Actual package scope confirmation");await page.waitForTimeout(1600);
  await delivery.getByRole("button",{name:"确认并生成训练数据包",exact:true}).click();
  const link=delivery.getByRole("link",{name:"下载数据集 ZIP",exact:true});await expect(link).toBeVisible({timeout:30_000});
  await expect(delivery).toContainText("已纳入 2 张原图、5 个正式对象");await expect(delivery).toContainText("排除 3");
  mark("Package Ready with explicit exclusions",{url:page.url()});await page.screenshot({path:info.outputPath("package-ready.png")});
  await page.reload();await expect(link).toBeVisible();
  const download=page.waitForEvent("download");await link.click();const file=await download;await file.saveAs(info.outputPath("live-robocup-training.zip"));expect(await file.failure()).toBeNull();
  mark("Browser downloaded real ZIP",{filename:file.suggestedFilename()});await page.waitForTimeout(2000);
});
