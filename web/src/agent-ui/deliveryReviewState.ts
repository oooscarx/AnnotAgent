export type FormalRunEvidence = {
  image_id:string;
  child_run_id:string|null;
  run_status:string|null;
  status:string|null;
  error:string|null;
};

export type FormalReviewEvidence = {
  image_id:string;
  confirmation_current:boolean;
  review_decision:string|null;
  unresolved_objects:number;
  execution_status:string|null;
  execution_error:string|null;
};

export type FormalReviewResolution = {
  state:"positive_complete"|"negative_confirmed"|"excluded"|"unresolved"|"failed";
  blocks_package:boolean;
  diagnostic:string|null;
};

const failedRunStatuses=new Set(["failed","budget_exceeded","interrupted","cancelled"]);
const successfulRunStatuses=new Set(["completed","completed_with_review","partial"]);

/**
 * A non-empty execution error is diagnostic text, not a failure discriminator.
 * In particular, completed_with_review records why human review was requested.
 */
export function resolveFormalReview(
  review:FormalReviewEvidence,
  formal:FormalRunEvidence|undefined,
):FormalReviewResolution {
  const diagnostic=review.execution_error||formal?.error||null;
  if(review.confirmation_current){
    if(review.review_decision==="excluded")return {state:"excluded",blocks_package:false,diagnostic};
    if(review.unresolved_objects===0&&review.review_decision==="positive_complete")return {state:"positive_complete",blocks_package:false,diagnostic};
    if(review.unresolved_objects===0&&review.review_decision==="negative_confirmed")return {state:"negative_confirmed",blocks_package:false,diagnostic};
    return {state:"unresolved",blocks_package:true,diagnostic:"整图决定与当前对象状态不一致"};
  }
  if(formal?.run_status&&failedRunStatuses.has(formal.run_status))return {state:"failed",blocks_package:true,diagnostic};
  if(formal?.run_status&&successfulRunStatuses.has(formal.run_status))return {state:"unresolved",blocks_package:true,diagnostic};
  if(review.execution_status&&failedRunStatuses.has(review.execution_status))return {state:"failed",blocks_package:true,diagnostic};
  return {state:"unresolved",blocks_package:true,diagnostic};
}

export function resolveFormalReviewPage(
  reviews:FormalReviewEvidence[],
  formalImages:FormalRunEvidence[],
):Map<string,FormalReviewResolution> {
  const byImage=new Map<string,FormalRunEvidence>();
  for(const formal of formalImages){
    if(byImage.has(formal.image_id))throw new Error("正式结果包含重复图片，不能计算审核状态");
    byImage.set(formal.image_id,formal);
  }
  return new Map(reviews.map(review=>[review.image_id,resolveFormalReview(review,byImage.get(review.image_id))]));
}
