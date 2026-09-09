import {expect,it} from "vitest";
import {ownedClarification} from "./SchemaClarifications";
it("rejects foreign clarification owners and unknown states",()=>{
  const item={id:"call",task_id:"task",conversation_id:"conversation",status:"pending",schema_draft_id:null,question:"Which label?",expected_schema_revision:"revision"};
  expect(ownedClarification(item,"conversation","task","call")).toEqual(item);
  for(const change of [{id:"other"},{task_id:"other"},{conversation_id:"other"},{status:"completed"},{expected_schema_revision:""}])expect(()=>ownedClarification({...item,...change},"conversation","task","call")).toThrow();
});
