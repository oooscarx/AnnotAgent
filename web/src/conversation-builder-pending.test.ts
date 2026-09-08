import { describe, expect, it } from "vitest";
import { builderConsent, restoreBuilderPending } from "./conversation-builder-pending";
import { builderMatchesClassRepair } from "./conversation-schema-history";
import type { ConversationBuilderItem, ConversationBuilderPreview } from "./types";

const schema={id:"TEST-schema",revision:2}, source={id:"TEST-review",draft:"TEST-repair",kind:"image_class_review" as const};
const preview:ConversationBuilderPreview={selection:{operation_id:"TEST-op",schema_id:schema.id,schema_revision:2,model_id:"TEST-model",image_class_review_id:source.id},image_class_repair:{review_id:source.id,draft_id:source.draft,revision:4,content_hash:"a".repeat(64),schema_id:schema.id,schema_revision:2,scope_digest:"b".repeat(64),feedback_digest:"c".repeat(64)},previous_grant_id:null,scope_hash:"d".repeat(64),expires_at:"2026-01-01T00:00:00Z",model_name:"TEST",remote_model:"TEST",destination:"TEST local",maximum_builder_calls:8,maximum_calls:16,used_calls:8,image_count:0,estimated_cost:null,data_scope:"TEST metadata",operation:"TEST repair only"};
const envelope=()=>({preview:structuredClone(preview),consent:builderConsent(structuredClone(preview))});
describe("exact Builder retry recovery",()=>{
  it("retains an expired original envelope for receipt lookup without creating new authority",()=>{
    const saved=envelope();expect(restoreBuilderPending(JSON.stringify(saved),schema,source)).toEqual(saved);
  });
  it("rejects wrong task source, Schema, draft and changed authorization",()=>{
    expect(restoreBuilderPending(JSON.stringify(envelope()),{...schema,revision:3},source)).toBeUndefined();
    expect(restoreBuilderPending(JSON.stringify(envelope()),schema,{...source,id:"TEST-other"})).toBeUndefined();
    expect(restoreBuilderPending(JSON.stringify(envelope()),schema,{...source,draft:"TEST-other"})).toBeUndefined();
    expect(restoreBuilderPending(JSON.stringify(envelope()),schema,{...source,kind:"human_request"})).toBeUndefined();
    expect(restoreBuilderPending(JSON.stringify(envelope()),schema)).toBeUndefined();
    for(const changed of [{...envelope(),consent:{...builderConsent(preview),allow_unknown_cost:false}},{...envelope(),consent:{...builderConsent(preview),scope_hash:"e".repeat(64)}},{...envelope(),preview:{...preview,maximum_calls:"unknown"}},{preview:{},consent:{}}])expect(restoreBuilderPending(JSON.stringify(changed),schema,source)).toBeUndefined();
    expect(restoreBuilderPending("broken",schema,source)).toBeUndefined();
  });
  it("matches admitted provenance before session seed, not another repair's working Draft",()=>{
    const entry={operation:{id:"TEST-op",task_id:"TEST-task",request_hash:"a".repeat(64),status:"reserved",evidence:{schema_id:schema.id,schema_revision:2,repair_source:{kind:"image_class_review",reference:preview.image_class_repair!}}}} as ConversationBuilderItem;
    expect(builderMatchesClassRepair(entry,schema,source)).toBe(true);
    expect(builderMatchesClassRepair(entry,schema,{...source,id:"TEST-other"})).toBe(false);
    expect(builderMatchesClassRepair({...entry,operation:{...entry.operation,evidence:{schema_id:schema.id,schema_revision:2}}},schema,source)).toBe(false);
    expect(builderMatchesClassRepair({...entry,schema_revision:3},schema,source)).toBe(false);
  });
});
