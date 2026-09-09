import {expect,it} from "vitest";
import {appendCatalogNode} from "./WorkflowCatalogEditor";
import type {WorkflowDraft,WorkflowCatalog} from "../types";
const draft={id:"d",project_id:"p",nodes:[],revision:2} as unknown as WorkflowDraft;
const catalog={project_id:"p",node_catalog:[{id:"core.image_input",config_schema:{properties:{test:{default:3}}},input_ports:[],output_ports:[{name:"image",artifact_type:"image",required:true,cardinality:"one"}]}]} as unknown as WorkflowCatalog;
it("adds only actual Registry nodes without invented models or connections",()=>{
  const next=appendCatalogNode(draft,catalog,"core.image_input","","n");expect(next.nodes[0].parameters).toEqual({test:3});expect(next.nodes[0].model_binding).toBeUndefined();expect(next.nodes[0].depends_on).toEqual([]);expect(draft.nodes).toEqual([]);expect(next.revision).toBe(2);
  expect(()=>appendCatalogNode(draft,catalog,"fake","","n")).toThrow();expect(()=>appendCatalogNode(draft,{...catalog,project_id:"other"},"core.image_input","","n")).toThrow();
});
it("edits the authoritative composition instead of overwritten compiled nodes",()=>{
  const composed={...draft,label_pipeline:{schema_version:1,shared_stages:[{id:"s",name:"Shared",steps:[]}],label_pipelines:[]}};
  const next=appendCatalogNode(composed,catalog,"core.image_input","shared:s","n");expect(next.nodes).toEqual([]);expect(next.label_pipeline?.shared_stages[0].steps[0].outputs).toEqual({image:"image"});expect(composed.label_pipeline.shared_stages[0].steps).toEqual([]);
  expect(()=>appendCatalogNode(next,catalog,"core.image_input","shared:s","n")).toThrow();
});
