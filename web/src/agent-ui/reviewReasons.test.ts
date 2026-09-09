import {expect,it} from "vitest";
import type {SkillDetail} from "../types";
import {reviewReasonOptions,reviewDecisionReason} from "./reviewReasons";
const skill=(id:string,codes:string[])=>({id,display_name:id,correction_taxonomy:codes}) as SkillDetail;
it("only offers enabled Skill taxonomies and disambiguates identical codes",()=>{
  const options=reviewReasonOptions([skill("a",["boundary","boundary"]),skill("b",["boundary"]),skill("disabled",["hidden"])],["a","b"]);
  expect(options.filter(o=>o.code==="boundary")).toHaveLength(2);
  expect(options.some(o=>o.code==="hidden")).toBe(false);
  expect(reviewDecisionReason(options,JSON.stringify(["b","boundary"]),"reject","a",["a","b"])).toEqual({code:"boundary",skillId:"b"});
});
it("never submits a removed Skill or stale taxonomy key and preserves common reasons",()=>{
  const options=reviewReasonOptions([],[]);
  expect(()=>reviewDecisionReason(options,JSON.stringify(["removed","boundary"]),"reject","removed",[])).toThrow();
  expect(reviewDecisionReason(options,"wrong_object","reject","removed",[])).toEqual({code:"wrong_object",skillId:undefined});
  expect(reviewDecisionReason(options,"wrong_object","accept","active",["active"])).toEqual({code:"accepted_as_is",skillId:"active"});
});
