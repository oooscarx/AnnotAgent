import type { Annotation } from "../types";

export type DeliverySampleResult = {
  project_id:string;conversation_id:string;task_id:string;project_schema_revision:string;
  draft_id:string;draft_revision:number;sample_test_id:string;
  images:{
    image_id:string;image_sha256:string;result_revision:string;
    source_artifacts:Record<string,string>;annotations:Annotation[];
  }[];
};
export type DeliveryFormalResult = {
  project_id:string;task_id:string;processing_operation_id:string;batch_id:string;workflow_version:string;
  status:string;images:{image_id:string;child_run_id:string|null}[];
};
export type FormalReviewWorkItem = {
  project_id:string;task_id:string;processing_operation_id:string;batch_id:string;workflow_version:string;
  image_id:string;child_run_id:string|null;
  state:"positive_complete"|"negative_confirmed"|"excluded"|"unresolved"|"failed";
  snapshot_sha256:string;intent_revision:number;intent_sha256:string;review_revision:number|null;
  annotations:Annotation[];error:string|null;
};
/** Structurally identical to the public mainline VisualSelection from G0. */
export type SampleVisualSelection = {
  project_id:string;conversation_id:string;task_id:string;project_schema_revision:string;
  image:{image_id:string;sha256:string};
  sample:{draft_id:string;draft_revision:number;sample_test_id:string};
  candidate:{candidate_id:string;source_artifact_id:string};
  annotation:{kind:"bounding_box"|"classification"|"semantic_mask"|"instance_mask";label?:string};
  result_revision:string;
};
export type FormalReviewSelection = {
  project_id:string;task_id:string;image_id:string;annotation_id:string|null;label:string|null;
  annotation_kind:Annotation["value"]["kind"]|null;
  source:{kind:"formal";processing_operation_id:string;batch_id:string;child_run_id:string|null;workflow_version:string};
  revision:{snapshot_sha256:string;intent_revision:number;intent_sha256:string;review_revision:number|null};
  save_target:"formal_object"|"formal_image_review";
};
export type VisualSelection = SampleVisualSelection|FormalReviewSelection;

const sampleAnnotationKind=(annotation:Annotation):SampleVisualSelection["annotation"]["kind"]=>{
  if(annotation.value.kind==="bounding_box"||annotation.value.kind==="classification"||annotation.value.kind==="semantic_mask"||annotation.value.kind==="instance_mask")return annotation.value.kind;
  throw new Error("当前候选类型不能作为 Mainline SampleCandidate 引用");
};
export function sampleVisualSelection(result:DeliverySampleResult,imageId:string,annotation:Annotation):SampleVisualSelection {
  const image=result.images.find(item=>item.image_id===imageId);if(!image)throw new Error("样例图片不属于当前任务结果");
  if(!image.annotations.some(item=>item.id===annotation.id))throw new Error("样例对象不属于当前图片");
  const sourceArtifactId=image.source_artifacts[annotation.id];if(!sourceArtifactId)throw new Error("样例候选缺少自己的来源 Artifact，不能创建可追溯对象引用");
  return {
    project_id:result.project_id,conversation_id:result.conversation_id,task_id:result.task_id,
    project_schema_revision:result.project_schema_revision,
    image:{image_id:imageId,sha256:image.image_sha256},
    sample:{draft_id:result.draft_id,draft_revision:result.draft_revision,sample_test_id:result.sample_test_id},
    candidate:{candidate_id:annotation.id,source_artifact_id:sourceArtifactId},
    annotation:{kind:sampleAnnotationKind(annotation),...(annotation.label?{label:annotation.label}:{})},
    result_revision:image.result_revision,
  };
}
export function formalVisualSelection(result:DeliveryFormalResult,imageId:string,snapshot:{sha256:string;intent_revision:number;intent_sha256:string;review_revision:number|null},annotation?:Annotation):FormalReviewSelection {
  const image=result.images.find(item=>item.image_id===imageId);if(!image)throw new Error("此图片不属于本任务绑定的 Batch");if(annotation&&!image.child_run_id)throw new Error("没有 child Run 的图片不能引用正式对象");
  return {project_id:result.project_id,task_id:result.task_id,image_id:imageId,annotation_id:annotation?.id||null,label:annotation?.label||null,annotation_kind:annotation?.value.kind||null,
    source:{kind:"formal",processing_operation_id:result.processing_operation_id,batch_id:result.batch_id,child_run_id:image.child_run_id,workflow_version:result.workflow_version},
    revision:{snapshot_sha256:snapshot.sha256,intent_revision:snapshot.intent_revision,intent_sha256:snapshot.intent_sha256,review_revision:snapshot.review_revision},save_target:annotation?"formal_object":"formal_image_review"};
}
export function formalWorkItemVisualSelection(item:FormalReviewWorkItem,annotation?:Annotation):FormalReviewSelection {
  if(annotation&&!item.annotations.some(saved=>saved.id===annotation.id))throw new Error("正式对象不属于当前 Review Work Item");
  return formalVisualSelection({
    project_id:item.project_id,task_id:item.task_id,processing_operation_id:item.processing_operation_id,
    batch_id:item.batch_id,workflow_version:item.workflow_version,status:item.state,
    images:[{image_id:item.image_id,child_run_id:item.child_run_id}],
  },item.image_id,{
    sha256:item.snapshot_sha256,intent_revision:item.intent_revision,
    intent_sha256:item.intent_sha256,review_revision:item.review_revision,
  },annotation);
}
