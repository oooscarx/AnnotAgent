import {expect,it} from "vitest";
import {matchesSchemaBuild,schemaBuildOutcome} from "./SchemaBuilder";
import type {ConversationBuilderItem} from "../types";
const item={operation:{task_id:"t",evidence:{schema_id:"s",schema_revision:2}},schema_id:"s",schema_revision:2} as ConversationBuilderItem;
it("does not present a completed failed operation receipt as a generated Draft",()=>{
  const settled={...item,operation:{...item.operation,status:"completed" as const}};
  expect(schemaBuildOutcome(settled)).toContain("未确认");
  expect(schemaBuildOutcome({...settled,operation:{...settled.operation,evidence:{...settled.operation.evidence,outcome:"failed"}}})).toBe("构建失败");
  expect(schemaBuildOutcome({...settled,operation:{...settled.operation,evidence:{...settled.operation.evidence,outcome:"draft_ready_for_human_review",draft_id:"draft"}}})).toContain("草稿已生成");
});
it("matches exact task and semantic revision without absorbing repairs or conflicting provenance",()=>{
  expect(matchesSchemaBuild(item,"t",{id:"s",revision:2})).toBe(true);
  expect(matchesSchemaBuild(item,"other",{id:"s",revision:2})).toBe(false);
  expect(matchesSchemaBuild(item,"t",{id:"s",revision:3})).toBe(false);
  expect(matchesSchemaBuild({...item,schema_id:"conflict"},"t",{id:"s",revision:2})).toBe(false);
  expect(matchesSchemaBuild({...item,operation:{...item.operation,evidence:{...item.operation.evidence,repair_source:{kind:"human_request"} as never}}},"t",{id:"s",revision:2})).toBe(false);
});
