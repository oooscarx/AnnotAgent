import {expect,it} from "vitest";
import {renderToStaticMarkup} from "react-dom/server";
import {ConversationHumanRequests} from "./ConversationHumanRequests";
import type {HumanRequest} from "../conversation-human-api";

const request=(id:string,task:string,status:HumanRequest["status"]):HumanRequest=>({input:{id,task_id:task,conversation_id:"conversation",sample_test_id:"sample",image_id:"image",content_hash:"hash",outcome_id:"outcome",expected_feedback_sequence:0,reason_code:"boundary",question:id,resume_checkpoint_ref:"checkpoint"},status});
const render=(task?:string,active?:string,ready=true)=>renderToStaticMarkup(<ConversationHumanRequests requests={[request("current-pending","current","pending"),request("other-pending","other","pending"),request("closed","current","cancelled"),request("applied","current","applied"),request("retry","current","answered")]} taskId={task} activeId={active} ready={ready} onRefresh={()=>{}} onOpen={()=>{}} onCancel={async()=>{}} onRetry={async()=>{}} onInspect={()=>{}}/>);
it("allows an explicit retry after a context read failure without claiming an empty queue",()=>{
  const html=renderToStaticMarkup(<ConversationHumanRequests requests={[]} taskId="current" ready={false} loadError="TEST read failed" onRefresh={()=>{}} onOpen={()=>{}} onCancel={async()=>{}} onRetry={async()=>{}} onInspect={()=>{}}/>);
  expect(html).toContain("TEST read failed");
  expect(html).not.toContain("No outstanding visual requests");
  expect(html).not.toContain("disabled=");
});
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
it("keeps deferred work unfinished and provides an explicit reopen action",()=>{
  const deferred={...request("later","current","pending"),deferred:true,deferral_revision:1};
  const html=renderToStaticMarkup(<ConversationHumanRequests requests={[deferred]} taskId="current" ready onRefresh={()=>{}} onOpen={()=>{}} onCancel={async()=>{}} onRetry={async()=>{}} onDefer={async()=>{}} onInspect={()=>{}}/>);
  expect(html).toContain("Deferred · not reviewed or completed");
  expect(html).toContain("1 deferred requests remain unfinished");
  expect(html).toContain("Reopen request");
  expect(html).not.toContain("No outstanding visual requests");
  expect(html).not.toContain("Retry Draft preparation");
});

const actions={onRefresh:()=>{},onOpen:()=>{},onCancel:async()=>{},onRetry:async()=>{},onInspect:()=>{}};
it("only compacts an actually loaded empty request list",()=>{
  const empty=renderToStaticMarkup(<ConversationHumanRequests {...actions} requests={[]} ready taskId="TEST-task"/>);
  expect(empty).toContain('data-empty="true"');
  expect(empty).toContain("Refresh requests");
  expect(empty).toContain("No outstanding visual requests for this goal.");
  const failed=renderToStaticMarkup(<ConversationHumanRequests {...actions} requests={[]} ready={false} loadError="TEST read failed"/>);
  expect(failed).toContain('data-empty="false"');
  expect(failed).toContain('role="alert"');
  expect(failed).toContain("TEST read failed");
  expect(failed).not.toContain("No outstanding");
});
it("keeps real pending requests and other-goal history out of the empty treatment",()=>{
  const request={input:{id:"TEST-request",task_id:"TEST-task",question:"TEST correct this boundary"},status:"pending"} as HumanRequest;
  for(const taskId of ["TEST-task","TEST-other"]){
    const html=renderToStaticMarkup(<ConversationHumanRequests {...actions} requests={[request]} ready taskId={taskId}/>);
    expect(html).toContain('data-empty="false"');
    expect(html).toContain("TEST correct this boundary");
    expect(html).toContain("Open requested result");
    expect(html).toContain("Cancel request");
  }
});
