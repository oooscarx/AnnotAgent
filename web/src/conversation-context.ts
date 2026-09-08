import type { HumanRequest } from "./conversation-human-api";

/** A request can be a correction target or an origin; never apply its answer to a new test. */
export function conversationSampleRelation(request:HumanRequest, draft?:string, test?:string, image?:string): "subject"|"baseline"|"comparison"|"unrelated" {
  if(!test || !draft)return "unrelated";
  if(test===request.input.sample_test_id)return image===request.input.image_id ? "subject" : "baseline";
  if(request.status==="applied" && draft===request.resume_draft_id)return "comparison";
  return "unrelated";
}
