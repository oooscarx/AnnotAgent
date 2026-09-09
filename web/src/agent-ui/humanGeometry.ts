import type {Annotation,ProjectSummary} from "../types";
export const editableHumanKinds = new Set(["classification","bounding_box","keypoints","polyline","polygon","semantic_mask","instance_mask"]);
export function humanAnnotationValue(kind:string,label:string):Annotation["value"] {
  if(kind==="classification")return {kind,labels:[label]};
  if(kind==="bounding_box")return {kind,rect:[0.35,0.35,0.3,0.3]};
  if(kind==="keypoints")return {kind,points:[{name:"point",point:[0.5,0.5],visible:true}]};
  if(kind==="polyline")return {kind,points:[[0.35,0.5],[0.65,0.5]]};
  const rings:[number,number][][]=[[[0.35,0.35],[0.65,0.35],[0.5,0.65]]];
  if(kind==="polygon")return {kind,rings};
  if(kind==="semantic_mask"||kind==="instance_mask")return {kind,mask:{encoding:"polygon",rings}};
  throw new Error("该类型没有人工编辑器");
}
export function relabelHumanAnnotation(annotation:Annotation,label:string):Annotation {
  return {...annotation,label,value:annotation.value.kind==="classification"?{kind:"classification",labels:[label]}:annotation.value};
}
export function newHumanAnnotation(imageId:string,task:ProjectSummary["annotation_schema"][number],id:string,createdAt:string):Annotation {
  if(!editableHumanKinds.has(task.kind)||!task.labels.length)throw new Error("该标签组没有可编辑的几何类型或类别");
  const value=humanAnnotationValue(task.kind,task.labels[0]);
  return {id,image_id:imageId,task_id:task.id,label:task.labels[0],value,attributes:{},source:"human",review_status:"needs_review",provenance:{tool_names:[],artifact_ids:[]},created_at:createdAt};
}
