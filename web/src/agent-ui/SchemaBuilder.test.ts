import {expect,it} from "vitest";
import {matchesSchemaBuild} from "./SchemaBuilder";
import type {ConversationBuilderItem} from "../types";
const item={operation:{task_id:"t",evidence:{schema_id:"s",schema_revision:2}},schema_id:"s",schema_revision:2} as ConversationBuilderItem;
it("matches exact task and semantic revision without absorbing repairs or conflicting provenance",()=>{
  expect(matchesSchemaBuild(item,"t",{id:"s",revision:2})).toBe(true);
  expect(matchesSchemaBuild(item,"other",{id:"s",revision:2})).toBe(false);
  expect(matchesSchemaBuild(item,"t",{id:"s",revision:3})).toBe(false);
  expect(matchesSchemaBuild({...item,schema_id:"conflict"},"t",{id:"s",revision:2})).toBe(false);
  expect(matchesSchemaBuild({...item,operation:{...item.operation,evidence:{...item.operation.evidence,repair_source:{kind:"human_request"} as never}}},"t",{id:"s",revision:2})).toBe(false);
});
