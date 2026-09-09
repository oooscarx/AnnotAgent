import {expect,it} from "vitest";
import {parseLimitMaximum,restoreLimitCommand} from "./ProjectCallLimit";
it("requires explicit integer cumulative limits without resetting reservations",()=>{
  expect(parseLimitMaximum("12",12)).toBe(12);for(const value of ["","-1","1.2","1e3","Infinity","11","9007199254740992"])expect(()=>parseLimitMaximum(value,12)).toThrow();
});
it("restores only exact well-formed revision-bound requests",()=>{
  const value={id:"00000000-0000-4000-8000-000000000001",expected_revision:4,maximum_calls:10};expect(restoreLimitCommand(JSON.stringify(value))).toEqual(value);
  for(const change of [{id:""},{expected_revision:-1},{maximum_calls:null},{maximum_calls:"10"}])expect(()=>restoreLimitCommand(JSON.stringify({...value,...change}))).toThrow();
});
