import type {
  DemoAnnotationOrigin,
  DemoDeliveryPanelRead,
  DemoImageReviewState,
  DemoReviewPanelRead,
} from "./deliveryService";

export const demoReviewStateLabel = (state:DemoImageReviewState):string => ({
  pending:"待处理",
  review_required:"待审核",
  positive_complete:"整图完整",
  negative_confirmed:"已确认负样本",
  excluded:"已排除",
  failed:"处理失败",
})[state];

export const demoOriginLabel = (origin:DemoAnnotationOrigin):string => {
  if(origin.kind==="preset_candidate")return "预置候选 · 无本次模型推理";
  if(origin.kind==="live_model_prediction")return origin.model_display_name
    ? `本次模型预测 · ${origin.model_display_name}`
    : "本次模型预测";
  return origin.actor_display_name ? `人工修订 · ${origin.actor_display_name}` : "人工修订";
};

export const demoSourceModeLabel = (demo:DemoReviewPanelRead["demo"]|DemoDeliveryPanelRead["demo"]):string => demo.source_mode==="preset_candidates"
  ? "预置候选模式 · 本次没有调用模型"
  : demo.live_inference_occurred
    ? "真实模型模式 · 已发生本次模型推理"
    : "真实模型模式 · 尚无本次模型推理回执";

export function assertDemoReviewPanelRead(view:DemoReviewPanelRead,project:string,task:string,reviewId?:string):DemoReviewPanelRead {
  if(view.contract_version!=="demo-review-v1"||view.project_id!==project||view.task_id!==task||!view.read_model_revision)throw new Error("Demo 审核结果不属于当前 Project/Task");
  if(reviewId&&view.review_id!==reviewId)throw new Error("Demo 审核结果与当前 Review 不匹配");
  const ids=new Set<string>();
  for(const image of view.images){
    if(!image.image_id||!image.url||ids.has(image.image_id))throw new Error("Demo 图片清单缺少稳定身份或包含重复图片");
    ids.add(image.image_id);
  }
  return view;
}

export function assertDemoDeliveryPanelRead(view:DemoDeliveryPanelRead,project:string,task:string,deliveryId?:string):DemoDeliveryPanelRead {
  if(view.contract_version!=="demo-delivery-v1"||view.project_id!==project||view.task_id!==task||!view.read_model_revision)throw new Error("Demo 交付范围不属于当前 Project/Task");
  if(deliveryId&&view.delivery_id!==deliveryId)throw new Error("Demo 交付范围与当前数据包不匹配");
  if(!Number.isSafeInteger(view.scope.revision)||view.scope.revision<1||!view.scope.content_sha256||!view.scope.image_ids.length)throw new Error("Demo 交付范围不完整");
  if(new Set(view.scope.image_ids).size!==view.scope.image_ids.length)throw new Error("Demo 交付范围包含重复图片");
  return view;
}
