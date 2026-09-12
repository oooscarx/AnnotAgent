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
  await expect(page.getByText("样例反馈",{exact:true})).toBeVisible();
  await expect(page.getByRole("tab",{name:"正式 Batch"})).toHaveCount(0);
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
  await expect(page.getByRole("button",{name:"新增漏标目标框",exact:true})).toBeDisabled();
  await expect(page.getByRole("button",{name:"接受这个对象",exact:true})).toBeDisabled();
  await expect(page.getByRole("button",{name:"拒绝这个对象",exact:true})).toBeDisabled();
  await expect(page.getByRole("button",{name:"确认整张图标注完整并继续",exact:true})).toBeDisabled();
});
