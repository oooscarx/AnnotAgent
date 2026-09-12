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
