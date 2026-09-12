import { AnnotationCanvas } from "../components/AnnotationCanvas";
import type { Annotation } from "../types";
import { DeliveryPackage } from "./DeliveryPackage";
import {
  DeliveryReview,
  type DeliveryReviewFocus,
  type DeliveryReviewPermissions,
} from "./DeliveryReview";
import type { DeliveryService } from "./deliveryService";
import type {
  DeliveryFormalResult,
  DeliverySampleResult,
  FormalReviewSelection,
  SampleVisualSelection,
} from "./deliveryVisualSelection";

type ResultImage={id:string;name:string;src?:string};
export type P0ResultAction={
  id:"sample_feedback"|"formal_edit_object"|"formal_create_object"|"formal_accept_object"|"formal_reject_object"|"formal_confirm_positive"|"formal_confirm_negative"|"formal_exclude_image";
  available:boolean;
  reason:string|null;
};
export type P0DiagnosticCategory="capability_missing"|"provider_not_received"|"outcome_unknown"|"invalid_structure"|"legal_empty"|"projection_failed";
export type P0ResultPanelView=
  |{kind:"preparing";stage:string;message:string;elapsed_ms:number|null}
  |{kind:"sample_feedback";images:ResultImage[];labels:{stable_id:string;display_name:string}[];sample_result:DeliverySampleResult;focus:DeliveryReviewFocus|null;actions:P0ResultAction[]}
  |{kind:"formal_review";images:ResultImage[];labels:{stable_id:string;display_name:string}[];formal_result:DeliveryFormalResult;focus:DeliveryReviewFocus|null;actions:P0ResultAction[]}
  |{kind:"diagnostic";category:P0DiagnosticCategory;message:string;image:ResultImage|null;annotations:Annotation[];focus_candidate_id:string|null}
  |{kind:"package";scope:{revision:number;content_sha256:string;image_ids:string[]};package_id:string|null};

export type P0ResultPanelProps={
  service:DeliveryService;
  projectId:string;
  taskId:string;
  view:P0ResultPanelView;
  onSelection?:(selection:SampleVisualSelection|FormalReviewSelection)=>void;
  onSampleIssue?:(selection:SampleVisualSelection)=>void;
  onPackageReady?:(packageId:string)=>void;
};

const available=(actions:P0ResultAction[],id:P0ResultAction["id"]):boolean=>actions.some(action=>action.id===id&&action.available);
const permissions=(actions:P0ResultAction[]):DeliveryReviewPermissions=>({
  sampleFeedback:available(actions,"sample_feedback"),
  editObject:available(actions,"formal_edit_object"),
  createObject:available(actions,"formal_create_object"),
  acceptObject:available(actions,"formal_accept_object"),
  rejectObject:available(actions,"formal_reject_object"),
  confirmPositive:available(actions,"formal_confirm_positive"),
  confirmNegative:available(actions,"formal_confirm_negative"),
  excludeImage:available(actions,"formal_exclude_image"),
});
const diagnosticTitle:Record<P0DiagnosticCategory,string>={
  capability_missing:"缺少可用的图像定位能力",
  provider_not_received:"模型服务未接收本次请求",
  outcome_unknown:"远端结果未知",
  invalid_structure:"模型返回结构无法使用",
  legal_empty:"本次没有检测到候选",
  projection_failed:"候选无法投影到原图",
};

/** The single result surface. Rust owns all progression; this component only renders the current read model. */
export function P0ResultPanel({service,projectId,taskId,view,onSelection,onSampleIssue,onPackageReady}:P0ResultPanelProps){
  if(view.kind==="preparing")return <section className="p0-result-status" aria-label="当前处理状态">
    <strong>{view.stage}</strong><p role="status">{view.message}</p>{view.elapsed_ms!==null&&<small>已用时 {Math.max(0,Math.round(view.elapsed_ms/1000))} 秒</small>}
  </section>;
  if(view.kind==="sample_feedback")return <DeliveryReview
    key={`sample:${view.sample_result.sample_test_id}:${view.sample_result.draft_revision}`}
    service={service} project={projectId} task={taskId} images={view.images} labels={view.labels}
    sampleResult={view.sample_result} formalResult={null} preferredMode="sample" fixedMode="sample" focus={view.focus}
    permissions={permissions(view.actions)} guided onVisualSelection={onSelection} onSampleIssue={onSampleIssue}
  />;
  if(view.kind==="formal_review")return <DeliveryReview
    key={`formal:${view.formal_result.processing_operation_id}:${view.formal_result.batch_id}`}
    service={service} project={projectId} task={taskId} images={view.images} labels={view.labels}
    sampleResult={null} formalResult={view.formal_result} preferredMode="formal" fixedMode="formal" focus={view.focus}
    permissions={permissions(view.actions)} guided onFormalSelection={onSelection}
  />;
  if(view.kind==="diagnostic")return <section className="p0-result-diagnostic" aria-label="结果诊断">
    <h3>{diagnosticTitle[view.category]}</h3><p role="alert">{view.message}</p>
    {view.image&&<AnnotationCanvas imageUrl={view.image.src} annotations={view.annotations} selectedId={view.focus_candidate_id||undefined} onSelect={()=>{}} onChange={()=>{}} readOnly compactList/>}
    {view.category==="legal_empty"&&<p>这是模型结果，不是人工负样本确认。只有你检查原图后才能保存“没有目标”。</p>}
  </section>;
  return <DeliveryPackage
    service={service} project={projectId} task={taskId} scope={view.scope}
    initialPackageId={view.package_id||undefined} onInspect={()=>{}}
    onReady={receipt=>onPackageReady?.(receipt.job.id)}
  />;
}
