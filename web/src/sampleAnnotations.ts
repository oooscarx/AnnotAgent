import type { Annotation, SampleTestOutcomeRecord } from "./types";

/** Saved outcomes only. Applying later feedback remains a separate editor concern. */
export function sampleAnnotations(outcomes:SampleTestOutcomeRecord[],imageId:string,testId:string):Annotation[]{
  return outcomes.flatMap(outcome=>outcome.value?[{
    id:outcome.id,image_id:imageId,task_id:"sample",
    label:outcome.value.kind==="classification"?outcome.value.labels.join(", "):outcome.label,
    value:outcome.value,attributes:{},confidence:outcome.confidence??undefined,
    source:"sample test",review_status:"needs_review" as const,provenance:{sample_test_id:testId},created_at:"",
  }]:[]);
}
