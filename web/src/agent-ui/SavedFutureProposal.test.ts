import {expect,it} from "vitest";
import {checkedFutureProposal} from "./SavedFutureProposal";
import type {FutureProposalStatus} from "../conversation-future-proposal-api";
it("accepts only the exact owned future-rule proposal context",()=>{
  const source={feedback_call_id:"f",scope_answer_command_id:"a",context_digest:"digest",base_schema_id:"s",base_schema_revision:2};
  const value={authorization:{source,grant:{task_id:"t"},context:{source,scope:"future_tasks_only",base_schema:{id:"s",revision:2,task_id:"t"}}}} as FutureProposalStatus;
  expect(checkedFutureProposal(value,"t",source)).toBe(value);
  expect(()=>checkedFutureProposal(value,"other",source)).toThrow();
  for(const change of [{feedback_call_id:"other"},{scope_answer_command_id:"other"},{context_digest:"other"},{base_schema_id:"other"},{base_schema_revision:3}])expect(()=>checkedFutureProposal(value,"t",{...source,...change})).toThrow();
});
