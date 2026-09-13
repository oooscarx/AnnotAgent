import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { RefinementEvidencePanel } from "./RefinementEvidencePanel";
import type { ImageRefinementEvidence } from "./refinementExecutionEvidence";

const evidence=(executed:boolean):ImageRefinementEvidence=>({configured_refiner:true,candidates:[{
  candidate_id:"ball",lineage_id:"detection:ball",executed,
  source:executed?"prompted_segmentation_refined":"vlm_only",
  reason:executed?"完整证据链。":"Coverage Gate：PartiallyCovered。",
  items:executed?[{kind:"node_receipt",label:"提示分割调用回执",node_id:"sam",status:"succeeded",artifact_id:null,artifact_ref:null,detail:null},{kind:"artifact",label:"Mask Artifact",node_id:"sam",status:null,artifact_id:"mask-id",artifact_ref:"mask:set",detail:null},{kind:"artifact",label:"mask_to_bbox Artifact",node_id:"bbox",status:null,artifact_id:"bbox-id",artifact_ref:"bbox:set",detail:null}]:[],
}]});

it("labels the terminal candidate as refined only with complete execution evidence",()=>{
  const complete=renderToStaticMarkup(<RefinementEvidencePanel evidence={evidence(true)} selected="ball"/>);
  expect(complete).toContain("当前终端候选来源：prompted segmentation refined");
  expect(complete).toContain("提示分割已执行");
  expect(complete).toContain("Mask Artifact");
  const stopped=renderToStaticMarkup(<RefinementEvidencePanel evidence={evidence(false)} selected="ball"/>);
  expect(stopped).toContain("当前终端候选来源：VLM-only");
  expect(stopped).toContain("方案包含但本图未执行提示分割");
  expect(stopped).not.toContain("提示分割已执行");
});
