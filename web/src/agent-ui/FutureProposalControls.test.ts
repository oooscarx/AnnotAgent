import {expect,it} from "vitest";
import {readFutureExecution} from "./FutureProposalControls";
it("restores exact dispatch and stop intent without accepting new authority",()=>{
  const source={feedback_call_id:"feedback",scope_answer_command_id:"answer",context_digest:"digest",base_schema_id:"base",base_schema_revision:1};
  const value={pending:{source,consent:{call_id:"call",message_id:"message",model_id:"model",previous_grant_id:null,scope_hash:"hash",expires_at:"2030-01-01T00:00:00Z",allow_unknown_cost:true},summary:{model_name:"model",remote_model:"remote",destination:"TEST",data_scope:"text",operation:"proposal",maximum_output_tokens:100}},submitted:true,stop:true};
  expect(readFutureExecution(JSON.stringify(value),source)).toEqual(value);
  for(const changed of [{...value,submitted:"true"},{...value,stop:undefined},{...value,auto_retry:true},{...value,pending:{...value.pending,source:{...source,base_schema_revision:2}}}])expect(()=>readFutureExecution(JSON.stringify(changed),source)).toThrow();
});
