import { describe, it, expect } from "vitest";
import { HttpAdapter, type Transport } from "./http";

const project = { project_id: "TEST-alpha", project_owner_id: "owner-a", title: "TEST 相同名字", conversation_id: "conversation-a" };
const settings = { revision: "revision-1", sections: { data_privacy: { workspace_id: "TEST-workspace" }, usage_budget: { future_run_budget: { max_requests: 10, max_cost: "2.50" } } } };
const navTask = (id: string) => ({ task_id: id, title: `TEST ${id}`, schema_revision: "schema-1", project_owner_id: "owner-a", conversation_id: "conversation-a", state: "idle" });
const root = "/api/projects/TEST-alpha/conversations/conversation-a/tasks";
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
  it("keeps original canvas resources separate from controlled thumbnail resources",async()=>{
    const {transport}=mockTransport({"/api/projects/TEST-alpha/images":{images:[{image_id:"i",name:"image",url:"/api/images/i/file",thumbnail_url:"/api/images/i/thumbnail"}]}});
    const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");expect(adapter.snapshot().artifacts[0]).toMatchObject({src:"/api/images/i/file",thumbnail:"/api/images/i/thumbnail"});
  });
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
