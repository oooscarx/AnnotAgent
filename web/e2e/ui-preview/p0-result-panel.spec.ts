import {expect,test} from "@playwright/test";
import {resolve} from "node:path";

const pixel="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=";
const harness=`/@fs/${resolve("e2e/ui-preview/delivery-review-harness.tsx")}`;

test("three terminal Sample images appear and the server focus is applied only once",async({page})=>{
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async([path,imageUrl])=>{
    const {React,createRoot,P0ResultPanel}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    const annotation=(id:string,image_id:string,label:string,rect:number[])=>({id,image_id,task_id:"objects",label,value:{kind:"bounding_box",rect},attributes:{},source:"model",review_status:"needs_review",provenance:{},created_at:"TEST"});
    const cup=annotation("cup-one","image-1","cup",[0.1,0.1,0.2,0.2]);
    const bottle=annotation("bottle-one","image-1","bottle",[0.5,0.1,0.2,0.4]);
    const second=annotation("cup-two","image-2","cup",[0.2,0.3,0.3,0.3]);
    const selection=(candidate:{id:string;image_id:string;label:string},artifact:string,revision:string)=>({project_id:"project",conversation_id:"conversation",task_id:"task",project_schema_revision:"schema",image:{image_id:candidate.image_id,sha256:`sha-${candidate.image_id}`},sample:{draft_id:"draft",draft_revision:4,sample_test_id:"sample"},candidate:{candidate_id:candidate.id,source_artifact_id:artifact},annotation:{kind:"bounding_box",label:candidate.label},result_revision:revision});
    const sample_result={project_id:"project",conversation_id:"conversation",task_id:"task",project_schema_revision:"schema",draft_id:"draft",draft_revision:4,sample_test_id:"sample",images:[
      {image_id:"image-1",image_sha256:"sha-image-1",result_revision:"result-1",candidates:[{candidate_id:cup.id,selection:selection(cup,"artifact-cup","result-1")},{candidate_id:bottle.id,selection:selection(bottle,"artifact-bottle","result-1")}],annotations:[cup,bottle]},
      {image_id:"image-2",image_sha256:"sha-image-2",result_revision:"result-2",candidates:[{candidate_id:second.id,selection:selection(second,"artifact-second","result-2")}],annotations:[second]},
      {image_id:"image-3",image_sha256:"sha-image-3",result_revision:"result-3",candidates:[],annotations:[]},
    ]};
    const view={kind:"sample_feedback",images:[{id:"image-1",name:"one",src:imageUrl},{id:"image-2",name:"two",src:imageUrl},{id:"image-3",name:"empty",src:imageUrl}],labels:[{stable_id:"cup",display_name:"杯子"},{stable_id:"bottle",display_name:"瓶子"}],sample_result,focus:{mode:"sample",image_id:"image-1",candidate_id:"bottle-one",result_revision:"result-1",reason:"瓶子边界需要判断"},actions:[{id:"sample_feedback",available:true,reason:null}]};
    const root=createRoot(host);
    const render=()=>root.render(React.createElement(P0ResultPanel,{service:{},projectId:"project",taskId:"task",view}));
    Object.assign(window,{p0ResultTest:{render}});render();
  },[harness,pixel]);

  await expect(page.getByText("需要判断：瓶子边界需要判断",{exact:true})).toBeVisible();
  await expect(page.getByRole("heading",{name:"检查样例结果 · 3 张"})).toBeVisible();
  await expect(page.getByText("样例反馈",{exact:true})).toBeVisible();
  await expect(page.getByRole("tab",{name:"正式 Batch"})).toHaveCount(0);
  const probe=page.locator(".delivery-review .canvas-dimension-probe");
  await expect(probe).toHaveCSS("position","absolute");
  const probeBox=await probe.boundingBox();
  expect(probeBox?.width).toBeLessThanOrEqual(1.5);
  expect(probeBox?.height).toBeLessThanOrEqual(1.5);
  const shellBox=await page.locator(".delivery-review .canvas-shell").boundingBox();
  const canvasBox=await page.locator(".delivery-review .annotation-canvas").boundingBox();
  expect(canvasBox?.width).toBeGreaterThan(100);
  expect(canvasBox!.width).toBeLessThanOrEqual(shellBox!.width+1);
  await page.getByRole("button",{name:"Zoom in"}).click();
  await expect(page.getByText("110%",{exact:true})).toBeVisible();
  await page.evaluate(()=>(window as unknown as {p0ResultTest:{render:()=>void}}).p0ResultTest.render());
  await expect(page.getByText("110%",{exact:true})).toBeVisible();
  await page.getByRole("button",{name:"Fit image"}).click();
  await expect(page.getByText("100%",{exact:true})).toBeVisible();
  await page.getByRole("button",{name:/Annotation list/}).click();
  await expect(page.getByRole("button",{name:/瓶子/})).toHaveAttribute("aria-pressed","true");
  await page.getByRole("button",{name:/杯子/}).first().click();
  await expect(page.getByRole("button",{name:/杯子/}).first()).toHaveAttribute("aria-pressed","true");
  await page.evaluate(()=>(window as unknown as {p0ResultTest:{render:()=>void}}).p0ResultTest.render());
  await expect(page.getByRole("button",{name:/杯子/}).first()).toHaveAttribute("aria-pressed","true");
  await page.getByLabel("图片",{exact:true}).selectOption("image-3");
  await expect(page.getByText("这张图片没有当前 Sample Test 结果。")).toHaveCount(0);
  await expect(page.getByRole("button",{name:/Annotation list · 0/})).toBeVisible();
  await expect(page.getByRole("button",{name:"这个样例框有问题",exact:true})).toBeDisabled();
});

