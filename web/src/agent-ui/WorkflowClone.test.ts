import {expect,it} from "vitest";
import type {FrozenWorkflowVersion} from "../types";
import {cloneRequest} from "./WorkflowClone";
it("binds clone to the published snapshot hash rather than the authoring draft hash",()=>{
  const source={project_id:"p",workflow_id:"w",version:3,content_hash:"frozen",draft:{content_hash:"authoring"}} as FrozenWorkflowVersion;
  expect(cloneRequest(source,"command")).toEqual({project_id:"p",source_snapshot_hash:"frozen",command_id:"command"});
  expect(()=>cloneRequest({...source,content_hash:""},"command")).toThrow();
});
