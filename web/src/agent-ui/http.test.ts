import { describe, it, expect } from "vitest";
import { HttpAdapter, type Transport } from "./http";

const project = { project_id: "TEST-alpha", project_owner_id: "owner-a", title: "TEST 相同名字", conversation_id: "conversation-a" };
const settings = { revision: "revision-1", sections: { data_privacy: { workspace_id: "TEST-workspace" }, usage_budget: { future_run_budget: { max_requests: 10, max_cost: "2.50" } } } };
const navTask = (id: string) => ({ task_id: id, title: `TEST ${id}`, schema_revision: "schema-1", project_owner_id: "owner-a", conversation_id: "conversation-a", state: "idle" });
const root = "/api/projects/TEST-alpha/conversations/conversation-a/tasks";
it("loads before/after from the exact repair request, not an unrelated older sample",async()=>{
  const record=(id:string)=>({sample_test:{id,project_id:project.project_id,draft_id:id+"-draft",draft_revision:1,inputs:[],report:{samples:[]}}});
  const {transport,paths}=mockTransport({
    [`${root}/t1/workspace`]:{project_id:project.project_id,project_owner_id:project.project_owner_id,conversation_id:project.conversation_id,task:{input:{id:"t1",schema_revision:"schema-1"}},agent_model:{revision:0,model_profile_id:null},actions:{},queue:[],calls:[],human_requests:[{input:{id:"repair",image_id:"image-uuid",sample_test_id:"source"},status:"applied"}],sample_operations:[{id:"source",draft_id:"source-draft",status:"succeeded",created_at:"2026-09-08"},{id:"unrelated",draft_id:"unrelated-draft",status:"succeeded",created_at:"2026-09-09"},{id:"new",draft_id:"new-draft",status:"succeeded",created_at:"2026-09-10"}],journey_consents:[{record:{consent:{id:"journey",sample_operation_id:"new",repair:{request_id:"repair"}}}}]},
    "/api/workflow-drafts/new-draft/sample-test?test_id=new":record("new"),
    "/api/workflow-drafts/source-draft/sample-test?test_id=source":record("source"),
  });
  const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask(project.project_id,"t1");
  expect(adapter.snapshot().tasks.find(t=>t.id==="t1")?.beforeRepair?.sample).toBe("source");
  expect(paths.some(p=>p.includes("unrelated-draft"))).toBe(false);
});
it("shows the latest successful sample instead of an older pending review after repair",async()=>{
  const {transport,paths}=mockTransport({
    [`${root}/t1/workspace`]:{project_id:project.project_id,project_owner_id:project.project_owner_id,conversation_id:project.conversation_id,task:{input:{id:"t1",schema_revision:"schema-1"}},agent_model:{revision:0,model_profile_id:null},actions:{},queue:[],calls:[],human_requests:[{input:{id:"old-review",image_id:"image-uuid",sample_test_id:"old"},status:"pending"}],sample_operations:[{id:"old",draft_id:"old-draft",status:"succeeded",created_at:"2026-09-09"},{id:"new",draft_id:"new-draft",status:"succeeded",created_at:"2026-09-10"}]},
    "/api/workflow-drafts/new-draft/sample-test?test_id=new":{sample_test:{id:"new",project_id:project.project_id,draft_id:"new-draft",draft_revision:2,inputs:[],report:{samples:[]}}},
  });
  const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask(project.project_id,"t1");
  const task=adapter.snapshot().tasks.find(t=>t.id==="t1")!;
  expect(task.sample?.id).toBe("new");expect(task.human).toBeUndefined();
  expect(paths).not.toContain("/api/workflow-drafts/old-draft/sample-test?test_id=old");
});
it("saves an issue-only answer without asserting corrected geometry or invoking a model",async()=>{
  const human={input:{id:"review",sample_test_id:"sample",image_id:"image-uuid",expected_feedback_sequence:0,outcome_id:"candidate"},status:"pending",deferred:false};
  const ws={project_id:project.project_id,project_owner_id:project.project_owner_id,conversation_id:project.conversation_id,task:{input:{id:"t1",schema_revision:"schema-1"}},agent_model:{revision:0,model_profile_id:null},actions:{},queue:[],calls:[],human_requests:[human],sample_operations:[{id:"sample",draft_id:"draft",status:"succeeded"}]};
  const {transport:read}=mockTransport({[`${root}/t1/workspace`]:ws,"/api/workflow-drafts/draft/sample-test?test_id=sample":{sample_test:{id:"sample",project_id:project.project_id,draft_id:"draft",draft_revision:1,inputs:[],report:{samples:[]}}}});
  const writes:{path:string;body:Record<string,unknown>}[]=[];
  const transport:Transport=async<T>(path:string,init?:RequestInit)=>{
    if(init?.method!=="POST")return read<T>(path,init);
    const body=JSON.parse(String(init.body));writes.push({path,body});human.status="applied";
    return {...human,answer:body.answer} as T;
  };
  const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask(project.project_id,"t1");
  const command={id:"answer",project:project.project_id,task:"t1",revision:"schema-1",selection:{image:"image-uuid",candidate:"",revision:"sample:"}};
  await expect(adapter.reportSampleIssue(command,"foreign-image","poor_boundary")).rejects.toThrow("选择或版本");
  await expect(adapter.reportSampleIssue({...command,selection:{...command.selection,revision:"stale"}},"image-uuid","poor_boundary")).rejects.toThrow("选择或版本");
  expect(writes).toHaveLength(0);
  await adapter.reportSampleIssue(command,"image-uuid","poor_boundary");
  expect(writes).toHaveLength(1);expect(writes[0].path).toBe(`${root}/t1/human-requests/review/answer`);
  expect(writes[0].body.answer).toMatchObject({reason:"poor_boundary",corrected_value:null,corrected_label:null,sequence:1});
  expect(adapter.snapshot().tasks.find(t=>t.id==="t1")?.repairRequests?.[0].status).toBe("applied");
});
it.each(["running","settled","budget"])("shows the real journey dispatch %s instead of declaring builder completion as sample success",async(status)=>{
  const {transport}=mockTransport({[`${root}/t1/workspace`]:{
    project_id:project.project_id,project_owner_id:project.project_owner_id,conversation_id:project.conversation_id,
    task:{input:{id:"t1",schema_revision:"schema-1"}},agent_model:{revision:0,model_profile_id:null},actions:{},queue:[],calls:[],
    journey_consents:[{record:{consent:{id:"journey"}},dispatch:{status:status==="budget"?"settled":status,error:status==="settled"?"No inference was started":null},builder:{evidence:{outcome:status==="budget"?"budget_exceeded":undefined}}}],
  }});
  const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask(project.project_id,"t1");
  const task=adapter.snapshot().tasks.find(t=>t.id==="t1")!;
  expect(task.phase).toBe(status==="running"?"running":"idle");
  expect(task.receipts?.at(-1)).toMatchObject({id:"journey",status:status==="running"?"running":"failed"});
});
it("sample consent excludes text-only models, includes ready segmentation and reuses saved schema",async()=>{
  const {transport:base}=mockTransport({
    "/api/model-profiles":{models:[{id:"text",enabled:true,status:"available",input_modalities:["text"],task_capabilities:[],protocol_features:{}},{id:"vision",enabled:true,status:"available",input_modalities:["text","image"],task_capabilities:[],protocol_features:{}}]},
    "/api/model-instances":{instances:[],model_profiles:[{selection_id:"model-instance:ready",selectable:true,availability:"available",capabilities:["prompted_segmentation"]},{selection_id:"model-instance:broken",selectable:false,availability:"unavailable",capabilities:["prompted_segmentation"]}]},
    "/api/projects/TEST-alpha/model-bindings":{bindings:[{model_profile_id:"text"},{model_profile_id:"vision"}]},
    [`${root}/t1/workspace`]:{project_id:project.project_id,project_owner_id:project.project_owner_id,conversation_id:project.conversation_id,task:{input:{id:"t1",schema_revision:"schema-1"}},agent_model:{revision:0,model_profile_id:null},actions:{},queue:[],calls:[{id:"call",status:"completed",evidence:{decision:{Ok:{decision:"draft"}}}}]},
    [`${root}/t1/calls/call/schema-draft`]:{id:"schema",task_id:"t1",revision:3},
  });
  let preview=false,schemaMissing=false;
  const transport:Transport=async<T>(path:string,init?:RequestInit)=>{
    if(schemaMissing && path.endsWith("/schema-draft"))return null as T;
    if(path.includes("/journey-preview?")){
      preview=true;const q=new URL(path,"http://test").searchParams;
      expect(JSON.parse(q.get("allowed_models")!)).toEqual(["model-profile:vision","model-instance:ready"]);
      expect(q.get("schema_id")).toBe("schema");expect(q.get("schema_revision")).toBe("3");expect(q.has("schema_call_id")).toBe(false);
      return {consent:{images:[],maximum_builder_calls:8,maximum_sample_calls:12,builder_scope_hash:"scope"},builder:{},data:{models:[]}} as T;
    }return base<T>(path,init);
  };
  const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask(project.project_id,"t1");
  await adapter.prepareAction({id:"journey",project:project.project_id,task:"t1",revision:"schema-1"},"sample");expect(preview).toBe(true);
  schemaMissing=true;
  await expect(adapter.prepareAction({id:"missing",project:project.project_id,task:"t1",revision:"schema-1"},"sample")).rejects.toThrow("目标草稿尚未保存");
});
it("restores a completed schema decision and prevents repeating initial planning",async()=>{
  const {transport,paths}=mockTransport({[`${root}/t1/workspace`]:{
    project_id:project.project_id,project_owner_id:project.project_owner_id,conversation_id:project.conversation_id,
    task:{input:{id:"t1",schema_revision:"schema-1"}},agent_model:{revision:0,model_profile_id:null},actions:{},queue:[],
    calls:[{id:"completed",status:"completed",evidence:{decision:{Ok:{decision:"draft",rationale:"Saved target"}}}}],
  }});
  const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask(project.project_id,"t1");
  expect(adapter.snapshot().tasks.find(t=>t.id==="t1")).toMatchObject({phase:"idle",schemaProposed:true});
  await expect(adapter.prepareAction({id:"repeat",project:project.project_id,task:"t1",revision:"schema-1"},"plan")).rejects.toThrow("已保存的目标草稿");
  expect(paths.some(p=>p.endsWith("schema-preview"))).toBe(false);
});
it("shows upstream HTTP failures and blocks replacement authorization without poisoning new tasks",async()=>{
  const {transport,paths}=mockTransport({[`${root}/t1/workspace`]:{
    project_id:project.project_id,project_owner_id:project.project_owner_id,conversation_id:project.conversation_id,
    task:{input:{id:"t1",schema_revision:"schema-1"}},agent_model:{revision:0,model_profile_id:null},actions:{},queue:[],
    calls:[{id:"unavailable",status:"in_doubt",failure:{category:"http_status",http_status:503,stage:"provider_request"}}],
  }});
  const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask(project.project_id,"t1");
  expect(adapter.snapshot().tasks.find(t=>t.id==="t1")).toMatchObject({phase:"outcome_unknown",remoteFailure:{httpStatus:503}});
  await expect(adapter.prepareAction({id:"replacement",project:project.project_id,task:"t1",revision:"schema-1"},"plan")).rejects.toThrow("不能重建初始授权");
  expect(paths.some(p=>p.endsWith("schema-preview"))).toBe(false);
  const id=await adapter.createTask(project.project_id);
  adapter.saveDraft(id,"保留的目标");
  expect(adapter.snapshot().tasks.find(t=>t.id===id)).toMatchObject({phase:"idle",draft:"保留的目标"});
  expect(adapter.snapshot().tasks.find(t=>t.id===id)?.remoteFailure).toBeUndefined();
});
it("planning an existing task never overrides its frozen Send model with the next-request preference", async () => {
  const { transport, paths } = mockTransport({
    "/api/agent-model-bindings": {pipeline_builder:"new-preference"},
    [`${root}/t1/schema-preview`]: {model_id:"frozen-send-model",model_name:"Frozen",destination:"TEST",data_scope:"text",image_count:0,maximum_calls:1,scope_hash:"scope",expires_at:"future"},
  });
  const adapter = new HttpAdapter(transport);
  await adapter.refresh(); await adapter.loadTask("TEST-alpha","t1");
  expect(adapter.snapshot().tasks.find(t=>t.id==="t1")?.model).toBe("new-preference");
  await adapter.prepareAction({id:"plan",project:"TEST-alpha",task:"t1",revision:"schema-1"},"plan");
  expect(paths).toContain(`${root}/t1/schema-preview`);
  expect(paths.some(path=>path.includes("model_id=new-preference"))).toBe(false);
  expect(adapter.snapshot().tasks.find(t=>t.id==="t1")?.approval?.scope).toContain("Frozen");
});
function mockTransport(overrides: Record<string, unknown | (() => Promise<unknown>)> = {}) {
  const paths: string[] = [];
  const data: Record<string, unknown | (() => Promise<unknown>)> = {
    "/api/navigation?limit=100": { items: [project], next_cursor: null },
    "/api/settings?view=agent-ui": settings,
    "/api/providers": { providers: [] }, "/api/model-profiles": { models: [] },
    "/api/agent-model-bindings": {}, "/api/plugins": { installations: [] }, "/api/model-instances": { instances: [] },
    "/api/projects/TEST-alpha/conversations/conversation-a/task-navigation?limit=100": { items: [navTask("t1"), navTask("t2")], next_cursor: null },
    "/api/projects/TEST-alpha/images": { images: [{ image_id: "image-uuid", name: "TEST.png", url: "/api/projects/TEST-alpha/images/image-uuid/file" }] },
    ...Object.fromEntries(["t1", "t2"].flatMap(id => [
      [`${root}/${id}/workspace`, { project_id: "TEST-alpha", project_owner_id: "owner-a", conversation_id: "conversation-a", task: { input: { id, schema_revision: "schema-1" } }, agent_model: { revision: 2, model_profile_id: null }, actions: { resume: { available: false, reason: "No checkpoint" } }, queue: [], calls: [] }],
      [`${root}/${id}/thread?limit=100`, { items: [{ id: `message-${id}`, task_id: id, project_owner_id: "owner-a", conversation_id: "conversation-a", role: "user", message: { input: { text: `真实已存 ${id}` } } }], next_cursor: null }],
      [`${root}/${id}/exports`, []],
    ])), ...overrides,
  };
  const transport: Transport = async <T>(path: string, init?: RequestInit) => {
    expect(init?.method || "GET").toBe("GET"); paths.push(path);
    if (!(path in data)) throw new Error(`Unexpected ${path}`);
    return (typeof data[path] === "function" ? await (data[path] as () => Promise<unknown>)() : structuredClone(data[path])) as T;
  };
  return { transport, paths };
}
describe("HTTP UI read boundary (synthetic transport tests, not HTTP E2E)", () => {
  it("preserves server timing and safe errors without an old unknown call masking active work", async () => {
    const {transport}=mockTransport({[`${root}/t1/workspace`]:{
      project_id:project.project_id,project_owner_id:project.project_owner_id,conversation_id:project.conversation_id,
      task:{input:{id:"t1",schema_revision:"schema-1"}},agent_model:{revision:0,model_profile_id:null},actions:{},queue:[],
      calls:[{id:"old",status:"in_doubt",started_at:"2026-09-09T00:00:00Z",completed_at:"2026-09-09T00:00:02Z",duration_ms:2000,stage:"settled",failure:{category:"timeout",stage:"response_body"}},
        {id:"active",status:"reserved",started_at:"2026-09-09T00:01:00Z",stage:"provider_request"}],
    }});
    const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask(project.project_id,"t1");
    const task=adapter.snapshot().tasks.find(t=>t.id==="t1")!;
    expect(task.phase).toBe("running");
    expect(task.receipts?.[0]).toMatchObject({durationMs:2000,detail:"请求超时 · 读取响应",finishedAt:"2026-09-09T00:00:02Z"});
    expect(task.receipts?.[1]).toMatchObject({stage:"模型请求处理中",startedAt:"2026-09-09T00:01:00Z"});
  });
  it("reads actual identity, thread and UUID images without POST, assistant fabrication or resume inference", async () => {
    const { transport, paths } = mockTransport(); const adapter = new HttpAdapter(transport);
    await adapter.refresh(); await adapter.loadTask("TEST-alpha", "t1");
    expect(adapter.snapshot().tasks.find(t => t.id === "t1")?.items).toEqual([{ id: "message-t1", role: "user", text: "真实已存 t1" }]);
    expect(adapter.snapshot().artifacts[0].id).toBe("image-uuid");
    expect(adapter.snapshot().settings.budget).toBe("2.50");
    expect(adapter.snapshot().usage).toEqual([]);
    expect(paths.some(p => p.includes("proposals"))).toBe(false);
  });
  it("does not turn transport failure into fixture success", async () => {
    const adapter = new HttpAdapter(async () => { throw new Error("503 TEST unavailable"); });
    await expect(adapter.refresh()).rejects.toThrow("503");
    expect(adapter.snapshot().projects).toEqual([]);
    expect(adapter.snapshot().error).toContain("503");
    expect(adapter.kind).toBe("http");
  });
  it("rejects foreign task snapshots and never selects a fallback", async () => {
    const { transport } = mockTransport({ [`${root}/t1/workspace`]: { project_id: "foreign", task: { input: { id: "t1" } } } });
    const adapter = new HttpAdapter(transport); await adapter.refresh();
    await expect(adapter.loadTask("TEST-alpha", "t1")).rejects.toThrow("归属");
    expect(adapter.snapshot().artifacts).toEqual([]);
    await expect(adapter.loadTask("foreign", "t1")).rejects.toThrow("不属于");
  });
  it("ignores an older slow task response even if transport ignores AbortSignal", async () => {
    let resolve!: (value: unknown) => void;
    const slow = new Promise(r => { resolve = r; });
    const { transport } = mockTransport({ [`${root}/t1/thread?limit=100`]: () => slow });
    const adapter = new HttpAdapter(transport); await adapter.refresh();
    const first = adapter.loadTask("TEST-alpha", "t1"); await adapter.loadTask("TEST-alpha", "t2");
    resolve({ items: [], next_cursor: null }); await first;
    expect(adapter.snapshot().tasks.find(t => t.id === "t2")?.items[0].text).toBe("真实已存 t2");
    expect(adapter.snapshot().tasks.find(t => t.id === "t1")?.items).toEqual([]);
  });
  it("follows navigation pages and fails closed on a repeated cursor", async () => {
    const { transport } = mockTransport({ "/api/navigation?limit=100": { items: [project], next_cursor: "owner-a" }, "/api/navigation?limit=100&cursor=owner-a": { items: [], next_cursor: "owner-a" } });
    await expect(new HttpAdapter(transport).refresh()).rejects.toThrow("游标重复");
  });
  it("preserves typing while a task's late evidence read settles",async()=>{
    let finish!:(v:unknown)=>void;
    const pending=new Promise(r=>{finish=r;});
    const {transport,paths}=mockTransport({[`${root}/t1/exports`]:()=>pending});
    const adapter=new HttpAdapter(transport);await adapter.refresh();
    const reading=adapter.loadTask("TEST-alpha","t1");
    for(let i=0;i<30&&!paths.includes(`${root}/t1/exports`);i++)await Promise.resolve();
    expect(paths).toContain(`${root}/t1/exports`);
    adapter.saveDraft("t1","输入不能被旧请求清空");finish([]);await reading;
    expect(adapter.snapshot().tasks.find(t=>t.id==="t1")?.draft).toBe("输入不能被旧请求清空");
  });
  it("does not persist an empty canvas before owned task evidence has loaded",async()=>{
    const {transport}=mockTransport();const adapter=new HttpAdapter(transport);await adapter.refresh();
    adapter.saveArtifactDraft("t1","image-uuid",[]);
    expect(adapter.snapshot().tasks.find(t=>t.id==="t1")?.editBoxes).toBeUndefined();
    await adapter.loadTask("TEST-alpha","t1");
    expect(adapter.snapshot().tasks.find(t=>t.id==="t1")?.loaded).toBe(true);
    adapter.saveArtifactDraft("t1","image-uuid",[]);
    expect(adapter.snapshot().tasks.find(t=>t.id==="t1")?.editBoxes).toEqual({});
  });
});
