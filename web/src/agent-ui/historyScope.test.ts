import { expect, it } from "vitest";
import { historyCommand, historyPolicy, historyQuery, validateHistoryScope, type HistoryScope, type HistoryPreview } from "./historyScope";
const scope:HistoryScope={id:"scope",revision:1,policy:historyPolicy,establishment_command_id:"command",established_at:"2026-09-10"};
const preview:HistoryPreview={policy:historyPolicy,expected_snapshot_hash:"hash",excluded_counts:{run:2},preserves_published_versions:true,preserves_annotations:true,preserves_direct_references:true};
it("requires server scope and bounds pagination without an unscoped fallback",()=>{
  expect(historyQuery(scope,50).toString()).toBe("history_scope=scope&limit=50&offset=50");
  for(const invalid of [-1,0.5,Infinity,NaN])expect(()=>historyQuery(scope,invalid)).toThrow();
  expect(()=>validateHistoryScope({...scope,id:""})).toThrow();
  expect(()=>validateHistoryScope({...scope,revision:2})).toThrow();
});
it("freezes exact preview identity and requires every preservation promise",()=>{
  expect(historyCommand(preview,"command")).toEqual({command_id:"command",expected_scope_revision:null,expected_snapshot_hash:"hash",policy:historyPolicy,confirmed:true});
  for(const key of ["preserves_published_versions","preserves_annotations","preserves_direct_references"] as const)expect(()=>historyCommand({...preview,[key]:false},"command")).toThrow();
  expect(()=>historyCommand({...preview,expected_snapshot_hash:""},"command")).toThrow();
});
