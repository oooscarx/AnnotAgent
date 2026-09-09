import { describe, it, expect } from "vitest";
import { HttpAdapter, type Transport } from "./http";

const project = { project_id: "TEST-alpha", project_owner_id: "owner-a", title: "TEST 相同名字", conversation_id: "conversation-a" };
const settings = { revision: "revision-1", sections: { data_privacy: { workspace_id: "TEST-workspace" }, usage_budget: { future_run_budget: { max_requests: 10, max_cost: "2.50" } } } };
const navTask = (id: string) => ({ task_id: id, title: `TEST ${id}`, schema_revision: "schema-1", project_owner_id: "owner-a", conversation_id: "conversation-a", state: "idle" });
const root = "/api/projects/TEST-alpha/conversations/conversation-a/tasks";
it("delivery labels display names without rewriting IDs and ignore older name revisions",async()=>{
  const reads=mockTransport();let revision=2;
  const transport:Transport=async<T>(path:string,init?:RequestInit)=>path.endsWith("/delivery-intent")?{saved:{revision,intent:{label_spec:[{stable_id:"stable-label",display_name:revision===2?"足球":"旧名称"}]}},missing_slots:[],blockers:[]} as T:reads.transport<T>(path,init);
  const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");
  const before=adapter.snapshot().tasks.find(t=>t.id==="t1")!.boxes;
  await adapter.deliveryIntake.read("TEST-alpha","t1");
  expect(adapter.snapshot().tasks.find(t=>t.id==="t1")!.labelNames).toEqual({"stable-label":"足球"});
  revision=1;await adapter.deliveryIntake.read("TEST-alpha","t1");
  const after=adapter.snapshot().tasks.find(t=>t.id==="t1")!;
  expect(after.labelNames).toEqual({"stable-label":"足球"});expect(after.boxes).toEqual(before);
});
function memoryStorage():Storage {
  const values=new Map<string,string>();
  return {get length(){return values.size;},clear:()=>values.clear(),getItem:key=>values.get(key)??null,setItem:(key,value)=>{values.set(key,String(value));},removeItem:key=>{values.delete(key);},key:index=>[...values.keys()][index]??null};
}
it("pending package admission survives a new adapter and only explicit retry reuses the original command",async()=>{
  const storage=memoryStorage();const reads=mockTransport();const posts:unknown[]=[];let received=false;
  const input={command_id:"uncertain",intent_revision:3,intent_sha256:"saved-scope",image_reviews:{one:2},confirmed:true};
  const transport:Transport=async<T>(path:string,init?:RequestInit)=>{
    if(path.endsWith("/delivery-packages")&&init?.method==="POST"){
      posts.push(JSON.parse(String(init.body)));if(!received)throw new Error("TEST connection lost");
      return {job:{id:"uncertain",phase:"exporting"},active:true,dispatched:false} as T;
    }
    return reads.transport<T>(path,init);
  };
  const first=new HttpAdapter(transport,storage);await first.refresh();await first.loadTask("TEST-alpha","t1");
  await expect(first.delivery.startPackage("TEST-alpha","t1",input)).rejects.toThrow("connection lost");
  const restored=new HttpAdapter(transport,storage);await restored.refresh();await restored.loadTask("TEST-alpha","t1");
  expect(posts).toHaveLength(1);
  expect(restored.delivery.pendingPackage("TEST-alpha","t1")).toEqual(input);
  expect(restored.delivery.pendingPackage("TEST-alpha","t2")).toBeUndefined();
  await expect(restored.delivery.startPackage("TEST-alpha","t1",{...input,command_id:"replacement"})).rejects.toThrow("待核实");
  expect(posts).toHaveLength(1);
  received=true;await restored.delivery.startPackage("TEST-alpha","t1",restored.delivery.pendingPackage("TEST-alpha","t1")!);
  expect(posts).toEqual([input,input]);expect(restored.delivery.pendingPackage("TEST-alpha","t1")).toBeUndefined();
});
it("training package history restores without interpreting its receipt as a legacy export report",async()=>{
  const packageRow={id:"package",format:"ultralytics_yolo_detection",created_at:"TEST",result:{images:11,objects:20,sha256:"frozen",bytes:500}};
  const reads=mockTransport({[`${root}/t1/exports`]:[packageRow],[`${root}/t1/exports?limit=100`]:[packageRow]});
  const adapter=new HttpAdapter(reads.transport);await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");
  expect(adapter.snapshot().error).toBeUndefined();
  expect(adapter.snapshot().tasks.find(t=>t.id==="t1")?.exports).toEqual([]);
  expect(await adapter.delivery.history("TEST-alpha","t1")).toEqual({items:[{id:"package",created_at:"TEST"}],next_cursor:null});
});
it("formal delivery reads never dispatch and commands retain frozen scope across retries", async () => {
  const reads = mockTransport();
  const calls: {path:string; method:string; body:unknown; signal?:AbortSignal|null}[] = [];
  let failure = false;
  const transport:Transport = async<T>(path:string, init?:RequestInit) => {
    if (!path.includes("/delivery-images/") && !path.includes("/delivery-packages")) return reads.transport<T>(path,init);
    calls.push({path,method:init?.method || "GET",body:init?.body ? JSON.parse(String(init.body)) : null,signal:init?.signal});
    if (failure) throw new Error("TEST stale whole-image snapshot");
    return {job:{id:"package",phase:"validating"},active:false,interrupted:true} as T;
  };
  const adapter = new HttpAdapter(transport,memoryStorage());
  await adapter.refresh(); await adapter.loadTask("TEST-alpha","t1");
  expect(calls).toEqual([]);
  const ctrl = new AbortController();
  await adapter.delivery.image("TEST-alpha","t1","image/one","run/one",ctrl.signal);
  await adapter.delivery.image("TEST-alpha","t1","image/one",null);
  const status = await adapter.delivery.packageStatus("TEST-alpha","t1","package",ctrl.signal);
  expect(status.interrupted).toBe(true);
  expect(calls.every(c=>c.method==="GET")).toBe(true);
  expect(calls[0].path).toBe(`${root}/t1/delivery-images/image%2Fone?source_run_id=run%2Fone`);
  expect(calls[0].signal).toBe(ctrl.signal);
  expect(calls[1].path).not.toContain("source_run_id");
  const input = {command_id:"package",intent_revision:7,intent_sha256:"frozen",image_reviews:{"image/one":4},confirmed:true};
  await adapter.delivery.startPackage("TEST-alpha","t1",input);
  await adapter.delivery.startPackage("TEST-alpha","t1",input);
  expect(calls[3]).toEqual(calls[4]); expect(calls[3].body).toEqual(input);
  const confirmation = {command_id:"confirm",intent_revision:7,intent_sha256:"frozen",image_id:"image/one",source_run_id:"run/one",expected_snapshot_sha256:"snapshot",expected_review_revision:4,decision:"positive_complete" as const,reason:null,confirmed:true};
  await adapter.delivery.confirmImage("TEST-alpha","t1",confirmation);
  expect(calls.at(-1)?.body).toEqual(confirmation);
  await adapter.delivery.cancelPackage("TEST-alpha","t1","package");
  expect(calls.at(-1)?.body).toEqual({confirmed:true});
  const count = calls.length;
  expect(adapter.delivery.downloadUrl("TEST-alpha","t1","package")).toBe(`${root}/t1/delivery-packages/package/download`);
  expect(calls).toHaveLength(count);
  expect(()=>adapter.delivery.downloadUrl("OTHER","t1","package")).toThrow("任务不属于");
  expect(calls).toHaveLength(count);
  failure = true;
  await expect(adapter.delivery.startPackage("TEST-alpha","t1",input)).rejects.toThrow("stale whole-image snapshot");
  await expect(adapter.delivery.confirmImage("TEST-alpha","t1",confirmation)).rejects.toThrow("stale whole-image snapshot");
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
      [`${root}/${id}/delivery-schema`, {required:false,schema:null}],
    ])), ...overrides,
  };
  const transport: Transport = async <T>(path: string, init?: RequestInit) => {
    expect(init?.method || "GET").toBe("GET"); paths.push(path);
    if (!(path in data)) throw new Error(`Unexpected ${path}`);
    return (typeof data[path] === "function" ? await (data[path] as () => Promise<unknown>)() : structuredClone(data[path])) as T;
  };
  return { transport, paths };
}
it("delivery preparation posts the exact owned revision and never falls back to fixture success",async()=>{
  const reads=mockTransport();const posts:{path:string;body:unknown}[]=[];let reject=false;
  const transport:Transport=async<T>(path:string,init?:RequestInit)=>{
    if(init?.method==="POST"){
      posts.push({path,body:JSON.parse(String(init.body))});
      if(reject)throw new Error("TEST delivery revision changed");
      return {id:"schema-delivery",revision:1} as T;
    }
    return reads.transport<T>(path,init);
  };
  const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");
  expect(posts).toEqual([]);
  const input={command_id:"same-command",expected_revision:3,expected_sha256:"frozen-delivery"};
  expect(await adapter.deliveryIntake.prepare!("TEST-alpha","t1",input)).toEqual({id:"schema-delivery",revision:1});
  expect(posts).toEqual([{path:`${root}/t1/delivery-schema`,body:input}]);
  reject=true;
  await expect(adapter.deliveryIntake.prepare!("TEST-alpha","t1",input)).rejects.toThrow("revision changed");
  await expect(adapter.deliveryIntake.prepare!("OTHER","t1",input)).rejects.toThrow("任务不属于");
  expect(posts).toHaveLength(2);
  expect(posts[1]).toEqual(posts[0]);
});
it("sample consent uses the saved delivery Schema instead of another Schema model call",async()=>{
  const reads=mockTransport({
    "/api/projects/TEST-alpha/model-bindings":{bindings:[{model_profile_id:"vision"}]},
    "/api/model-instances":{instances:[],model_profiles:[{selectable:true,capabilities:["prompted_segmentation"],selection_id:"model-instance:ready-local"},{selectable:false,capabilities:["prompted_segmentation"],selection_id:"model-instance:not-ready"},{selectable:true,capabilities:["object_detection"],selection_id:"model-instance:unrelated"}]},
    [`${root}/t1/delivery-schema`]:{required:true,schema:{id:"delivery-schema",revision:4}},
  });
  let preview="";
  const transport:Transport=async<T>(path:string,init?:RequestInit)=>{
    if(path.includes("journey-preview?")){preview=path;throw new Error("TEST observed exact scope");}
    return reads.transport<T>(path,init);
  };
  const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");
  await expect(adapter.prepareAction({id:"sample",project:"TEST-alpha",task:"t1",revision:"schema-1"},"sample")).rejects.toThrow("TEST observed");
  const query=new URL(preview,"http://TEST.local").searchParams;
  expect(query.get("schema_id")).toBe("delivery-schema");expect(query.get("schema_revision")).toBe("4");
  expect(query.has("schema_call_id")).toBe(false);
  expect(JSON.parse(query.get("allowed_models")!)).toEqual(["model-profile:vision","model-instance:ready-local"]);
  expect(reads.paths.some(p=>p.includes("schema-preview"))).toBe(false);
});
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
