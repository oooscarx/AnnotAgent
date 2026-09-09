import type {Annotation,ProjectSummary} from "../types";
export const editableHumanKinds = new Set(["bounding_box","keypoints","polyline","polygon"]);
export function newHumanAnnotation(imageId:string,task:ProjectSummary["annotation_schema"][number],id:string,createdAt:string):Annotation {
  if(!editableHumanKinds.has(task.kind)||!task.labels.length)throw new Error("该标签组没有可编辑的几何类型或类别");
  const value:Annotation["value"] = task.kind==="bounding_box" ? {kind:"bounding_box",rect:[0.35,0.35,0.3,0.3]} : task.kind==="keypoints" ? {kind:"keypoints",points:[{name:"point",point:[0.5,0.5],visible:true}]} : task.kind==="polyline" ? {kind:"polyline",points:[[0.35,0.5],[0.65,0.5]]} : {kind:"polygon",rings:[[[0.35,0.35],[0.65,0.35],[0.5,0.65]]]};
  return {id,image_id:imageId,task_id:task.id,label:task.labels[0],value,attributes:{},source:"human",review_status:"needs_review",provenance:{tool_names:[],artifact_ids:[]},created_at:createdAt};
}
