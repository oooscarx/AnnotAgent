import type {Annotation,DetectionEvidenceDto} from "../types";
export function parseReviewAttributes(text:string):Record<string,unknown> {
  const value:unknown=JSON.parse(text);
  if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("属性必须是 JSON 对象，不能是数组或空值");
  return value as Record<string,unknown>;
}
export function validEvidenceBox(rect:number[]):boolean {
  if(rect.length!==4||!rect.every(Number.isFinite))return false;
  const [x,y,w,h]=rect;return x>=0&&y>=0&&w>0&&h>0&&x+w<=1&&y+h<=1;
}
export function applyReviewEvidence(annotation:Annotation,evidence:DetectionEvidenceDto):Annotation {
  if(annotation.value.kind!=="bounding_box"||!validEvidenceBox(evidence.bbox))throw new Error("来源证据不是可用的归一化框");
  return {...annotation,value:{kind:"bounding_box",rect:[...evidence.bbox]},attributes:{...annotation.attributes,selected_detection_evidence:evidence},provenance:{...annotation.provenance,selected_geometry_evidence:{source_model_id:evidence.source_model_id,source_artifact_id:evidence.source_artifact_id,source_capability:evidence.source_capability,score:evidence.score}}};
}
