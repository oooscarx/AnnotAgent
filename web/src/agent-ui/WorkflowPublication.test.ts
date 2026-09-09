import {expect,it} from "vitest";
import {publicationRequest} from "./WorkflowPublication";
import type {WorkflowDraft} from "../types";
it("binds exact publication command to one owned saved revision and hash",()=>{
  const d={id:"draft",project_id:"project",revision:4,content_hash:"draft-hash"} as WorkflowDraft;
  expect(publicationRequest(d,"command")).toEqual({command_id:"command",project_id:"project",expected_revision:4,expected_content_hash:"draft-hash"});
  for(const invalid of [{...d,revision:0},{...d,revision:1.5},{...d,project_id:""},{...d,content_hash:""}])expect(()=>publicationRequest(invalid,"command")).toThrow();
});
