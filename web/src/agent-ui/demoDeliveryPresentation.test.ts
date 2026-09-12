import {expect,it} from "vitest";
import {
  assertDemoDeliveryPanelRead,
  assertDemoReviewPanelRead,
  demoOriginLabel,
  demoPackageEvidenceError,
  demoSourceModeLabel,
} from "./demoDeliveryPresentation";

it("labels preset, live and human sources without conflating them",()=>{
  expect(demoSourceModeLabel({id:"demo",version:"1.0.0",source_mode:"preset_candidates",live_inference_occurred:false})).toContain("本次没有调用模型");
  expect(demoSourceModeLabel({id:"demo",version:"1.0.0",source_mode:"live_model",live_inference_occurred:true})).toContain("已发生本次模型推理");
  expect(demoOriginLabel({kind:"preset_candidate",source_id:"preset",source_artifact_id:"artifact",model_display_name:null,actor_display_name:null,created_at:null})).toBe("预置候选 · 无本次模型推理");
  expect(demoOriginLabel({kind:"human_revision",source_id:"revision",source_artifact_id:null,model_display_name:null,actor_display_name:"Reviewer",created_at:"TEST"})).toBe("人工修订 · Reviewer");
});

it("requires complete Demo evidence before presenting a Ready package as downloadable",()=>{
  const expected={id:"demo",version:"1.0.0",source_mode:"preset_candidates" as const,live_inference_occurred:false};
  const result={sha256:"zip",bytes:42,images:2,objects:1,negatives:1,excluded:0,summary:{labels:["cup"],splits:{train:1,val:1},warnings:[],exclusions:{},demo:{id:"demo",version:"1.0.0",data_sha256:"data"},source_mode:"preset_candidates" as const,live_inference_occurred:false,review_sources:["reviewer"]}};
  expect(demoPackageEvidenceError(result,expected)).toBeNull();
  expect(demoPackageEvidenceError({...result,summary:{...result.summary,splits:{train:2,val:1}}},expected)).toContain("划分数量");
  expect(demoPackageEvidenceError({...result,summary:{...result.summary,demo:null}},expected)).toContain("示例版本");
});

it("rejects foreign review identity and duplicate delivery images",()=>{
  expect(()=>assertDemoReviewPanelRead({
    contract_version:"demo-review-v1",project_id:"foreign",task_id:"task",review_id:null,read_model_revision:"r1",
    demo:{id:"demo",version:"1.0.0",source_mode:"preset_candidates",live_inference_occurred:false},
    images:[],labels:[],sample_result:null,formal_result:null,
  },"project","task")).toThrow("不属于当前");
  expect(()=>assertDemoDeliveryPanelRead({
    contract_version:"demo-delivery-v1",project_id:"project",task_id:"task",delivery_id:null,read_model_revision:"r1",
    demo:{id:"demo",version:"1.0.0",source_mode:"preset_candidates",live_inference_occurred:false},
    scope:{revision:1,content_sha256:"scope",image_ids:["same","same"]},
  },"project","task")).toThrow("重复图片");
});
