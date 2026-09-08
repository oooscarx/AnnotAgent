import {expect,it} from "vitest";
import {renderToStaticMarkup} from "react-dom/server";
import {ConversationHumanRequests} from "./ConversationHumanRequests";
import type {HumanRequest} from "../conversation-human-api";

const request=(id:string,task:string,status:HumanRequest["status"]):HumanRequest=>({input:{id,task_id:task,conversation_id:"conversation",sample_test_id:"sample",image_id:"image",content_hash:"hash",outcome_id:"outcome",expected_feedback_sequence:0,reason_code:"boundary",question:id,resume_checkpoint_ref:"checkpoint"},status});
const render=(task?:string,active?:string,ready=true)=>renderToStaticMarkup(<ConversationHumanRequests requests={[request("current-pending","current","pending"),request("other-pending","other","pending"),request("closed","current","cancelled"),request("applied","current","applied"),request("retry","current","answered")]} taskId={task} activeId={active} ready={ready} onRefresh={()=>{}} onOpen={()=>{}} onCancel={async()=>{}} onRetry={async()=>{}} onInspect={()=>{}}/>);
it("keeps other task requests and closed history out of the active help region",()=>{
  const html=render("current");const history=html.indexOf("<details");
  expect(html.slice(0,history)).toContain("current-pending");
  expect(html.slice(0,history)).toContain("retry");
  expect(html.slice(0,history)).not.toContain("other-pending");
  expect(html.slice(0,history)).not.toContain(">closed<");
  expect(html.slice(history)).toContain("other-pending");
  expect(html.slice(history)).toContain("Open requested result");
  expect(html).not.toContain("<details open");
});
it("keeps the explicitly selected completed request visible without accepting foreign ownership",()=>{
  expect(render("current","applied").split("<details")[0]).toContain(">applied<");
  expect(render("current","other-pending").split("<details")[0]).not.toContain("other-pending");
  expect(render(undefined).split("<details")[0]).not.toContain("current-pending");
});
it("does not show stale actionable requests while their owner is loading",()=>{
  const html=render("current",undefined,false);
  expect(html).toContain("Loading saved requests");
  expect(html).not.toContain("Cancel request");
});
