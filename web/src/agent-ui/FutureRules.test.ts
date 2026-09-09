import {expect,it} from "vitest";
import {ownedFutureRules} from "./FutureRules";
import type {FutureSchemaState} from "../conversation-future-schema-api";
it("checks future-only scope, task and exact feedback intent before showing rules",()=>{
  const value={scope:"future_tasks_only",base_schema:{task_id:"t"},source:{scope_answer_command_id:"cmd",context_digest:"digest"},schema:null,record:null} as FutureSchemaState;
  expect(ownedFutureRules(value,"t","call","cmd","digest")).toBe(value);
  for(const change of [{scope:"all"},{base_schema:{task_id:"other"}},{source:{scope_answer_command_id:"other",context_digest:"digest"}},{source:{scope_answer_command_id:"cmd",context_digest:"other"}},{schema:{task_id:"other"}},{record:{schema_id:"d",input:{feedback_call_id:"other"}}}])expect(()=>ownedFutureRules({...value,...change} as FutureSchemaState,"t","call","cmd","digest")).toThrow();
});
