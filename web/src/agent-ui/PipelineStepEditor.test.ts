import {it,expect} from "vitest";
import {replacePipelineStep,stepSources} from "./PipelineStepEditor";
import type {PipelineStep,WorkflowDraft} from "../types";
const detector={id:"detect",node_type:"capability.detect",outputs:{detections:"detection_set"}} as unknown as PipelineStep;
const crop={id:"crop",node_type:"core.crop",outputs:{crops:"crop_set"},parameters:{padding:0.1}} as unknown as PipelineStep;
const draft={project_id:"p",label_pipeline:{schema_version:1,shared_stages:[{id:"shared",name:"Shared",steps:[detector]}],label_pipelines:[{id:"ball",target_task_id:"objects",target_label:"ball",steps:[crop]}]}} as unknown as WorkflowDraft;
const location={kind:"label",group:"ball",step:"crop"} as const;
it("offers typed shared sources without inventing detection identity",()=>{
  expect(stepSources(draft,location,"detection_set")).toEqual([{label:"共享 shared / detect · detections",source:{source:"shared_stage",stage_id:"shared",step_id:"detect",port:"detections",artifact_type:"detection_set"}}]);
  expect(stepSources(draft,location,"image")[0].source).toEqual({source:"image"});expect(stepSources(draft,location,"mask_set")).toEqual([]);
});
it("updates only exact current step and preserves original composition",()=>{
  const updated=replacePipelineStep(draft,location,crop,{...crop,parameters:{padding:0.2}});
  expect(updated.label_pipeline?.label_pipelines[0].steps[0].parameters).toEqual({padding:0.2});expect(crop.parameters).toEqual({padding:0.1});
  expect(()=>replacePipelineStep(updated,location,crop,crop)).toThrow();expect(()=>replacePipelineStep(draft,location,crop,{...crop,id:"foreign"})).toThrow();
});
