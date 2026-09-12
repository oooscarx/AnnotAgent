import {expect,test} from "@playwright/test";
import {resolve} from "node:path";

const pixel="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=";
const harness=`/@fs/${resolve("e2e/ui-preview/delivery-review-harness.tsx")}`;

test("six-image preset review exposes source and status without accepting on mount",async({page})=>{
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async([path,imageUrl])=>{
    const {React,createRoot,DemoReviewPanel}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    const annotation={id:"candidate-one",image_id:"image-1",task_id:"objects",label:"cup",value:{kind:"bounding_box",rect:[0.1,0.2,0.3,0.4]},attributes:{},source:"import",review_status:"needs_review",provenance:{},created_at:"TEST"};
    const images=Array.from({length:6},(_,index)=>({
      image_id:`image-${index+1}`,name:`Demo ${index+1}`,url:imageUrl,thumbnail_url:imageUrl,
      state:index===1?"negative_confirmed":index===2?"excluded":"review_required",source_artifact_id:`artifact-${index+1}`,
      annotation_origins:index===0?{"candidate-one":{kind:"preset_candidate",source_id:"preset-v1",source_artifact_id:"artifact-1",model_display_name:null,actor_display_name:null,created_at:"TEST"}}:{},
    }));
    const state={confirmWrites:0,artifact:"",ready:0};Object.assign(window,{demoReviewPanelTest:state});
    const formal={project_id:"project",task_id:"task",processing_operation_id:"operation",batch_id:"batch",workflow_version:"workflow@1",status:"completed",images:images.map(image=>({image_id:image.image_id,child_run_id:`run-${image.image_id}`}))};
    const service={
      demoReviewPanel:async()=>({contract_version:"demo-review-v1",project_id:"project",task_id:"task",review_id:"review",read_model_revision:"read-1",demo:{id:"object-detection-review",version:"1.0.0",source_mode:"preset_candidates",live_inference_occurred:false},images,labels:[{stable_id:"cup",display_name:"杯子"},{stable_id:"bottle",display_name:"瓶子"}],sample_result:null,formal_result:formal}),
      image:async(_p:string,_t:string,id:string,run:string|null)=>({intent_revision:1,intent_sha256:"intent",snapshot:{image_id:id,source_run_id:run,sha256:`snapshot-${id}`,content_sha256:`pixels-${id}`,annotations:id==="image-1"?[annotation]:[]},sources:[],review:null,confirmation_current:false,accepted_objects:0,unresolved_objects:id==="image-1"?1:0,notice:"TEST"}),
      confirmImage:async()=>{state.confirmWrites+=1;throw new Error("must not confirm on start");},
      editObject:async()=>{},history:async()=>({items:[],next_cursor:null}),packageStatus:async()=>{throw new Error("unused");},cancelPackage:async()=>{},downloadUrl:()=>"/unused",pendingPackage:()=>undefined,startPackage:async()=>{throw new Error("unused");},
    };
    createRoot(host).render(React.createElement(DemoReviewPanel,{service,projectId:"project",taskId:"task",reviewId:"review",onOpenArtifact:(id:string)=>{state.artifact=id;},onReady:()=>{state.ready+=1;}}));
  },[harness,pixel]);

  await expect(page.getByRole("heading",{name:"6 张示例图片"})).toBeVisible();
  await expect(page.getByText("预置候选模式 · 本次没有调用模型",{exact:true})).toBeVisible();
  await expect(page.getByText("预置候选 · 无本次模型推理",{exact:true}).first()).toBeVisible();
  await expect(page.getByText("待审核",{exact:true}).first()).toBeVisible();
  await expect(page.getByText("已确认负样本",{exact:true})).toBeVisible();
  await expect(page.getByText("已排除",{exact:true})).toBeVisible();
  await page.getByRole("button",{name:"查看来源 Artifact",exact:true}).first().click();
  expect(await page.evaluate(()=>(window as unknown as {demoReviewPanelTest:{confirmWrites:number;artifact:string;ready:number}}).demoReviewPanelTest)).toEqual({confirmWrites:0,artifact:"artifact-1",ready:1});
});

