import { AnnotationCanvas } from "../components/AnnotationCanvas";
import type { Annotation } from "../types";
import { DeliveryPackage } from "./DeliveryPackage";
import { terminalSampleProjection } from "./deliveryVisualSelection";
import {
  DeliveryReview,
  type DeliverySampleConfirmation,
  type DeliveryReviewFocus,
  type DeliveryReviewPermissions,
} from "./DeliveryReview";
import type { DeliveryService } from "./deliveryService";
import type {
  DeliveryFormalResult,
  DeliverySampleResult,
  SampleVisualSelection,
} from "./deliveryVisualSelection";
import type { FormalVisualSelection } from "./mainline";
import type { MainlineResultDiagnostic } from "./mainline";

type ResultImage={id:string;name:string;src?:string};
export type P0ResultAction={
  id:"sample_feedback"|"formal_edit_object"|"formal_create_object"|"formal_accept_object"|"formal_reject_object"|"formal_confirm_positive"|"formal_confirm_negative"|"formal_exclude_image";
  available:boolean;
  reason:string|null;
};
export type P0DiagnosticCategory="capability_missing"|"provider_not_received"|"outcome_unknown"|"invalid_structure"|"legal_empty"|"projection_failed"|"authorization_blocked";
export type P0ResultPanelView=
  |{kind:"preparing";stage:string;message:string;elapsed_ms:number|null}
  |{kind:"sample_feedback";images:ResultImage[];labels:{stable_id:string;display_name:string}[];sample_result:DeliverySampleResult;focus:DeliveryReviewFocus|null;actions:P0ResultAction[];diagnostics?:MainlineResultDiagnostic[]}
  |{kind:"formal_review";images:ResultImage[];labels:{stable_id:string;display_name:string}[];formal_result?:DeliveryFormalResult;source_mode?:"preset_candidates"|"live_model";focus:DeliveryReviewFocus|null;actions:P0ResultAction[]}
  |{kind:"diagnostic";category:P0DiagnosticCategory;message:string;image:ResultImage|null;annotations:Annotation[];focus_candidate_id:string|null}
  |{kind:"package";scope:{revision:number;content_sha256:string;image_ids:string[]};package_id:string|null};

export type P0ResultPanelProps={
  service:DeliveryService;
  projectId:string;
  taskId:string;
  view:P0ResultPanelView;
  onSelection?:(selection:SampleVisualSelection|FormalVisualSelection)=>void;
  onSampleIssue?:(selection:SampleVisualSelection)=>void;
  onSampleConfirm?:(selection:SampleVisualSelection,confirmation?:DeliverySampleConfirmation)=>Promise<void>;
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
  authorization_blocked:"当前授权不能继续",
};
const sampleDiagnosticCopy:Partial<Record<MainlineResultDiagnostic["code"],string>>={
  legal_empty_detection:"没有检测到候选；这不是人工确认的负样本。",
  candidate_projection_failed:"候选无法安全投影到原图；其他有效候选仍保留。",
};

function DiagnosticPanel({view}:{view:Extract<P0ResultPanelView,{kind:"diagnostic"}>}){
  return <section className="p0-result-diagnostic" aria-label="结果诊断">
    <h3>{diagnosticTitle[view.category]}</h3><p role="alert">{view.message}</p>
    {view.image&&<AnnotationCanvas imageUrl={view.image.src} annotations={view.annotations} selectedId={view.focus_candidate_id||undefined} onSelect={()=>{}} onChange={()=>{}} readOnly compactList/>}
    {view.category==="legal_empty"&&<p>这是模型结果，不是人工负样本确认。只有你检查原图后才能保存“没有目标”。</p>}
  </section>;
}

/** The single result surface. Rust owns all progression; this component only renders the current read model. */
export function P0ResultPanel({service,projectId,taskId,view,onSelection,onSampleIssue,onSampleConfirm,onPackageReady}:P0ResultPanelProps){
  if(view.kind==="preparing")return <section className="p0-result-status" aria-label="当前处理状态">
    <strong>{view.stage}</strong><p role="status">{view.message}</p>{view.elapsed_ms!==null&&<small>已用时 {Math.max(0,Math.round(view.elapsed_ms/1000))} 秒</small>}
  </section>;
  if(view.kind==="sample_feedback"){
    if(view.sample_result.project_id!==projectId||view.sample_result.task_id!==taskId){
      return <DiagnosticPanel view={{kind:"diagnostic",category:"invalid_structure",message:"样例结果不属于当前 Project/Task，未显示其中的图片或候选。",image:null,annotations:[],focus_candidate_id:null}}/>;
    }
    const projection=terminalSampleProjection(view.sample_result);
    const terminalCount=projection.result.images.reduce((sum,image)=>sum+image.annotations.length,0);
    const sourceCount=view.sample_result.images.reduce((sum,image)=>sum+image.annotations.length,0);
    if(sourceCount>0&&terminalCount===0){
      const first=view.sample_result.images.find(image=>image.annotations.length>0)!;
      return <DiagnosticPanel view={{kind:"diagnostic",category:"projection_failed",message:"服务端返回了候选，但没有对象属于当前任务的终端候选投影。中间粗框不会进入样例审核。",image:view.images.find(image=>image.id===first.image_id)||null,annotations:first.annotations,focus_candidate_id:first.annotations[0]?.id||null}}/>;
    }
    const diagnostics=(view.diagnostics||[]).filter(item=>item.source.kind==="sample_test"&&item.source.id===view.sample_result.sample_test_id&&sampleDiagnosticCopy[item.code]);
    return <>
      {projection.rejected.length>0&&<p className="p0-result-projection-warning" role="alert">已隐藏 {projection.rejected.length} 个不属于当前终端投影或引用已失效的候选；有效结果仍可审核。</p>}
      {diagnostics.length>0&&<div className="p0-result-projection-warning" aria-label="样例结果诊断">{diagnostics.map(item=>{
        const index=item.source.image_index;
        const name=typeof index==="number"?view.images[index]?.name:undefined;
        return <p key={`${item.code}:${item.source.id}:${index??"all"}`}><strong>{name?`${name}：`:""}</strong>{sampleDiagnosticCopy[item.code]} 服务器不会自动重试，已有结果不会被覆盖。</p>;
      })}</div>}
      <DeliveryReview
        key={`sample:${projection.result.sample_test_id}:${projection.result.draft_revision}`}
        service={service} project={projectId} task={taskId} images={view.images} labels={view.labels}
        sampleResult={projection.result} formalResult={null} preferredMode="sample" fixedMode="sample" focus={view.focus}
        permissions={permissions(view.actions)} guided onVisualSelection={onSelection} onSampleIssue={onSampleIssue} onSampleConfirm={onSampleConfirm}
      />
    </>;
  }
  if(view.kind==="formal_review")return <DeliveryReview
    key={view.formal_result?`formal:${view.formal_result.processing_operation_id}:${view.formal_result.batch_id}`:"formal:loading"}
    service={service} project={projectId} task={taskId} images={view.images} labels={view.labels}
    sampleResult={null} formalResult={view.formal_result} preferredMode="formal" fixedMode="formal" focus={view.focus}
    formalSourceMode={view.source_mode}
    permissions={permissions(view.actions)} guided onFormalSelection={onSelection}
  />;
  if(view.kind==="diagnostic")return <DiagnosticPanel view={view}/>;
  return <DeliveryPackage
    service={service} project={projectId} task={taskId} scope={view.scope}
    initialPackageId={view.package_id||undefined} onInspect={()=>{}}
    onReady={receipt=>onPackageReady?.(receipt.job.id)}
  />;
}
