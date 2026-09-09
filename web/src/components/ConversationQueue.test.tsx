import { expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { QueueRows, type QueuedMessage } from "./ConversationQueue";
const row = (status: QueuedMessage["status"]): QueuedMessage => ({ input: { message: { id:"TEST-message", text:"TEST supplement", image:{image_id:"TEST-image",sha256:"a".repeat(64)} }, task_id:"TEST-task", schema_revision:"a".repeat(64), mode:"plan" }, receipt:{message:{sequence:1,input:{id:"TEST-message",text:"TEST supplement"}} as QueuedMessage["receipt"]["message"],task_id:"TEST-task",disposition:"task_message",mode:"plan",agent_model:{revision:2,model_profile_id:"TEST-frozen-model"}}, status, cancelled_at:null, planning_call_id:null });
it("shows frozen context and does not claim waiting instructions have executed",()=>{
  const html=renderToStaticMarkup(<QueueRows rows={[row("waiting_for_dispatch")]} busy={false} onCancel={()=>{}}/>);
  for(const text of ["TEST supplement","TEST-image","TEST-frozen-model","Queued · not executed","Plan requested","Cancel queued instruction"])expect(html).toContain(text);
});
it("does not cancel running or completed calls through the inbox",()=>{
  for(const status of ["running","completed","failed","cancelled"] as const){
    const html=renderToStaticMarkup(<QueueRows rows={[row(status)]} busy={false} onCancel={()=>{}}/>);
    expect(html).not.toContain("Cancel queued instruction");
    if(status==="running")expect(html).toContain("task stop control");
  }
});