test("the fitted result canvas stays within mobile and 200%-equivalent viewports",async({page})=>{
  await page.setViewportSize({width:720,height:450});
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async([path,imageUrl])=>{
    const {React,createRoot,P0ResultPanel}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    const annotation={id:"candidate",image_id:"image",task_id:"objects",label:"cup",value:{kind:"bounding_box",rect:[0.1,0.1,0.2,0.2]},attributes:{},source:"model",review_status:"needs_review",provenance:{},created_at:"TEST"};
    const view={kind:"sample_feedback",images:[{id:"image",name:"one",src:imageUrl}],labels:[{stable_id:"cup",display_name:"杯子"}],sample_result:{project_id:"project",conversation_id:"conversation",task_id:"task",project_schema_revision:"schema",draft_id:"draft",draft_revision:1,sample_test_id:"sample",images:[{image_id:"image",image_sha256:"pixels",result_revision:"result",candidates:[{candidate_id:"candidate",selection:null}],annotations:[annotation]}]},focus:null,actions:[]};
    createRoot(host).render(React.createElement(P0ResultPanel,{service:{},projectId:"project",taskId:"task",view}));
  },[harness,pixel]);
  const assertContained=async()=>{
    const canvas=await page.locator(".delivery-review .annotation-canvas").boundingBox();
    const viewport=page.viewportSize()!;
    expect(canvas).not.toBeNull();
    expect(canvas!.x).toBeGreaterThanOrEqual(0);
    expect(canvas!.x+canvas!.width).toBeLessThanOrEqual(viewport.width+1);
  };
  await assertContained();
  await page.setViewportSize({width:390,height:844});
  await assertContained();
  await expect(page.getByRole("button",{name:"Fit image"})).toBeVisible();
});

