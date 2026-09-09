import { expect, it } from "vitest";
import { assertRunOwner, runControls } from "./RunDetail";
import type { HistoryRun } from "../types";
const run={id:"r",project_id:"p",ownership_status:"resolved",controllable:true,status:"running",in_trash:false} as HistoryRun;
it("rejects another project, wrong ID and unresolved legacy ownership",()=>{
  expect(assertRunOwner(run,"p","r")).toBe(run);
  expect(()=>assertRunOwner(run,"other","r")).toThrow();
  expect(()=>assertRunOwner(run,"p","other")).toThrow();
  expect(()=>assertRunOwner({...run,ownership_status:"legacy_orphan"},"p","r")).toThrow();
});
it("never invents resume controls for unknown or terminal executions",()=>{
  expect(runControls(run)).toEqual(["pause","cancel"]);
  expect(runControls({...run,status:"paused"})).toEqual(["resume","cancel"]);
  for(const status of ["failed","interrupted","completed","cancelled"] as const)expect(runControls({...run,status})).toEqual([]);
  expect(runControls({...run,controllable:false})).toEqual([]);
  expect(runControls({...run,in_trash:true})).toEqual([]);
});