test("ready delivery renders persisted Rust package evidence and a controlled download",async({page})=>{
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async(path)=>{
    const {React,createRoot,DemoDeliveryPanel}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    const state={starts:0,downloads:[] as string[],ready:0};Object.assign(window,{demoDeliveryPanelTest:state});
    const receipt={job:{id:"package-new",phase:"ready",intent_revision:4,snapshot_sha256:"review-snapshot",result:{sha256:"zip-sha-NEW",bytes:54321,images:6,objects:7,negatives:1,excluded:1,summary:{labels:["cup","bottle"],splits:{train:4,val:2},warnings:[],exclusions:{"image-3":"blurred"},demo:{id:"object-detection-review",version:"1.0.0",data_sha256:"demo-data-sha"},source_mode:"preset_candidates",live_inference_occurred:false,source_counts:{preset_candidate:6,human_revision:1},review_sources:["current-user"]}},error:null},active:false,interrupted:false};
    const service={
      demoDeliveryPanel:async()=>({contract_version:"demo-delivery-v1",project_id:"project",task_id:"task",delivery_id:"package-new",read_model_revision:"read-1",scope:{revision:4,content_sha256:"intent",image_ids:["1","2","3","4","5","6"]},demo:{id:"object-detection-review",version:"1.0.0",source_mode:"preset_candidates",live_inference_occurred:false}}),
      history:async()=>({items:[{id:"package-new",created_at:"TEST now"}],next_cursor:null}),pendingPackage:()=>undefined,
      packageReadiness:async()=>({intent_revision:4,intent_sha256:"intent",ready:true,counts:{total:6,complete:6,positive:4,negative:1,excluded:1,unresolved:0,failed:0},review_revisions:{},blockers:[],consent:{input:{id:"consent",intent_revision:4,intent_sha256:"intent",confirmed:true},state:"consumed"},package:receipt}),
      packageStatus:async()=>receipt,startPackage:async()=>{state.starts+=1;throw new Error("must not start in React");},cancelPackage:async()=>{throw new Error("unused");},downloadUrl:()=>"/api/owned/package-new/download",
    };
    createRoot(host).render(React.createElement(DemoDeliveryPanel,{service,projectId:"project",taskId:"task",deliveryId:"package-new",onReady:()=>{state.ready+=1;},onDownload:(id:string)=>state.downloads.push(id)}));
  },harness);

  await expect(page.getByText("当前范围 6 张",{exact:false})).toBeVisible();
  await expect(page.getByText("类别：cup、bottle",{exact:true})).toBeVisible();
  await expect(page.getByText("训练图片 4 · 验证图片 2",{exact:false})).toBeVisible();
  await expect(page.getByText("示例 object-detection-review · 1.0.0 · 数据 demo-data-sha",{exact:true})).toBeVisible();
  await expect(page.getByText("来源：预置候选 · 本次无模型请求",{exact:true})).toBeVisible();
  await expect(page.getByText("预置候选 6 · 模型预测 0 · 人工修订 1",{exact:true})).toBeVisible();
  await page.getByText("查看真实检查报告",{exact:true}).click();
  await expect(page.getByText("zip-sha-NEW",{exact:false})).toBeVisible();
  const download=page.getByRole("link",{name:"下载数据集 ZIP"});
  await expect(download).toHaveAttribute("href","/api/owned/package-new/download");
  await download.evaluate(element=>element.addEventListener("click",event=>event.preventDefault(),{once:true}));
  await download.click();
  expect(await page.evaluate(()=>(window as unknown as {demoDeliveryPanelTest:{starts:number;downloads:string[];ready:number}}).demoDeliveryPanelTest)).toEqual({starts:0,downloads:["package-new"],ready:1});
});

