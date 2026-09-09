import {it,expect} from "vitest";
import {parseVersion,versionLink,verifyFrozenVersion} from "./WorkflowVersionDetail";
import type {FrozenWorkflowVersion} from "../types";
it("rejects ambiguous versions rather than opening the default",()=>{
  expect(parseVersion("1")).toBe(1);for(const value of ["", "0","01","-1","1.5","1e2","NaN","9007199254740992"])expect(parseVersion(value)).toBeUndefined();
  expect(versionLink("p","workflow@3")).toBe("/projects/p/manage/pipelines/workflow?version=3");expect(versionLink("p","workflow")).toBeUndefined();
});
it("requires the exact frozen identity, owner and source instead of using current metadata",()=>{
  const value={workflow_id:"w",version:2,project_id:"p",source_draft_id:"d",content_hash:"snapshot",draft:{id:"d",project_id:"p"}} as FrozenWorkflowVersion;
  expect(verifyFrozenVersion(value,"p","w",2)).toBe(value);
  for(const wrong of [{...value,version:3},{...value,project_id:"other"},{...value,workflow_id:"other"},{...value,source_draft_id:"other"},{...value,content_hash:""}])expect(()=>verifyFrozenVersion(wrong,"p","w",2)).toThrow();
});
