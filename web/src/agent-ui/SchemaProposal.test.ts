import {expect,it,vi} from "vitest";
import type {api} from "../api";
import {saveOwnedProposal} from "./SchemaProposal";
it("recovers a lost save by reading the original call without a second POST",async()=>{
  const draft={id:"d",task_id:"t",source_call_id:"call"};let saved=false;
  const read=vi.fn(async()=>saved?draft:null);const save=vi.fn(async()=>{saved=true;throw new Error("response lost");});
  const service={conversationSchemaDraftForCall:read,saveConversationSchemaDraft:save} as unknown as Pick<typeof api,"conversationSchemaDraftForCall"|"saveConversationSchemaDraft">;
  await expect(saveOwnedProposal(service,"p","conv","t","call")).rejects.toThrow("response lost");
  expect(await saveOwnedProposal(service,"p","conv","t","call")).toEqual(draft);expect(save).toHaveBeenCalledTimes(1);
  read.mockResolvedValue({...draft,source_call_id:"foreign"});await expect(saveOwnedProposal(service,"p","conv","t","call")).rejects.toThrow("原调用");expect(save).toHaveBeenCalledTimes(1);
});
