import type { Annotation } from "../types";

export type DeliverySampleResult = {
  project_id:string;task_id:string;draft_id:string;draft_revision:number;sample_test_id:string;
  feedback_revision:number;images:{image_id:string;snapshot_sha256:string;source_artifact_id:string|null;annotations:Annotation[]}[];
};
export type DeliveryFormalResult = {
  project_id:string;task_id:string;processing_operation_id:string;batch_id:string;workflow_version:string;
  status:string;images:{image_id:string;child_run_id:string|null}[];
};
export type VisualSelection = {
  project_id:string;task_id:string;image_id:string;annotation_id:string|null;label:string|null;
  annotation_kind:Annotation["value"]["kind"]|null;
  source:
    | {kind:"sample";draft_id:string;draft_revision:number;sample_test_id:string;source_artifact_id:string}
    | {kind:"formal";processing_operation_id:string;batch_id:string;child_run_id:string|null;workflow_version:string};
  revision:{snapshot_sha256:string;intent_revision:number|null;intent_sha256:string|null;review_revision:number|null;feedback_revision:number|null};
  save_target:"sample_feedback"|"formal_object"|"formal_image_review";
};

export function sampleVisualSelection(result:DeliverySampleResult,imageId:string,annotation?:Annotation):VisualSelection {
  const image=result.images.find(item=>item.image_id===imageId);if(!image)throw new Error("样例图片不属于当前任务结果");
  if(annotation&&!image.annotations.some(item=>item.id===annotation.id))throw new Error("样例对象不属于当前图片");
  if(!image.source_artifact_id)throw new Error("样例结果缺少来源 Artifact，不能创建可追溯对象引用");
  return {project_id:result.project_id,task_id:result.task_id,image_id:imageId,annotation_id:annotation?.id||null,label:annotation?.label||null,annotation_kind:annotation?.value.kind||null,
    source:{kind:"sample",draft_id:result.draft_id,draft_revision:result.draft_revision,sample_test_id:result.sample_test_id,source_artifact_id:image.source_artifact_id},
    revision:{snapshot_sha256:image.snapshot_sha256,intent_revision:null,intent_sha256:null,review_revision:null,feedback_revision:result.feedback_revision},save_target:"sample_feedback"};
}
export function formalVisualSelection(result:DeliveryFormalResult,imageId:string,snapshot:{sha256:string;intent_revision:number;intent_sha256:string;review_revision:number|null},annotation?:Annotation):VisualSelection {
  const image=result.images.find(item=>item.image_id===imageId);if(!image)throw new Error("此图片不属于本任务绑定的 Batch");if(annotation&&!image.child_run_id)throw new Error("没有 child Run 的图片不能引用正式对象");
  return {project_id:result.project_id,task_id:result.task_id,image_id:imageId,annotation_id:annotation?.id||null,label:annotation?.label||null,annotation_kind:annotation?.value.kind||null,
    source:{kind:"formal",processing_operation_id:result.processing_operation_id,batch_id:result.batch_id,child_run_id:image.child_run_id,workflow_version:result.workflow_version},
    revision:{snapshot_sha256:snapshot.sha256,intent_revision:snapshot.intent_revision,intent_sha256:snapshot.intent_sha256,review_revision:snapshot.review_revision,feedback_revision:null},save_target:annotation?"formal_object":"formal_image_review"};
}