test("formal box and label edits save a changed snapshot without confirming the image",async({page})=>{
  const imageUrl="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='100' height='100'%3E%3Crect width='100' height='100' fill='white'/%3E%3C/svg%3E";
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async([path,image])=>{
    const {React,createRoot,DeliveryReview}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    let annotation={id:"object",image_id:"image",task_id:"objects",label:"cup",value:{kind:"bounding_box",rect:[0.1,0.1,0.2,0.2]},attributes:{},source:"import",review_status:"needs_review",provenance:{},created_at:"TEST"};
    const state={edits:[] as unknown[],creates:[] as unknown[],confirms:0};Object.assign(window,{demoEditTest:state});
    const service={
      image:async()=>({intent_revision:3,intent_sha256:"intent",snapshot:{image_id:"image",source_run_id:"child-run",sha256:`snapshot-${state.edits.length}-${state.creates.length}`,content_sha256:"pixels",annotations:[annotation]},sources:[],review:null,confirmation_current:false,accepted_objects:0,unresolved_objects:1,notice:"TEST"}),
      editObject:async(_p:string,_t:string,_i:string,input:{label:string;value:typeof annotation.value})=>{state.edits.push(structuredClone(input));annotation={...annotation,label:input.label,value:input.value};},
      createObject:async(_p:string,_t:string,_i:string,input:unknown)=>{state.creates.push(structuredClone(input));},
      confirmImage:async()=>{state.confirms+=1;throw new Error("must not confirm with object save");},
    };
    createRoot(host).render(React.createElement(DeliveryReview,{service,project:"project",task:"task",labels:[{stable_id:"cup",display_name:"杯子"},{stable_id:"bottle",display_name:"瓶子"}],images:[{id:"image",name:"Demo",src:image}],formalResult:{project_id:"project",task_id:"task",processing_operation_id:"operation",batch_id:"batch",workflow_version:"workflow@1",status:"completed",images:[{image_id:"image",child_run_id:"child-run"}]}}));
  },[harness,imageUrl]);

  await page.getByRole("button",{name:/Annotation list/}).click();
  await page.getByRole("button",{name:/杯子/}).click();
  await expect(page.locator(".canvas-keyboard-tools button").first()).toBeVisible();
  await page.locator(".canvas-keyboard-tools button").first().press("ArrowRight");
  await page.getByLabel("对象类别").selectOption("bottle");
  await page.getByRole("button",{name:"保存对象修改",exact:true}).click();
  await expect(page.getByText("对象修改已保存；这不等于整张图已经检查完整。",{exact:true})).toBeVisible();
  const state=await page.evaluate(()=>(window as unknown as {demoEditTest:{edits:{label:string;value:{rect:number[]}}[];creates:unknown[];confirms:number}}).demoEditTest);
  expect(state.edits).toHaveLength(1);
  expect(state.edits[0].label).toBe("bottle");
  expect(state.edits[0].value.rect).not.toEqual([0.1,0.1,0.2,0.2]);
  expect(state.confirms).toBe(0);

  await page.getByRole("button",{name:"新增漏标目标框",exact:true}).click();
  await page.getByRole("button",{name:"保存新增目标框",exact:true}).click();
  expect((await page.evaluate(()=>(window as unknown as {demoEditTest:{creates:unknown[]}}).demoEditTest.creates))).toHaveLength(1);
});