test("legacy coarse outcomes never enter the terminal Sample review layer",async({page})=>{
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async([path,imageUrl])=>{
    const {React,createRoot,P0ResultPanel}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    const annotation=(id:string,label:string,rect:number[])=>({id,image_id:"image",task_id:"objects",label,value:{kind:"bounding_box",rect},attributes:{},source:"model",review_status:"needs_review",provenance:{},created_at:"TEST"});
    const coarse=annotation("coarse","coarse",[0,0,1,1]);
    const terminal=annotation("terminal","cup",[0.2,0.2,0.2,0.2]);
    const selection={project_id:"project",conversation_id:"conversation",task_id:"task",project_schema_revision:"schema",image:{image_id:"image",sha256:"pixels"},sample:{draft_id:"draft",draft_revision:2,sample_test_id:"sample"},candidate:{candidate_id:"terminal",source_artifact_id:"artifact-terminal"},annotation:{kind:"bounding_box",label:"cup"},result_revision:"result"};
    const view={kind:"sample_feedback",images:[{id:"image",name:"one",src:imageUrl}],labels:[{stable_id:"cup",display_name:"终端候选"},{stable_id:"coarse",display_name:"中间粗框"}],sample_result:{project_id:"project",conversation_id:"conversation",task_id:"task",project_schema_revision:"schema",draft_id:"draft",draft_revision:2,sample_test_id:"sample",images:[{image_id:"image",image_sha256:"pixels",result_revision:"result",candidates:[{candidate_id:"terminal",selection}],annotations:[coarse,terminal]}]},focus:null,actions:[]};
    createRoot(host).render(React.createElement(P0ResultPanel,{service:{},projectId:"project",taskId:"task",view}));
  },[harness,pixel]);
  await expect(page.getByRole("alert")).toContainText("已隐藏 1 个");
  await expect(page.getByRole("button",{name:/Annotation list · 1/})).toBeVisible();
  await page.getByRole("button",{name:/Annotation list · 1/}).click();
  await expect(page.getByRole("button",{name:/终端候选/})).toBeVisible();
  await expect(page.getByRole("button",{name:/中间粗框/})).toHaveCount(0);
});

test("server actions keep unavailable formal edits and decisions disabled",async({page})=>{
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async([path,imageUrl])=>{
    const {React,createRoot,P0ResultPanel}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    const annotation={id:"object",image_id:"image",task_id:"objects",label:"cup",value:{kind:"bounding_box",rect:[0.1,0.1,0.2,0.2]},attributes:{},source:"model",review_status:"needs_review",provenance:{},created_at:"TEST"};
    const service={image:async()=>({intent_revision:1,intent_sha256:"intent",snapshot:{image_id:"image",source_run_id:"child",sha256:"snapshot",content_sha256:"pixels",annotations:[annotation]},sources:[],review:null,confirmation_current:false,accepted_objects:0,unresolved_objects:1,notice:"TEST"}),editObject:async()=>{throw new Error("must remain disabled");},createObject:async()=>{throw new Error("must remain disabled");},confirmImage:async()=>{throw new Error("must remain disabled");}};
    const view={kind:"formal_review",images:[{id:"image",name:"one",src:imageUrl}],labels:[{stable_id:"cup",display_name:"杯子"}],formal_result:{project_id:"project",task_id:"task",processing_operation_id:"operation",batch_id:"batch",workflow_version:"workflow@1",status:"completed",images:[{image_id:"image",child_run_id:"child"}]},focus:{mode:"formal",image_id:"image",candidate_id:"object",result_revision:"snapshot",reason:"等待服务端开放审核"},actions:[]};
    createRoot(host).render(React.createElement(P0ResultPanel,{service,projectId:"project",taskId:"task",view}));
  },[harness,pixel]);
  await page.getByRole("button",{name:/Annotation list/}).click();
  await expect(page.getByText("本任务的正式处理结果 · 来源已绑定",{exact:true})).toBeVisible();
  await expect(page.getByText(/Batch|Workflow|child Run/)).toHaveCount(0);
  await expect(page.getByRole("button",{name:"新增漏标目标框",exact:true})).toBeDisabled();
  await expect(page.getByRole("button",{name:"接受这个对象",exact:true})).toBeDisabled();
  await expect(page.getByRole("button",{name:"拒绝这个对象",exact:true})).toBeDisabled();
  await expect(page.getByRole("button",{name:"确认整张图标注完整并继续",exact:true})).toBeDisabled();
});

