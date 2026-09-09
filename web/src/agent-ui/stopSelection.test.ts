import {expect,it} from "vitest";
import {ownedStopSelection} from "./stopSelection";
import type {StopRequestRecord} from "../conversation-stop-api";
const target={kind:"builder" as const,id:"op",task_id:"task"};
const record={message:{conversation_id:"conversation",input:{id:"stop",text:"停止",image:null,reference:{scope:"stop_request",task_id:"task"}}},selected_target:null} as StopRequestRecord;
const local={message_id:"stop",pending:true,target};
it("restores exact selected target and rejects replacement or foreign acknowledgements",()=>{
  expect(ownedStopSelection(record,"conversation","stop",JSON.stringify(local))).toEqual(local);
  expect(ownedStopSelection({...record,selected_target:target},"conversation","stop",JSON.stringify(local))).toEqual(local);
  expect(()=>ownedStopSelection({...record,selected_target:{...target,id:"other"}},"conversation","stop",JSON.stringify(local))).toThrow("不同停止目标");
  expect(()=>ownedStopSelection(record,"other","stop",null)).toThrow();expect(()=>ownedStopSelection(record,"conversation","other",null)).toThrow();
});
it("cleared local state is not a new selection and corrupt data fails closed",()=>{
  expect(ownedStopSelection(record,"conversation","stop","null")).toBeUndefined();
  for(const raw of ["{}","broken",JSON.stringify({...local,message_id:"other"})])expect(()=>ownedStopSelection(record,"conversation","stop",raw)).toThrow();
});
