import { expect, it } from "vitest";
import { parsePendingSend, sameSendCommand } from "./conversation-send";
import type { SendMode } from "./conversation-send";
const pending = {conversation:"00000000-0000-4000-8000-000000000001",input:{message:{id:"00000000-0000-4000-8000-000000000002",text:"测试",image:null},task_id:null,schema_revision:"a".repeat(64)}};
it("restores the exact requested mode without granting or upgrading legacy commands",()=>{
  for(const mode of ["plan","execute"] as SendMode[]){
    const value={...pending,input:{...pending.input,mode}};
    expect(parsePendingSend(JSON.stringify(value))).toEqual(value);
    expect(sameSendCommand(value.input,{...value.input,mode:mode==="plan"?"execute":"plan"})).toBe(false);
    expect(sameSendCommand(value.input,pending.input)).toBe(false);
  }
  for(const mode of [null,"unrestricted",true,{},"Plan"])
    expect(parsePendingSend(JSON.stringify({...pending,input:{...pending.input,mode}}))).toBeUndefined();
});
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
it("retains an observed model choice and rejects corrupt recovery metadata",()=>{
  const choice={revision:3,model_profile_id:pending.conversation};
  const value={...pending,input:{...pending.input,agent_model:choice}};
  expect(parsePendingSend(JSON.stringify(value))).toEqual(value);
  expect(sameSendCommand(value.input,{...value.input,agent_model:{...choice,revision:4}})).toBe(false);
  expect(sameSendCommand(value.input,{...value.input,agent_model:{...choice,model_profile_id:null}})).toBe(false);
  for(const invalid of [null,{}, {revision:-1,model_profile_id:null},{revision:1.5,model_profile_id:null},{revision:3,model_profile_id:"foreign"}]) {
    expect(parsePendingSend(JSON.stringify({...pending,input:{...pending.input,agent_model:invalid}}))).toBeUndefined();
  }
});
it("restores an exact new-task image scope and rejects changed or foreign scope",()=>{
  const image={image_id:"00000000-0000-4000-8000-000000000003",sha256:"b".repeat(64)};
  const value={...pending,input:{...pending.input,task_images:[image]}};
  expect(parsePendingSend(JSON.stringify(value))).toEqual(value);
  expect(sameSendCommand(value.input,{...value.input,task_images:[{...image,sha256:"c".repeat(64)}]})).toBe(false);
  expect(parsePendingSend(JSON.stringify({...value,input:{...value.input,task_id:"00000000-0000-4000-8000-000000000004"}}))).toBeUndefined();
  expect(parsePendingSend(JSON.stringify({...value,input:{...value.input,task_images:[image,image]}}))).toBeUndefined();
  expect(parsePendingSend(JSON.stringify({...value,input:{...value.input,task_images:[{...image,sha256:"not-a-hash"}]}}))).toBeUndefined();
});
