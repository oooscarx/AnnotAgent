import {expect,it} from "vitest";
import {workflowEdit,editableWorkflow} from "./WorkflowEditor";
import type {WorkflowDraft} from "../types";
const base={id:"d",project_id:"p",name:"before",revision:7,status:"editing",nodes:[],created_at:"old",updated_at:"old",content_hash:"hash"} as WorkflowDraft;
it("preserves immutable draft identity and exact revision",()=>{
  const next=workflowEdit(base,"after",JSON.stringify(editableWorkflow(base)));
  expect(next.id).toBe("d");expect(next.project_id).toBe("p");expect(next.revision).toBe(7);expect(next.name).toBe("after");
});
it("rejects protected keys and malformed config without saving",()=>{
  for(const value of ["{", "[]",'{"nodes":[],"revision":8}','{"nodes":[],"project_id":"other"}','{"nodes":null}'])expect(()=>workflowEdit(base,"name",value)).toThrow();
});
