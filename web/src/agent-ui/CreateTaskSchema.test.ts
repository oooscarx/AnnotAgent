import {expect,it} from "vitest";
import {readManualSchemaCommand} from "./CreateTaskSchema";
const input={request_id:"11111111-1111-4111-8111-111111111111",decision:{decision:"draft",kind:"bounding_box",labels:["cup"],multi_label:false,attributes:{},boundary_rules:[],rationale:"human"}};
it("retains exact manual semantics while excluding execution and clarification authority",()=>{
  expect(readManualSchemaCommand(JSON.stringify(input))).toEqual(input);
  for(const extra of [{journey_consent_id:"saved"},{clarification:{call_id:"call"}},{execute:true}])expect(()=>readManualSchemaCommand(JSON.stringify({...input,...extra}))).toThrow();
});
it("rejects incomplete recovery without creating substitute defaults",()=>{
  for(const value of [null,{}, {...input,request_id:""},{...input,decision:{...input.decision,labels:[]}},{...input,decision:{...input.decision,kind:"unknown"}}])expect(()=>readManualSchemaCommand(JSON.stringify(value))).toThrow();
});
