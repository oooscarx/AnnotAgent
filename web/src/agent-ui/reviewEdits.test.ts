import {expect,it} from "vitest";
import type {DetectionEvidenceDto} from "../types";
import {newHumanAnnotation} from "./humanGeometry";
import {applyReviewEvidence,parseReviewAttributes,validEvidenceBox} from "./reviewEdits";
it("rejects non-object attribute payloads without coercion",()=>{
  expect(parseReviewAttributes('{"flag":true,"count":2}')).toEqual({flag:true,count:2});
  for(const s of ["null","[]","false","1",'"text"',"{"])expect(()=>parseReviewAttributes(s)).toThrow();
});
it("keeps annotation identity/status/confidence and provenance when choosing source geometry",()=>{
  const a={...newHumanAnnotation("image",{id:"t",display_name:"T",kind:"bounding_box",labels:["target"],required:false},"a","now"),source:"model",confidence:0.4,provenance:{geometry_semantics:"coarse_hypothesis"}};
  const evidence={source_model_id:"m",source_artifact_id:"artifact",source_capability:"vision_language",bbox:[0.1,0.2,0.3,0.4],score:{}} as DetectionEvidenceDto;
  const chosen=applyReviewEvidence(a,evidence);expect(chosen.value).toEqual({kind:"bounding_box",rect:evidence.bbox});expect(chosen).toMatchObject({id:a.id,image_id:a.image_id,source:"model",confidence:0.4,review_status:"needs_review",provenance:{geometry_semantics:"coarse_hypothesis",selected_geometry_evidence:{source_artifact_id:"artifact"}}});expect(a.value).not.toEqual(chosen.value);
  for(const box of [[0,0,1,2],[-1,0,1,1],[0,0,NaN,1],[0,0,0,1],[0,0,1]])expect(validEvidenceBox(box)).toBe(false);
});
