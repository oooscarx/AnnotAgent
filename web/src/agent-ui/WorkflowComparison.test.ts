import {expect,it} from "vitest";
import {frozenDifferences} from "./WorkflowComparison";
import type {FrozenWorkflowVersion} from "../types";
it("compares complete authoring and execution fields without mistaking object key ordering for changes",()=>{
  const a={draft:{nodes:[{id:"a",config:{x:1,y:2}}]},snapshot:{models:[],draft:null}} as unknown as FrozenWorkflowVersion;
  const b={draft:{nodes:[{config:{y:2,x:1},id:"a"}]},snapshot:{draft:null,models:[]}} as unknown as FrozenWorkflowVersion;
  expect(frozenDifferences(a,b)).toEqual([]);
  const c={...b,snapshot:{...b.snapshot,models:["new"]},draft:{...b.draft,name:"added"}} as unknown as FrozenWorkflowVersion;
  expect(frozenDifferences(a,c).map(d=>d.path)).toEqual(["draft.name","snapshot.models"]);
});
