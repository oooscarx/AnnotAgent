import {expect,it,vi} from "vitest";
import {readTaskSchemas,type TaskSchemaService} from "./TaskSchemaDrafts";
it("only reads saved owned schema proposals and deduplicates the same draft",async()=>{
  const draft={id:"d",task_id:"t"};const service={humanConversationSchemas:vi.fn().mockResolvedValue([draft]),conversationSchemaCalls:vi.fn().mockResolvedValue([{id:"c",task_id:"t",status:"completed",evidence:{decision:{Ok:{decision:"draft"}}}},{id:"running",task_id:"t",status:"reserved"}]),conversationSchemaDraftForCall:vi.fn().mockResolvedValue(draft)} as unknown as TaskSchemaService;
  expect(await readTaskSchemas(service,"p","c","t",new AbortController().signal)).toEqual([draft]);expect(service.conversationSchemaDraftForCall).toHaveBeenCalledTimes(1);
});
it("rejects foreign call evidence before reading any referenced schema",async()=>{
  const service={humanConversationSchemas:vi.fn().mockResolvedValue([]),conversationSchemaCalls:vi.fn().mockResolvedValue([{task_id:"other"}]),conversationSchemaDraftForCall:vi.fn()} as unknown as TaskSchemaService;
  await expect(readTaskSchemas(service,"p","c","t",new AbortController().signal)).rejects.toThrow("不属于");expect(service.conversationSchemaDraftForCall).not.toHaveBeenCalled();
});
