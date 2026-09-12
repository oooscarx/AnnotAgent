import {expect,it} from "vitest";
import {projectCallMessages} from "./messageProjection";
it("projects only persisted model content and keeps its receipt identity",()=>{
  const [item]=projectCallMessages([{id:"call",task_id:"task",status:"completed",evidence:{decision:{Ok:{decision:"clarify",question:"需要框还是分类？",rationale:"输出类型尚未明确"}}}}]);
  expect(item).toMatchObject({role:"assistant",kind:"clarification",text:"需要框还是分类？",source:{kind:"model_call",id:"call",status:"completed"}});expect(item.details).toEqual(["输出类型尚未明确"]);
});
it("does not fabricate an Agent reply for failures or empty content",()=>{
  expect(projectCallMessages([{id:"failed",task_id:"task",status:"failed",evidence:{error:"safe error"}}])).toEqual([]);
  expect(projectCallMessages([{id:"empty",task_id:"task",status:"completed",evidence:{decision:{Ok:{decision:"draft",rationale:""}}}}])).toEqual([]);
});
