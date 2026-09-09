import { expect,it } from "vitest";
import { removeCoveredChildren } from "./HistoryManagement";
import { parseAgentRoute } from "./navigationContract";
import { trashTargets } from "./TrashManagement";
import type { TrashEntry } from "../types";
import type { PipelineLifecycleSummary,ManagementObjectRef } from "../types";
it("native history routes resolve exact management page and owner",()=>{
  for(const page of ["runs","pipelines","trash"])expect(parseAgentRoute(new URL(`https://local/projects/project/manage/${page}`))).toEqual({kind:"management",projectId:"project",page});
});
it("Trash selection removes a Pipeline's same-identity child before requesting its impact",()=>{
  const objects:ManagementObjectRef[]=[{kind:"pipeline",id:"a",expected_revision:2},{kind:"workflow_draft",id:"a",expected_revision:2},{kind:"workflow_version",id:"a",version:1,expected_revision:3},{kind:"workflow_draft",id:"b",expected_revision:4}];
  expect(trashTargets(objects.map(object=>({object}) as TrashEntry))).toEqual([objects[0],objects[3]]);
});
it("bulk actions omit children already covered by their selected Pipeline",()=>{
  const parent:ManagementObjectRef={kind:"pipeline",id:"pipeline",expected_revision:1};
  const draft:ManagementObjectRef={kind:"workflow_draft",id:"draft",expected_revision:3};
  const other:ManagementObjectRef={kind:"workflow_draft",id:"other",expected_revision:4};
  const p:PipelineLifecycleSummary={project_id:"p",display_name:"Pipeline",lifecycle_revision:1,workflow_id:"pipeline",drafts:[{object:draft,display_name:"Draft",content_hash:"hash",is_default:false,historical_run_references:0}],versions:[]};
  expect(removeCoveredChildren([parent,draft,other],[p])).toEqual([parent,other]);
  expect(removeCoveredChildren([draft,other],[p])).toEqual([draft,other]);
});
