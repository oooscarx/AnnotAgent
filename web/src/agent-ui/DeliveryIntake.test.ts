import { describe, expect, it } from "vitest";
import { intakeLabels } from "./DeliveryIntake";

describe("delivery label intent", () => {
  it("preserves existing IDs, aliases and boundaries while preserving user order", () => {
    const previous = [{stable_id:"stable-z",display_name:"斑马",aliases:["zebra"],include:"完整或遮挡目标",exclude:"图案"},{stable_id:"stable-a",display_name:"苹果",aliases:[],include:"",exclude:""}];
    expect(intakeLabels("苹果\n斑马", previous, () => { throw new Error("should reuse IDs"); })).toEqual([previous[1],previous[0]]);
  });
  it("new names get stable independent IDs without implying shape or a detector class", () => {
    expect(intakeLabels("\n球\n",[],()=>"stable-id")).toEqual([{stable_id:"stable-id",display_name:"球",aliases:[],include:"",exclude:""}]);
  });
});
