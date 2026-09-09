import { expect, it } from "vitest";
import { parsePendingSend, sameSendCommand } from "./conversation-send";
const pending = {conversation:"00000000-0000-4000-8000-000000000001",input:{message:{id:"00000000-0000-4000-8000-000000000002",text:"测试",image:null},task_id:null,schema_revision:"a".repeat(64)}};
it("restores a pending command without deriving task from current UI",()=>{
  expect(parsePendingSend(JSON.stringify(pending))).toEqual(pending);
  expect(parsePendingSend("not JSON")).toBeUndefined();
  expect(parsePendingSend(JSON.stringify({...pending,conversation:"https://external.example"}))).toBeUndefined();
  expect(parsePendingSend(JSON.stringify({...pending,input:{...pending.input,task_id:"other"}}))).toBeUndefined();
  expect(parsePendingSend(JSON.stringify({...pending,input:{...pending.input,message:{...pending.input.message,reference:{scope:"stop_request",task_id:null}}}}))).toBeUndefined();
});
it("compares saved commands independent of JSON key order but not scope",()=>{
  expect(sameSendCommand(pending.input,{schema_revision:pending.input.schema_revision,task_id:null,message:{text:"测试",image:null,id:pending.input.message.id}})).toBe(true);
  expect(sameSendCommand(pending.input,{...pending.input,schema_revision:"b".repeat(64)})).toBe(false);
});