test("a stale formal save keeps the edit and retries the same command identity",async({page})=>{
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async([path,imageUrl])=>{
    const {React,createRoot,P0ResultPanel}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    const annotation={id:"object",image_id:"image",task_id:"objects",label:"cup",value:{kind:"bounding_box",rect:[0.1,0.1,0.2,0.2]},attributes:{},source:"model",review_status:"needs_review",provenance:{},created_at:"TEST"};
    const commands:unknown[]=[];
    Object.assign(window,{p0StaleCommands:commands});
    const service={image:async()=>({intent_revision:1,intent_sha256:"intent",snapshot:{image_id:"image",source_run_id:"child",sha256:"snapshot",content_sha256:"pixels",annotations:[annotation]},sources:[],review:null,confirmation_current:false,accepted_objects:0,unresolved_objects:1,notice:"TEST"}),editObject:async(_project:string,_task:string,_image:string,input:unknown)=>{commands.push(structuredClone(input));throw new Error("stale snapshot: reload the current review item");},confirmImage:async()=>{}};
    const view={kind:"formal_review",images:[{id:"image",name:"one",src:imageUrl}],labels:[{stable_id:"cup",display_name:"杯子"},{stable_id:"bottle",display_name:"瓶子"}],formal_result:{project_id:"project",task_id:"task",processing_operation_id:"operation",batch_id:"batch",workflow_version:"workflow@1",status:"completed",images:[{image_id:"image",child_run_id:"child"}]},focus:{mode:"formal",image_id:"image",candidate_id:"object",result_revision:"snapshot",reason:"检查边界"},actions:[{id:"formal_edit_object",available:true,reason:null}]};
    createRoot(host).render(React.createElement(P0ResultPanel,{service,projectId:"project",taskId:"task",view}));
  },[harness,pixel]);
  await page.getByRole("button",{name:/Annotation list/}).click();
  await page.getByRole("button",{name:/杯子/}).click();
  await page.getByLabel("对象类别").selectOption("bottle");
  await page.getByRole("button",{name:"保存对象修改",exact:true}).click();
  await expect(page.getByRole("alert")).toContainText("stale snapshot");
  await expect(page.getByLabel("对象类别")).toHaveValue("bottle");
  await page.getByRole("button",{name:"保存对象修改",exact:true}).click();
  const commands=await page.evaluate(()=>(window as unknown as {p0StaleCommands:{command_id:string}[]}).p0StaleCommands);
  expect(commands).toHaveLength(2);
  expect(commands[1]).toEqual(commands[0]);
});

test("browser history cannot discard an unsaved formal edit",async({page})=>{
  await page.goto("/ui-preview?task=new&delivery_image=image-1");
  await page.evaluate(async([path,imageUrl])=>{
    const {React,createRoot,P0ResultPanel}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    const annotation=(image_id:string)=>({id:`object-${image_id}`,image_id,task_id:"objects",label:"cup",value:{kind:"bounding_box",rect:[0.1,0.1,0.2,0.2]},attributes:{},source:"model",review_status:"needs_review",provenance:{},created_at:"TEST"});
    const service={image:async(_p:string,_t:string,image_id:string,source_run_id:string)=>({intent_revision:1,intent_sha256:"intent",snapshot:{image_id,source_run_id,sha256:`snapshot-${image_id}`,content_sha256:`pixels-${image_id}`,annotations:[annotation(image_id)]},sources:[],review:null,confirmation_current:false,accepted_objects:0,unresolved_objects:1,notice:"TEST"}),editObject:async()=>{},confirmImage:async()=>{}};
    const view={kind:"formal_review",images:[{id:"image-1",name:"one",src:imageUrl},{id:"image-2",name:"two",src:imageUrl}],labels:[{stable_id:"cup",display_name:"杯子"},{stable_id:"bottle",display_name:"瓶子"}],formal_result:{project_id:"project",task_id:"task",processing_operation_id:"operation",batch_id:"batch",workflow_version:"workflow@1",status:"completed",images:[{image_id:"image-1",child_run_id:"run-1"},{image_id:"image-2",child_run_id:"run-2"}]},focus:null,actions:[{id:"formal_edit_object",available:true,reason:null}]};
    createRoot(host).render(React.createElement(P0ResultPanel,{service,projectId:"project",taskId:"task",view}));
  },[harness,pixel]);
  await page.getByLabel("图片",{exact:true}).selectOption("image-2");
  await page.getByRole("button",{name:/Annotation list/}).click();
  await page.getByRole("button",{name:/杯子/}).click();
  await page.getByLabel("对象类别").selectOption("bottle");
  await expect(page.getByText(/尚未保存/)).toBeVisible();
  page.once("dialog",dialog=>dialog.dismiss());
  await page.goBack();
  await expect(page.getByLabel("图片",{exact:true})).toHaveValue("image-2");
  await expect(page.getByLabel("对象类别")).toHaveValue("bottle");
  await expect(page).toHaveURL(/delivery_image=image-2/);
});
