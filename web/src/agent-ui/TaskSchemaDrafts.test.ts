import {expect,it,vi} from "vitest";
import {readTaskSchemas,readTaskSchemaState,type TaskSchemaService} from "./TaskSchemaDrafts";
it("only reads saved owned schema proposals and deduplicates the same draft",async()=>{
  const draft={id:"d",task_id:"t",source_call_id:"c"};const service={humanConversationSchemas:vi.fn().mockResolvedValue([draft]),conversationSchemaCalls:vi.fn().mockResolvedValue([{id:"c",task_id:"t",status:"completed",evidence:{decision:{Ok:{decision:"draft"}}}},{id:"running",task_id:"t",status:"reserved"}]),conversationSchemaDraftForCall:vi.fn().mockResolvedValue(draft)} as unknown as TaskSchemaService;
  expect(await readTaskSchemas(service,"p","c","t",new AbortController().signal)).toEqual([draft]);expect(service.conversationSchemaDraftForCall).toHaveBeenCalledTimes(1);
});
it("preserves unsaved proposals without POST and rejects mismatched draft sources",async()=>{
  const call={id:"c",task_id:"t",status:"completed",evidence:{decision:{Ok:{decision:"draft"}}}};
  const read=vi.fn().mockResolvedValue(null);const save=vi.fn();const service={humanConversationSchemas:vi.fn().mockResolvedValue([]),conversationSchemaCalls:vi.fn().mockResolvedValue([call]),conversationSchemaDraftForCall:read,saveConversationSchemaDraft:save} as unknown as TaskSchemaService;
  expect(await readTaskSchemaState(service,"p","conv","t",new AbortController().signal)).toEqual({drafts:[],proposals:[call]});expect(save).not.toHaveBeenCalled();
  read.mockResolvedValue({id:"d",task_id:"t",source_call_id:"other"});await expect(readTaskSchemaState(service,"p","conv","t",new AbortController().signal)).rejects.toThrow("原调用");
});
it("rejects foreign call evidence before reading any referenced schema",async()=>{
  const service={humanConversationSchemas:vi.fn().mockResolvedValue([]),conversationSchemaCalls:vi.fn().mockResolvedValue([{task_id:"other"}]),conversationSchemaDraftForCall:vi.fn()} as unknown as TaskSchemaService;
  await expect(readTaskSchemas(service,"p","c","t",new AbortController().signal)).rejects.toThrow("不属于");expect(service.conversationSchemaDraftForCall).not.toHaveBeenCalled();
});