test("a Ready phase without frozen Demo evidence is not downloadable",async({page})=>{
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async path=>{
    const {React,createRoot,DemoDeliveryPanel}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    const state={ready:0};Object.assign(window,{invalidDemoPackageTest:state});
    const receipt={job:{id:"invalid-ready",phase:"ready",intent_revision:1,snapshot_sha256:"snapshot",result:{sha256:"zip",bytes:42,images:6,objects:1,negatives:0,excluded:0,summary:{labels:["cup"],splits:{train:4,val:2},warnings:[],exclusions:{}}},error:null},active:false,interrupted:false};
    const service={demoDeliveryPanel:async()=>({contract_version:"demo-delivery-v1",project_id:"project",task_id:"task",delivery_id:"invalid-ready",read_model_revision:"read",scope:{revision:1,content_sha256:"intent",image_ids:["1","2","3","4","5","6"]},demo:{id:"demo",version:"1.0.0",source_mode:"preset_candidates",live_inference_occurred:false}}),history:async()=>({items:[],next_cursor:null}),pendingPackage:()=>undefined,packageReadiness:async()=>({intent_revision:1,intent_sha256:"intent",ready:true,counts:{total:6,complete:6,positive:6,negative:0,excluded:0,unresolved:0,failed:0},review_revisions:{},blockers:[],consent:null,package:receipt}),packageStatus:async()=>receipt,startPackage:async()=>{throw new Error("unused");},cancelPackage:async()=>{},downloadUrl:()=>"/must-not-download"};
    createRoot(host).render(React.createElement(DemoDeliveryPanel,{service,projectId:"project",taskId:"task",deliveryId:"invalid-ready",onReady:()=>{state.ready+=1;}}));
  },harness);
  await expect(page.getByRole("alert")).toContainText("缺少当前示例版本与数据 hash");
  await expect(page.getByRole("link",{name:"下载数据集 ZIP"})).toHaveCount(0);
  expect(await page.evaluate(()=>(window as unknown as {invalidDemoPackageTest:{ready:number}}).invalidDemoPackageTest.ready)).toBe(0);
});

test("an active package can be cancelled and refresh never creates a replacement",async({page})=>{
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async path=>{
    const {React,createRoot,DemoDeliveryPanel}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    const state={phase:"exporting",cancels:0,starts:0};Object.assign(window,{demoCancelPackageTest:state});
    const read=()=>({job:{id:"active-package",phase:state.phase,intent_revision:1,snapshot_sha256:"snapshot",result:null,error:null},active:state.phase==="exporting",interrupted:false});
    const service={demoDeliveryPanel:async()=>({contract_version:"demo-delivery-v1",project_id:"project",task_id:"task",delivery_id:"active-package",read_model_revision:"read",scope:{revision:1,content_sha256:"intent",image_ids:["1","2","3","4","5","6"]},demo:{id:"demo",version:"1.0.0",source_mode:"preset_candidates",live_inference_occurred:false}}),history:async()=>({items:[{id:"active-package",created_at:"TEST"}],next_cursor:null}),pendingPackage:()=>undefined,packageReadiness:async()=>({intent_revision:1,intent_sha256:"intent",ready:false,counts:{total:6,complete:6,positive:6,negative:0,excluded:0,unresolved:0,failed:0},review_revisions:{},blockers:[],consent:{input:{id:"consent",intent_revision:1,intent_sha256:"intent",confirmed:true},state:"consumed"},package:read()}),packageStatus:async()=>read(),startPackage:async()=>{state.starts+=1;throw new Error("must not start");},cancelPackage:async()=>{state.cancels+=1;state.phase="cancelled";return read().job;},downloadUrl:()=>"/unused"};
    createRoot(host).render(React.createElement(DemoDeliveryPanel,{service,projectId:"project",taskId:"task",deliveryId:"active-package"}));
  },harness);
  await expect(page.getByText("正在打包原图和标签",{exact:true})).toBeVisible();
  await page.getByRole("button",{name:"取消这个打包任务",exact:true}).click();
  await expect(page.getByText("已取消",{exact:true})).toBeVisible();
  await page.getByRole("button",{name:"刷新审核与打包状态",exact:true}).click();
  expect(await page.evaluate(()=>(window as unknown as {demoCancelPackageTest:{phase:string;cancels:number;starts:number}}).demoCancelPackageTest)).toEqual({phase:"cancelled",cancels:1,starts:0});
  await expect(page.getByRole("link",{name:"下载数据集 ZIP"})).toHaveCount(0);
});
