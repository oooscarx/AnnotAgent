import { describe, expect, it } from "vitest";
import type { WorkflowDryRunReport } from "../types";
import { imageRefinementEvidence } from "./refinementExecutionEvidence";

type Sample = WorkflowDryRunReport["samples"][number];

const sample = (overrides: Partial<Sample> = {}): Sample => ({
  image_index:0,image_name:"ball.png",width:1280,height:720,result_count:1,
  auto_accepted_count:0,review_count:1,failed:false,empty:false,outcomes:[],failure_classes:[],
  nodes:[
    {node_id:"sam",status:"succeeded",output_types:["mask_set"],latency_ms:12,estimated_cost:"0",issues:[],metadata:{}},
    {node_id:"bbox",status:"succeeded",output_types:["detection_set"],latency_ms:1,estimated_cost:"0",issues:[],metadata:{}},
  ],
  projection:{
    final_candidates:[],review_candidates:[{candidate:{source_artifact_id:"terminal-id",source_artifact_ref:"bbox:set",lineage_id:"detection:ball",outcome:{id:"ball",label:"ball",status:"needs_review",value:{kind:"bounding_box",rect:[0.4,0.4,0.1,0.1]}},localization:"whole",geometry:"refined",final_status:"review"},explanation:{title:"review",summary:"review"}}],
    committed_annotations:[],no_target:false,intermediate_artifact_ids:[],
    debug_stages:[
      {artifact_id:"mask-id",artifact_ref:"mask:set",node_id:"sam",lineage_id:"detection:box-prompt:ball",stage:"mask",source:"Prompted segmentation",terminal:false,detail:"Prompted-segmentation mask"},
      {artifact_id:"bbox-id",artifact_ref:"bbox:set",node_id:"bbox",lineage_id:"detection:ball",stage:"refined",source:"core.mask to bbox",terminal:false,value:{kind:"bounding_box",rect:[0.4,0.4,0.1,0.1]}},
    ],
  },
  ...overrides,
});

describe("prompted-segmentation execution evidence", () => {
  it("links the persisted mask prompt lineage to its terminal detection lineage", () => {
    const result=imageRefinementEvidence(sample(),["ball"],true).candidates[0];
    expect(result).toMatchObject({candidate_id:"ball",lineage_id:"detection:ball",source:"prompted_segmentation_refined",executed:true});
    expect(result.items.map(item=>item.label)).toEqual(["提示分割调用回执","Mask Artifact","mask_to_bbox 回执","mask_to_bbox Artifact"]);
  });

  it("recognizes the production refined outcome shape without trusting its geometry claim", () => {
    const value=sample();
    const candidate=value.projection!.review_candidates[0].candidate;
    candidate.lineage_id="detection:detection-0";
    candidate.outcome.id="refined:detection-0";
    candidate.geometry="Refined by prompted segmentation";
    value.projection!.debug_stages[0]={...value.projection!.debug_stages[0],node_id:"refine_validated_prompt",lineage_id:"detection:box-prompt:detection-0",artifact_ref:"mask-set:run:image:refine_validated_prompt"};
    value.projection!.debug_stages[1]={...value.projection!.debug_stages[1],node_id:"project_mask_bbox",lineage_id:"detection:detection-0",source:"core.mask to bbox"};
    value.nodes=[
      {...value.nodes[0],node_id:"refine_validated_prompt"},
      {...value.nodes[1],node_id:"project_mask_bbox"},
    ];

    expect(imageRefinementEvidence(value,["refined:detection-0"],true).candidates[0]).toMatchObject({
      source:"prompted_segmentation_refined",executed:true,
    });
  });

  it("does not promote a Draft refiner or an unrelated Mask Artifact to executed", () => {
    const value=sample();
    value.projection!.debug_stages[0].lineage_id="detection:box-prompt:other";
    const result=imageRefinementEvidence(value,["ball"],true).candidates[0];
    expect(result.source).toBe("vlm_only");
    expect(result.executed).toBe(false);
    expect(result.reason).toContain("没有形成");
  });

  it("explains the actual Coverage Gate stop instead of claiming SAM ran", () => {
    const value=sample({nodes:[],projection:{
      final_candidates:[],review_candidates:sample().projection!.review_candidates,
      committed_annotations:[],no_target:false,intermediate_artifact_ids:[],
      debug_stages:[{artifact_id:"coverage-id",artifact_ref:"coverage:set",node_id:"coverage",lineage_id:"detection:ball",stage:"prompt_coverage",source:"core prompt coverage",terminal:false,detail:"PartiallyCovered · RequestHumanReview"}],
    }});
    const result=imageRefinementEvidence(value,["ball"],true).candidates[0];
    expect(result).toMatchObject({source:"vlm_only",executed:false});
    expect(result.reason).toContain("PartiallyCovered");
  });
});
