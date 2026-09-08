import type { Annotation, SampleTestOutcomeRecord } from "./types";

/** A comparison must use the same final-only projection as the result canvas.
 * Legacy aggregated outcomes are not evidence of terminal annotations. */
export function terminalSampleAnnotations(sample:{
  outcomes:SampleTestOutcomeRecord[];
  projection?:{
    final_candidates:{outcome:SampleTestOutcomeRecord}[];
    review_candidates:{candidate:{outcome:SampleTestOutcomeRecord}}[];
  };
},imageId:string,testId:string):Annotation[]{
  if(!sample.projection)return [];
  const outcomes=[...sample.projection.final_candidates.map(candidate=>candidate.outcome),
    ...sample.projection.review_candidates.map(review=>review.candidate.outcome)];
  return sampleAnnotations(outcomes.filter((outcome,index)=>outcomes.findIndex(other=>other.id===outcome.id)===index),imageId,testId);
}

/** Saved outcomes only. Applying later feedback remains a separate editor concern. */
export function sampleAnnotations(outcomes:SampleTestOutcomeRecord[],imageId:string,testId:string):Annotation[]{
  return outcomes.flatMap(outcome=>outcome.value?[{
    id:outcome.id,image_id:imageId,task_id:"sample",
    label:outcome.value.kind==="classification"?outcome.value.labels.join(", "):outcome.label,
    value:outcome.value,attributes:{},confidence:outcome.confidence??undefined,
    source:"sample test",review_status:"needs_review" as const,provenance:{sample_test_id:testId},created_at:"",
  }]:[]);
}
