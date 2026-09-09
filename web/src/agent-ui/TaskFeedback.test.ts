import {expect,it} from "vitest";
import {ownedFeedback} from "./TaskFeedback";
import type {FeedbackStatus} from "../conversation-feedback-api";
it("validates the message and grant owner before exposing saved feedback",()=>{
  const value={authorization:{context:{message:{conversation_id:"c",input:{id:"m"}}},grant:{task_id:"t"},consent:{message_id:"m"}}} as FeedbackStatus;
  expect(ownedFeedback(value,"c","t","m")).toBe(value);
  for(const [c,t,m] of [["foreign","t","m"],["c","foreign","m"],["c","t","foreign"]])expect(()=>ownedFeedback(value,c,t,m)).toThrow();
});
