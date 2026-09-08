import type {Annotation} from "./types";

/** A seed box/category is an editing affordance, not a submitted human answer. */
export function humanReferenceReady(annotation:Annotation|undefined,additionId:string,seed:Annotation|undefined):boolean {
  if(!annotation||!seed||annotation.provenance.addition_id!==additionId||!annotation.label?.trim())return false;
  if(annotation.value.kind==="bounding_box"&&seed.value.kind==="bounding_box"){
    const original=seed.value.rect;
    return annotation.value.rect.some((value,index)=>value!==original[index]);
  }
  if(annotation.value.kind==="classification"&&seed.value.kind==="classification")return JSON.stringify(annotation.value.labels)!==JSON.stringify(seed.value.labels);
  return false;
}
