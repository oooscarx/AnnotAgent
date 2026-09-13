import type { Annotation } from "../types";

export type DeliverySampleResult = {
  project_id:string;conversation_id:string;task_id:string;project_schema_revision:string;
  draft_id:string;draft_revision:number;sample_test_id:string;
  images:{
    image_id:string;image_sha256:string;result_revision:string;
    candidates:{candidate_id:string;selection:SampleVisualSelection|null}[];
    annotations:Annotation[];
    execution_evidence?:import("./refinementExecutionEvidence").ImageRefinementEvidence;
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
export type RejectedSampleProjection = {
  image_id:string;
  annotation_id:string;
  reason:"not_terminal"|"identity_mismatch"|"stale_reference";
};

const sampleAnnotationKind=(annotation:Annotation):SampleVisualSelection["annotation"]["kind"]=>{
  if(annotation.value.kind==="bounding_box"||annotation.value.kind==="classification"||annotation.value.kind==="semantic_mask"||annotation.value.kind==="instance_mask")return annotation.value.kind;
  throw new Error("当前候选类型不能作为 Mainline SampleCandidate 引用");
};
export function sampleVisualSelection(result:DeliverySampleResult,imageId:string,annotation:Annotation):SampleVisualSelection {
  const image=result.images.find(item=>item.image_id===imageId);if(!image)throw new Error("样例图片不属于当前任务结果");
  if(!image.annotations.some(item=>item.id===annotation.id))throw new Error("样例对象不属于当前图片");
  const candidate=image.candidates.find(item=>item.candidate_id===annotation.id);
  const selection=candidate?.selection;
  if(!selection)throw new Error("样例候选没有服务端签发的完整选择引用");
  const expectedKind=sampleAnnotationKind(annotation);
  const valid=
    selection.project_id===result.project_id&&selection.conversation_id===result.conversation_id&&
    selection.task_id===result.task_id&&selection.project_schema_revision===result.project_schema_revision&&
    selection.image.image_id===image.image_id&&selection.image.sha256===image.image_sha256&&
    selection.sample.draft_id===result.draft_id&&selection.sample.draft_revision===result.draft_revision&&
    selection.sample.sample_test_id===result.sample_test_id&&selection.candidate.candidate_id===annotation.id&&
    selection.candidate.source_artifact_id.length>0&&selection.annotation.kind===expectedKind&&
    (selection.annotation.label??null)===(annotation.label??null)&&selection.result_revision===image.result_revision;
  if(!valid)throw new Error("服务端选择引用与当前样例候选不一致");
  return structuredClone(selection);
}

/**
 * The review canvas may only consume annotations represented by the canonical
 * terminal-candidate read model. Legacy aggregate outcomes and stale selection
 * references stay out of the review layer instead of becoming convincing boxes.
 */
export function terminalSampleProjection(result:DeliverySampleResult):{
  result:DeliverySampleResult;
  rejected:RejectedSampleProjection[];
}{
  const imageIds=result.images.map(image=>image.image_id);
  const rejected:RejectedSampleProjection[]=[];
  const duplicateImages=new Set(imageIds.filter((id,index)=>imageIds.indexOf(id)!==index));
  const images=result.images.map(image=>{
    const candidateCounts=new Map<string,number>();
    const annotationCounts=new Map<string,number>();
    for(const candidate of image.candidates)candidateCounts.set(candidate.candidate_id,(candidateCounts.get(candidate.candidate_id)||0)+1);
    for(const annotation of image.annotations)annotationCounts.set(annotation.id,(annotationCounts.get(annotation.id)||0)+1);
    const annotations=image.annotations.filter(annotation=>{
      if(duplicateImages.has(image.image_id)||annotation.image_id!==image.image_id||annotationCounts.get(annotation.id)!==1){
        rejected.push({image_id:image.image_id,annotation_id:annotation.id,reason:"identity_mismatch"});return false;
      }
      if(candidateCounts.get(annotation.id)!==1){
        rejected.push({image_id:image.image_id,annotation_id:annotation.id,reason:"not_terminal"});return false;
      }
      const candidate=image.candidates.find(item=>item.candidate_id===annotation.id)!;
      if(candidate.selection){
        try{sampleVisualSelection(result,image.image_id,annotation);}
        catch{rejected.push({image_id:image.image_id,annotation_id:annotation.id,reason:"stale_reference"});return false;}
      }
      return true;
    });
    return {...image,candidates:image.candidates.filter(candidate=>annotations.some(annotation=>annotation.id===candidate.candidate_id)),annotations};
  });
  return {result:{...result,images},rejected};
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
