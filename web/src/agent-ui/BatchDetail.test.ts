import { expect, it } from "vitest";
import { batchControls, ownedBatch } from "./BatchDetail";
import { parseAgentRoute } from "./navigationContract";
import type { DatasetBatchSummary } from "../types";
const batch={id:"b",project_id:"p",status:"running",in_trash:false} as DatasetBatchSummary;
it("uses native Batch detail and rejects foreign ownership",()=>{
  expect(parseAgentRoute(new URL("http://localhost/projects/p/manage/batches/b"))).toEqual({kind:"detail",projectId:"p",page:"batches",objectId:"b"});
  expect(ownedBatch(batch,"p","b")).toBe(batch);
  expect(()=>ownedBatch(batch,"other","b")).toThrow();expect(()=>ownedBatch(batch,"p","other")).toThrow();
});
it("preserves supported controls without inventing terminal resume",()=>{
  expect(batchControls(batch)).toEqual(["pause","cancel"]);
  expect(batchControls({...batch,status:"paused"})).toEqual(["resume","cancel"]);
  expect(batchControls({...batch,in_trash:true})).toEqual([]);
  for(const status of ["failed","interrupted","cancelled","completed"] as const)expect(batchControls({...batch,status})).toEqual([]);
});
