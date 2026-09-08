import { describe, expect, it } from "vitest";
import { sampleAnnotations } from "./sampleAnnotations";
import { sampleFeedbackOverlay } from "./sampleFeedbackOverlay";
import type { SampleFeedbackRevision, SampleTestOutcomeRecord } from "./types";
const outcome:SampleTestOutcomeRecord={id:"cup",label:"cup",status:"needs_review",value:{kind:"bounding_box",rect:[0.1,0.2,0.3,0.4]}};
const original=sampleAnnotations([outcome,{...outcome,id:"other",label:"bottle"}],"image","test");
const revision:SampleFeedbackRevision={revision_id:"exclude",sample_test_id:"test",image_id:"image",sequence:1,reason:"exclude_target",outcome_id:"cup",corrected_value:null,corrected_label:null,note:"TEST explicit exclusion",created_at:"2026-09-08T00:00:00Z"};
describe("sample feedback overlay",()=>{
  it("hides only explicit excluded candidates, preserving inspectable original evidence and other objects",()=>{
    const snapshot=JSON.stringify(original);
    const result=sampleFeedbackOverlay(original,[revision]);
    expect(result.annotations.map(item=>item.id)).toEqual(["other"]);
    expect(result.excluded[0].annotation.value).toEqual(outcome.value);
    expect(result.excluded[0].revision).toEqual(revision);
    expect(JSON.stringify(original)).toBe(snapshot);
  });
  it("does not reinterpret legacy wrong_target as exclusion",()=>{
    expect(sampleFeedbackOverlay(original,[{...revision,reason:"wrong_target"}]).annotations).toEqual(original);
    expect(sampleFeedbackOverlay(original,[{...revision,reason:"wrong_target"}]).excluded).toEqual([]);
  });
  it("applies corrected classification tokens without hiding the remaining classes",()=>{
    const classes=sampleAnnotations([{...outcome,value:{kind:"classification",labels:["cup","indoor"]}}],"image","test");
    const result=sampleFeedbackOverlay(classes,[{...revision,reason:"wrong_target",corrected_value:{kind:"classification",labels:["indoor"]},corrected_label:"indoor"}]);
    expect(result.annotations[0].value).toEqual({kind:"classification",labels:["indoor"]});
    expect(result.excluded).toEqual([]);
  });
  it("retains baseline edits and additions while excluding only the selected object",()=>{
    const corrected={...revision,revision_id:"correct",sequence:1,reason:"poor_boundary" as const,corrected_value:{kind:"bounding_box" as const,rect:[0.2,0.2,0.1,0.1] as [number,number,number,number]}};
    const result=sampleFeedbackOverlay(original,[corrected,{...revision,sequence:2},{...revision,revision_id:"addition",sequence:3,reason:"missing_target",outcome_id:null,addition_id:"added",corrected_value:outcome.value,corrected_label:"cup"}]);
    expect(result.excluded[0].annotation.value).toEqual(corrected.corrected_value);
    expect(result.annotations.map(item=>item.id)).toEqual(["other","human-sample:added"]);
  });
});
