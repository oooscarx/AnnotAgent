import {expect,it} from "vitest";
import {readManualSchemaCommand,readClarificationCommand} from "./CreateTaskSchema";
const input={request_id:"11111111-1111-4111-8111-111111111111",decision:{decision:"draft",kind:"bounding_box",labels:["cup"],multi_label:false,attributes:{},boundary_rules:[],rationale:"human"}};
it("restores exact clarification answers without admitting journey execution authority",()=>{
  const reference={call_id:"c",expected_schema_revision:"r"};const saved={...input,clarification:reference};expect(readClarificationCommand(JSON.stringify(saved),reference)).toEqual(saved);
  for(const value of [{...saved,journey_consent_id:"paid"},{...saved,clarification:{...reference,call_id:"other"}},{...saved,clarification:{...reference,expected_schema_revision:"new"}},input])expect(()=>readClarificationCommand(JSON.stringify(value),reference)).toThrow();
});
it("retains exact manual semantics while excluding execution and clarification authority",()=>{
  expect(readManualSchemaCommand(JSON.stringify(input))).toEqual(input);
  for(const extra of [{journey_consent_id:"saved"},{clarification:{call_id:"call"}},{execute:true}])expect(()=>readManualSchemaCommand(JSON.stringify({...input,...extra}))).toThrow();
});
it("rejects incomplete recovery without creating substitute defaults",()=>{
  for(const value of [null,{}, {...input,request_id:""},{...input,decision:{...input.decision,labels:[]}},{...input,decision:{...input.decision,kind:"unknown"}}])expect(()=>readManualSchemaCommand(JSON.stringify(value))).toThrow();
});
