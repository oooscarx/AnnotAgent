import {expect,it} from "vitest";
import {replayScopeAllowed,assertReplayReceipt} from "./NodeReplay";
import type {NodeReplayPreview,NodeReplayCommand,NodeReplayReceipt} from "../types";
const preview:NodeReplayPreview={project_id:"p",source_run_id:"r",node_id:"n",scope_hash:"hash",source_record_hash:"record",source_snapshot_hash:"snapshot",checkpoint_hash:"checkpoint",image_hash:"image",downstream_nodes:[],preserved_upstream_nodes:[],current_bindings:[],available:true,refusal_reasons:[],limits:{maximum_model_requests:0,unknown_cost:false,timeout_seconds:30},destinations:{sandbox:true,formal_annotations:false,source_checkpoint_write:false,published_write:false}};
it("only admits exact owned zero-external-call sandbox previews",()=>{
  expect(replayScopeAllowed(preview,"p","r","n")).toBe(true);expect(()=>replayScopeAllowed(preview,"other","r","n")).toThrow();
  expect(replayScopeAllowed({...preview,available:false,refusal_reasons:["current_binding_replay_unsupported"]},"p","r","n")).toBe(false);
  expect(replayScopeAllowed({...preview,limits:{...preview.limits,maximum_model_requests:1}},"p","r","n")).toBe(false);
  expect(replayScopeAllowed({...preview,destinations:{...preview.destinations,published_write:true}},"p","r","n")).toBe(false);
});
it("rejects changed receipts instead of displaying another command as success",()=>{
  const c={project_id:"p",command_id:"c",scope_hash:"hash",maximum_model_requests:0,allow_unknown_cost:false} as NodeReplayCommand;
  const r={project_id:"p",run_id:"r",node_id:"n",command_id:"c",request:c,status:"outcome_unknown",result:null} as NodeReplayReceipt;
  expect(assertReplayReceipt(r,c,"r","n")).toBe(r);expect(()=>assertReplayReceipt({...r,request:{...c,scope_hash:"other"}},c,"r","n")).toThrow();
  expect(()=>assertReplayReceipt({...r,node_id:"other"},c,"r","n")).toThrow();
});
