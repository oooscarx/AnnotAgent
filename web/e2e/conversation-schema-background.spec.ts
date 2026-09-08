import {randomUUID} from "node:crypto";
import {test,expect} from "./fixtures";

test("Schema survives an aborted browser request while explicit cancellation still stops its worker",async({page,request})=>{
  test.setTimeout(90_000);
  const provider=await (await request.post("/api/providers",{data:{display_name:"TEST background Schema",adapter:"open_ai_compatible",base_url:"http://127.0.0.1:8796/openai/v1"}})).json();
  expect((await request.post(`/api/providers/${provider.id}/credential`,{data:{source:"workspace_file",secret:"TEST-background-schema-only"}})).ok()).toBe(true);
  const model=await (await request.post("/api/model-profiles",{data:{provider_id:provider.id,display_name:"TEST slow Schema",remote_model_id:"e2e-conversation-classification-schema-background",input_modalities:["text"],task_capabilities:["text_generation"],protocol_features:{tool_calls:true,structured_output:true}}})).json();
  expect((await request.post(`/api/providers/${provider.id}/active-probe`,{data:{model_profile_id:model.id,confirmed_billable:true}})).ok()).toBe(true);
  const project=`TEST-schema-background-${Date.now()}`;
  expect((await request.post("/api/projects",{data:{id:project,yaml:"version: 1\nproject:\n  name: TEST Schema background lifecycle\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}})).ok()).toBe(true);
  const conversation=(await (await request.post(`/api/projects/${project}/conversations`)).json()).conversation_id;
  const root=`/api/projects/${project}/conversations/${conversation}`;
  const revision=(await (await request.get(`/api/projects/${project}/goal`)).json()).revision;
  for(const mode of ["leave","stop"]){
    const message=randomUUID(),task=randomUUID();
    expect((await request.post(`${root}/messages`,{data:{id:message,text:"按室内、室外给图片分类",image:null}})).ok()).toBe(true);
    expect((await request.post(`${root}/tasks`,{data:{id:task,source_message_id:message,schema_revision:revision}})).ok()).toBe(true);
    const taskRoot=`${root}/tasks/${task}`;
    const preview=await (await request.get(`${taskRoot}/schema-preview?model_id=${model.id}`)).json();
    const consent={call_id:randomUUID(),model_id:model.id,scope_hash:preview.scope_hash,expires_at:preview.expires_at,allow_unknown_cost:true};
    await page.goto(`/projects/${project}/work?conversation=${conversation}&task=${task}`);
    // Real browser fetch, deliberately aborted while the TEST model is still slow.
    // Unlike route.fetch + dropped acknowledgement this closes an in-flight request.
    await page.evaluate(async({path,consent})=>{
      const session=await (await fetch("/api/session")).json();
      const state=window as typeof window & {testSchemaAbort?:AbortController;testSchemaOutcome?:string};
      state.testSchemaAbort=new AbortController();
      void fetch(path,{method:"POST",headers:{"content-type":"application/json","x-annotagent-csrf":session.csrf_token},body:JSON.stringify(consent),signal:state.testSchemaAbort.signal})
        .then(response=>{state.testSchemaOutcome=`HTTP ${response.status}`;}).catch(error=>{state.testSchemaOutcome=error.name;});
    },{path:`${taskRoot}/schema-proposals`,consent});
    await expect.poll(async()=> (await (await request.get(`${taskRoot}/calls`)).json())[0]?.status).toBe("reserved");
    if(mode==="stop")expect((await request.post(`${taskRoot}/calls/${consent.call_id}/cancel`,{data:{}})).ok()).toBe(true);
    await page.evaluate(()=>{(window as typeof window & {testSchemaAbort:AbortController}).testSchemaAbort.abort();});
    if(mode==="leave")await expect.poll(()=>page.evaluate(()=>(window as typeof window & {testSchemaOutcome?:string}).testSchemaOutcome)).toBe("AbortError");
    await page.goto("/projects");
    await expect.poll(async()=> (await (await request.get(`${taskRoot}/calls`)).json())[0]?.status).toBe(mode==="leave"?"completed":"in_doubt");
    const calls=await (await request.get(`${taskRoot}/calls`)).json();expect(calls).toHaveLength(1);expect(calls[0].id).toBe(consent.call_id);
    if(mode==="stop")expect(calls[0].evidence.error).toContain("Remote completion and cost are unknown");
    const schema=await (await request.get(`${taskRoot}/calls/${consent.call_id}/schema-draft`)).json();
    if(mode==="leave"){expect(schema.revision).toBe(1);expect(schema.definition.task.kind).toBe("classification");}else expect(schema).toBeNull();
    await page.goto(`/projects/${project}/work?conversation=${conversation}&task=${task}`);
    if(mode==="leave")await expect(page.getByText("Schema Draft saved · Revision 1",{exact:true})).toBeVisible();
    else await expect(page.getByText(/Cancellation saved\. No automatic retry/)).toBeVisible();
    expect(await (await request.get(`${taskRoot}/calls`)).json()).toEqual(calls);
  }
});
