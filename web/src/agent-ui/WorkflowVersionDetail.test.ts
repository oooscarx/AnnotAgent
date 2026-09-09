import {it,expect} from "vitest";
import {parseVersion,versionLink} from "./WorkflowVersionDetail";
it("rejects ambiguous versions rather than opening the default",()=>{
  expect(parseVersion("1")).toBe(1);for(const value of ["", "0","01","-1","1.5","1e2","NaN","9007199254740992"])expect(parseVersion(value)).toBeUndefined();
  expect(versionLink("p","workflow@3")).toBe("/projects/p/manage/pipelines/workflow?version=3");expect(versionLink("p","workflow")).toBeUndefined();
});
