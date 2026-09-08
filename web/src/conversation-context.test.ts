import { describe, expect, it } from "vitest";
import { conversationSampleRelation } from "./conversation-context";
import type { HumanRequest } from "./conversation-human-api";

describe("conversation sample origin",()=>{
  const request:HumanRequest={input:{id:"request",task_id:"task",conversation_id:"conversation",sample_test_id:"baseline",image_id:"image",content_hash:"hash",outcome_id:"box",expected_feedback_sequence:0,reason_code:"boundary",question:"TEST",resume_checkpoint_ref:"copy"},status:"applied",resume_draft_id:"copy"};
  it("keeps the original subject distinct from comparison and other images",()=>{
    expect(conversationSampleRelation(request,"draft","baseline","image")).toBe("subject");
    expect(conversationSampleRelation(request,"draft","baseline","other")).toBe("baseline");
    expect(conversationSampleRelation(request,"copy","new-test","image")).toBe("comparison");
    expect(conversationSampleRelation(request,"unrelated","new-test","image")).toBe("unrelated");
    expect(conversationSampleRelation({...request,status:"pending"},"copy","new-test","image")).toBe("unrelated");
  });
});
