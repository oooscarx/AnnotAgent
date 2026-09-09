import { test, expect, protectedRequestContext } from "./fixtures";
import { sample } from "./conversation-feedback-helpers";
import { randomUUID } from "node:crypto";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, join, dirname } from "node:path";
import { createServer } from "node:net";
import { spawn, execFile, type ChildProcess } from "node:child_process";
import { promisify } from "node:util";
import { isolatedEvidencePath } from "./evidence";

async function freePort() {
  const socket=createServer();
  await new Promise<void>(done=>socket.listen(0,"127.0.0.1",done));
  const port=(socket.address() as {port:number}).port;
  await new Promise<void>((done,reject)=>socket.close(error=>error?reject(error):done()));
  return port;
}
async function stop(child?:ChildProcess, signal:NodeJS.Signals="SIGTERM") {
  if(!child||child.exitCode!==null||child.signalCode!==null)return;
  await new Promise<void>((done,reject)=>{
    const timer=setTimeout(()=>reject(new Error("Owned test process did not exit")),10_000);
    child.once("exit",()=>{clearTimeout(timer);done();});
    child.kill(signal);
  });
}

test("a killed answer writer recovers through real server startup without replaying completed work",async({browser,playwright})=>{
  test.setTimeout(240_000);
  const workspace=mkdtempSync(join(tmpdir(),"TEST-answer-restart-"));
  const port=await freePort(), baseURL=`http://127.0.0.1:${port}`;
  const root=resolve("..");
  const compilation=await promisify(execFile)("cargo",["test","-p","annotagent-server","--lib","--all-features","--no-run","--message-format=json"],{cwd:root,maxBuffer:16*1024*1024});
  const artifacts=compilation.stdout.split("\n").filter(line=>line.startsWith("{")).map(line=>JSON.parse(line));
  const executable=artifacts.find(item=>item.reason==="compiler-artifact"&&item.target?.name==="annotagent_server"&&item.profile?.test&&item.executable)?.executable;
  expect(executable).toBeTruthy();
  let server:ChildProcess|undefined, writer:ChildProcess|undefined;
  let log="";
  const raw=await playwright.request.newContext({baseURL});
  const start=async()=>{
    server=spawn(resolve(root,"target/debug/annotagent"),["serve","--workspace",workspace,"--port",String(port)],{cwd:root,stdio:["ignore","pipe","pipe"]});
    server.stdout?.on("data",chunk=>{log=(log+chunk).slice(-4000);});
    server.stderr?.on("data",chunk=>{log=(log+chunk).slice(-4000);});
    await expect.poll(async()=>{
      if(server?.exitCode!==null)throw new Error(`Owned server exited: ${log}`);
      return raw.get("/api/health").then(value=>value.status()).catch(()=>0);
    },{timeout:30_000}).toBe(200);
  };
  const context=await browser.newContext({baseURL});
  const page=await context.newPage();
  try{
    await start();
    const request=protectedRequestContext(raw);
    const state=await sample(request,page,"process-restart",true);
    const help=state.savedRequests[0];
    const original=await (await request.get(`/api/workflow-drafts/${state.record.draft_id}/sample-test?test_id=${state.record.id}`)).json();
    const query=new URLSearchParams({consent_id:randomUUID(),builder_operation_id:randomUUID(),sample_operation_id:randomUUID(),schema_id:original.annotation_schema.schema_draft_id,schema_revision:String(original.annotation_schema.revision),planner_model_id:state.model.id,pending_request_id:help.input.id,allowed_models:JSON.stringify([`model-profile:${state.model.id}`])});
    const preview=await request.get(`${state.taskRoot}/journey-preview?${query}`);
    expect(preview.ok(),await preview.text()).toBe(true);
    const consent={...(await preview.json()).consent,allow_unknown_cost:true};
    expect((await request.post(`${state.taskRoot}/journey-consents`,{data:consent})).ok()).toBe(true);
    let answer:any;
    await page.route(`**${state.taskRoot}/human-requests/${help.input.id}/answer`,async route=>{
      answer=route.request().postDataJSON().answer;
      await route.abort("failed");
    });
    await page.goto(`${state.url}&request=${help.input.id}`);
    await page.getByLabel("Correct label",{exact:true}).fill("cup");
    await page.getByRole("spinbutton",{name:"width",exact:true}).fill("0.12");
    await page.getByRole("button",{name:/^Submit correction( and continue)?$/}).click();
    await expect.poll(()=>Boolean(answer)).toBe(true);
    const before=await (await request.get(`${state.taskRoot}/calls`)).json();
    const beforeBudget=await (await request.get(`${state.taskRoot}/budget`)).json();
    expect((await (await request.get(`${state.taskRoot}/human-requests`)).json()).find((item:any)=>item.input.id===help.input.id).status).toBe("pending");
    await stop(server);server=undefined;
    writer=spawn(executable,["--exact","tests::answer_checkpoint_process","--ignored","--nocapture"],{
      cwd:root,stdio:["ignore","pipe","pipe"],env:{...process.env,ANNOTAGENT_TEST_ANSWER_CHECKPOINT:JSON.stringify({workspace,project:state.project,conversation:state.conversation,task:state.task,request:help.input.id,consent:consent.id,answer})},
    });
    let writerLog="";
    writer.stdout?.on("data",chunk=>{writerLog+=chunk;});writer.stderr?.on("data",chunk=>{writerLog+=chunk;});
    await expect.poll(()=>{
      if(writer?.exitCode!==null)throw new Error(`Checkpoint writer exited: ${writerLog}`);
      return writerLog.includes("TEST_ANSWER_COMMITTED_BEFORE_DISPATCH");
    },{timeout:30_000}).toBe(true);
    await stop(writer,"SIGKILL");
    expect(writer.signalCode).toBe("SIGKILL");writer=undefined;
    await start();
    const execution=`${state.taskRoot}/journey-consents/${consent.id}/execution`;
    await expect.poll(async()=> (await (await raw.get(execution)).json()).sample?.status,{timeout:75_000}).toBe("succeeded");
    const recovered=await (await raw.get(execution)).json();
    expect(recovered.answer_delivery.status).toBe("dispatched");
    expect(recovered.record.resolved_consent.repair.request_id).toBe(help.input.id);
    expect(recovered.sample.id).toBe(consent.sample_operation_id);
    expect(recovered.sample.draft_id).toBe(help.input.resume_checkpoint_ref);
    const after=await (await raw.get(`${state.taskRoot}/calls`)).json();
    const afterBudget=await (await raw.get(`${state.taskRoot}/budget`)).json();
    expect(after.length).toBeGreaterThan(before.length);
    expect(afterBudget.total_reserved_calls).toBeGreaterThan(beforeBudget.total_reserved_calls);
    const revisions=await (await raw.get(`/api/workflow-sample-tests/${state.record.id}/images/${help.input.image_id}/feedback`)).json();
    expect(revisions.revisions).toHaveLength(1);
    expect(revisions.revisions[0].revision_id).toBe(answer.revision_id);
    await stop(server);server=undefined;
    await start();
    expect((await (await raw.get(execution)).json()).sample.id).toBe(consent.sample_operation_id);
    expect(await (await raw.get(`${state.taskRoot}/calls`)).json()).toEqual(after);
    expect(await (await raw.get(`${state.taskRoot}/budget`)).json()).toEqual(afterBudget);
    const evidencePath=isolatedEvidencePath("../docs/execution/conversational-workspace/answer-restart-evidence.json");
    mkdirSync(dirname(evidencePath),{recursive:true});
    writeFileSync(evidencePath,JSON.stringify({
      kind:"TEST process restart; not Live quality evidence",workspace,project:state.project,task:state.task,request:help.input.id,
      consent:consent.id,writer_exit_signal:"SIGKILL",checkpoint:"real answer and local repair committed, no journey dispatch",
      recovery_entry:"production serve startup",sample:recovered.sample.id,draft:recovered.sample.draft_id,
      feedback_revision:answer.revision_id,feedback_count:revisions.revisions.length,
      before_budget:beforeBudget,after_budget:afterBudget,completed_restart_budget_unchanged:true,
    },null,2));
  }finally{
    await stop(writer,"SIGKILL");await stop(server);await context.close();await raw.dispose();
  }
});
