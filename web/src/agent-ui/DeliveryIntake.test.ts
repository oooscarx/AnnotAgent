import { describe, expect, it } from "vitest";
import { intakeLabels,mergeDeliveryProposal } from "./DeliveryIntake";

describe("delivery label intent", () => {
  it("preserves existing IDs, aliases and boundaries while preserving user order", () => {
    const previous = [{stable_id:"stable-z",display_name:"斑马",aliases:["zebra"],include:"完整或遮挡目标",exclude:"图案"},{stable_id:"stable-a",display_name:"苹果",aliases:[],include:"",exclude:""}];
    expect(intakeLabels("苹果\n斑马", previous, () => { throw new Error("should reuse IDs"); })).toEqual([previous[1],previous[0]]);
  });
  it("new names get stable independent IDs without implying shape or a detector class", () => {
    expect(intakeLabels("\n球\n",[],()=>"stable-id")).toEqual([{stable_id:"stable-id",display_name:"球",aliases:[],include:"",exclude:""}]);
  });
  it("adopts explicit known identities while preserving other slots and rejecting foreign label references",()=>{
    const previous=[{stable_id:"a",display_name:"杯子",aliases:[],include:"old",exclude:""}];
    const proposal={call_id:"TEST",question:"什么任务？",semantics:{training_target:null,labels:[{existing_id:"a",display_name:"水杯",aliases:["cup"],include:"new",exclude:"图案"},{existing_id:null,display_name:"瓶子",aliases:[],include:"",exclude:""}]}};
    const merged=mergeDeliveryProposal(previous,proposal,()=>"new-id");
    expect(merged.map(l=>l.stable_id)).toEqual(["a","new-id"]);expect(merged[0].display_name).toBe("水杯");expect(previous[0].display_name).toBe("杯子");
    expect(proposal.semantics.training_target).toBeNull();
    expect(()=>mergeDeliveryProposal([],{...proposal,semantics:{...proposal.semantics,labels:[proposal.semantics.labels[0]]}})).toThrow("不在当前交付版本");
  });
});
