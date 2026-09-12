import { describe, it, expect } from "vitest";
import { HttpAdapter, type Transport } from "./http";

const project = { project_id: "TEST-alpha", project_owner_id: "owner-a", title: "TEST 相同名字", conversation_id: "conversation-a" };
const settings = { revision: "revision-1", sections: { data_privacy: { workspace_id: "TEST-workspace" }, usage_budget: { future_run_budget: { max_requests: 10, max_cost: "2.50" } } } };
const navTask = (id: string) => ({ task_id: id, title: `TEST ${id}`, schema_revision: "schema-1", project_owner_id: "owner-a", conversation_id: "conversation-a", state: "idle" });
const root = "/api/projects/TEST-alpha/conversations/conversation-a/tasks";
const mainline=(id:string)=>({contract_version:"mainline-task-v1",project_id:"TEST-alpha",project_owner_id:"owner-a",conversation_id:"conversation-a",task_id:id,read_model_revision:`read-${id}`,delivery:{saved:{revision:3,content_sha256:"frozen-delivery"}},schema:null,review_summary:{selected_images:0,saved_review_receipts:0,current_reviews:0,pending_reviews:0},package:{consents:[],jobs:[]},available_actions:[{id:"prepare_delivery_schema",state:"authorized",method:"POST",url:`${root}/${id}/advance`,requires_confirmation:false,reason:null}],blockers:[],completion:{model_request_completed:false,processing_completed:false,package_ready:false,task_completed:false}});
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
it("reads only task-owned physical usage attempts and preserves the server Decimal strings",async()=>{
  const page={scope:{project_id:"TEST-alpha",conversation_id:"conversation-a",task_id:"t1"},state:"complete" as const,summary:{attempt_count:1,known_cost:"0.007",currency:"USD",input_tokens:1500,cached_input_tokens:0,output_tokens:500,unknown_attempt_count:0},attempts:{items:[],next_cursor:"next/attempt"}};
  const second={...page,attempts:{items:[],next_cursor:null}};
  const reads=mockTransport({
    [`${root}/t1/model-usage?limit=50`]:page,
    [`${root}/t1/model-usage?limit=50&cursor=next%2Fattempt`]:second,
  });
  const adapter=new HttpAdapter(reads.transport,memoryStorage());await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");reads.paths.length=0;
  expect(await adapter.taskUsage.getTaskUsage("TEST-alpha","t1")).toEqual(page);
  expect(await adapter.taskUsage.getTaskUsage("TEST-alpha","t1","next/attempt")).toEqual(second);
  expect(reads.paths).toEqual([`${root}/t1/model-usage?limit=50`,`${root}/t1/model-usage?limit=50&cursor=next%2Fattempt`]);
  await expect(adapter.taskUsage.getTaskUsage("OTHER","t1")).rejects.toThrow("任务不属于此项目");
});
it("adapts exact task formal review and package consent reads without starting work",async()=>{
  const formal={project_id:"TEST-alpha",task_id:"t1",processing_operation_id:"process",batch_id:"batch",workflow_version:"7",status:"partial",images:[{image_id:"image-uuid",child_run_id:"run",run_status:"completed_with_review",status:"awaiting_review",error:"3 Artifact(s) require human review"},{image_id:"other",child_run_id:null,run_status:"failed",status:"failed",error:"TEST child failed"}]};
  const reference={scope:"formal_annotation",task_id:"t1",project_schema_revision:"schema-1",intent_revision:3,intent_sha256:"frozen-delivery",processing_operation_id:"process",batch_id:"batch",source_run_id:"run",annotation_id:"annotation",annotation_revision_id:"annotation-revision",expected_snapshot_sha256:"snapshot"};
  const reviewPage={project_id:"TEST-alpha",task_id:"t1",intent_revision:3,intent_sha256:"frozen-delivery",summary:{selected:2,positive:1,negative:0,excluded:0,unreviewed:1},items:[{image_id:"image-uuid",content_sha256:"pixels",processing_operation_id:"process",batch_id:"batch",child_run_id:"run",execution_status:"awaiting_review",execution_error:"3 Artifact(s) require human review",unresolved_objects:0,review_revision:4,review_decision:"positive_complete",confirmation_current:true,snapshot_sha256:"snapshot",annotations:[{annotation_id:"annotation",label:"cup",value:{kind:"bounding_box",rect:[.1,.2,.3,.4]},annotation_revision_id:"annotation-revision",feedback_available:true,conversation_reference:reference}]},{image_id:"other",content_sha256:"other-pixels",processing_operation_id:"process",batch_id:"batch",child_run_id:null,execution_status:"failed",execution_error:"TEST child failed",unresolved_objects:0,review_revision:0,review_decision:null,confirmation_current:false,snapshot_sha256:"other-snapshot",annotations:[]}],next_cursor:null};
  const consent={input:{id:"package",intent_revision:3,intent_sha256:"frozen-delivery",confirmed:true as const},state:"armed",effective_state:"blocked",readiness:{ready:false,selected_images:2,confirmed_images:1,blocked_images:1,reasons:["whole_image_review_missing_or_stale"]},job:null};
  const calls:{path:string;method:string}[]=[];
  const reads=mockTransport({
    [`${root}/t1/formal-result`]:formal,
    [`${root}/t1/delivery-review-items?cursor=0&limit=50`]:reviewPage,
    [`${root}/t1/delivery-package-consents`]:{items:[consent],next_cursor:null},
  });
  const transport:Transport=async<T>(path:string,init?:RequestInit)=>{calls.push({path,method:init?.method||"GET"});return reads.transport<T>(path,init);};
  const adapter=new HttpAdapter(transport,memoryStorage());await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");calls.length=0;
  expect(await adapter.delivery.formalResult!("TEST-alpha","t1")).toEqual(formal);
  const summary=await adapter.delivery.reviewSummary!("TEST-alpha","t1");
  expect(summary).toMatchObject({counts:{total:2,complete:1,unresolved:1,failed:1},items:[{image_id:"image-uuid",image_sha256:"pixels",state:"positive_complete",review_revision:4,formal_selections:{annotation:{image:{image_id:"image-uuid",sha256:"pixels"},reference}}},{image_id:"other",state:"failed",error:"TEST child failed"}]});
  const readiness=await adapter.delivery.packageReadiness!("TEST-alpha","t1");
  expect(readiness).toMatchObject({intent_revision:3,intent_sha256:"frozen-delivery",ready:false,consent:{input:{id:"package"},state:"armed"},package:null,blockers:[{code:"whole_image_review_missing_or_stale"}]});
  expect(calls.every(call=>call.method==="GET")).toBe(true);
  expect(calls.filter(call=>call.path.endsWith("/delivery-package-consents"))).toHaveLength(1);
});
it("posts package permission only on explicit authorization and checks the frozen receipt",async()=>{
  const reads=mockTransport();const posts:{path:string;body:unknown}[]=[];
  const input={id:"package-consent",intent_revision:3,intent_sha256:"frozen-delivery",confirmed:true as const};
  const transport:Transport=async<T>(path:string,init?:RequestInit)=>{
    if(init?.method==="POST"){
      const body=JSON.parse(String(init.body));posts.push({path,body});
      if(path.endsWith("/cancel"))return {input,state:"cancelled",effective_state:"cancelled"} as T;
      return {input,state:"armed",effective_state:"blocked"} as T;
    }
    return reads.transport<T>(path,init);
  };
  const adapter=new HttpAdapter(transport,memoryStorage());await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");
  expect(await adapter.delivery.authorizePackage!("TEST-alpha","t1",input)).toEqual({input,state:"armed"});
  expect(await adapter.delivery.cancelPackageAuthorization!("TEST-alpha","t1",input.id)).toEqual({input,state:"cancelled"});
  expect(posts).toEqual([
    {path:`${root}/t1/delivery-package-consents`,body:input},
    {path:`${root}/t1/delivery-package-consents/package-consent/cancel`,body:{confirmed:true}},
  ]);
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
      [`${root}/${id}/workspace`, { project_id: "TEST-alpha", project_owner_id: "owner-a", conversation_id: "conversation-a", task: { input: { id, schema_revision: "schema-1" } }, agent_model: { revision: 2, model_profile_id: null }, actions: { resume: { available: false, reason: "No checkpoint" } }, queue: [], calls: [],read_model_revision:`read-${id}`,mainline:mainline(id) }],
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
      return {command_id:"same-command",action_id:"prepare_delivery_schema",replayed:false,result:{id:"schema-delivery",revision:1},workspace:mainline("t1")} as T;
    }
    return reads.transport<T>(path,init);
  };
  const adapter=new HttpAdapter(transport);await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");
  expect(posts).toEqual([]);
  const input={command_id:"same-command",expected_revision:3,expected_sha256:"frozen-delivery"};
  expect(await adapter.deliveryIntake.prepare!("TEST-alpha","t1",input)).toEqual({id:"schema-delivery",revision:1});
  expect(posts).toEqual([{path:`${root}/t1/advance`,body:{command_id:"same-command",expected_read_model_revision:"read-t1",action_id:"prepare_delivery_schema"}}]);
  reject=true;
  await expect(adapter.deliveryIntake.prepare!("TEST-alpha","t1",input)).rejects.toThrow("revision changed");
  await expect(adapter.deliveryIntake.prepare!("OTHER","t1",input)).rejects.toThrow("任务不属于");
  expect(posts).toHaveLength(2);
  expect(posts[1]).toEqual(posts[0]);
});
it("replays the exact server-authorized advance after a lost response and reload",async()=>{
  const storage=memoryStorage(),reads=mockTransport();const posts:unknown[]=[];let lost=true;
  const transport:Transport=async<T>(path:string,init?:RequestInit)=>{
    if(path===`${root}/t1/advance`&&init?.method==="POST"){
      const body=JSON.parse(String(init.body));posts.push(body);
      if(lost){lost=false;throw new Error("TEST response lost");}
      return {command_id:body.command_id,action_id:body.action_id,replayed:true,result:{id:"schema-delivery",revision:1},workspace:mainline("t1")} as T;
    }
    return reads.transport<T>(path,init);
  };
  const scope={expected_revision:3,expected_sha256:"frozen-delivery"};
  const first=new HttpAdapter(transport,storage);await first.refresh();await first.loadTask("TEST-alpha","t1");
  await expect(first.deliveryIntake.prepare!("TEST-alpha","t1",{command_id:"original-command",...scope})).rejects.toThrow("response lost");
  const restored=new HttpAdapter(transport,storage);await restored.refresh();await restored.loadTask("TEST-alpha","t1");
  expect(await restored.deliveryIntake.prepare!("TEST-alpha","t1",{command_id:"replacement-must-not-win",...scope})).toEqual({id:"schema-delivery",revision:1});
  expect(posts).toHaveLength(2);expect(posts[1]).toEqual(posts[0]);
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
it("continues the exact saved Journey into Sample without creating another Journey or Builder",async()=>{
  const consentId="journey-1",sampleId="sample-1",readPath=`${root}/t1/journey-consents/${consentId}`,executePath=`${readPath}/execution`;
  const images=[{image_id:"image-uuid",content_hash:"pixels"}],allowed_models=[{model_id:"model-profile:vision",binding_digest:"binding-digest"}];
  const scope={journey_consent_id:consentId,sample_operation_id:sampleId,draft_id:"draft-current",draft_revision:7,draft_content_hash:"d".repeat(64),images,allowed_models,maximum_sample_calls:2,expires_at:"2099-01-01T00:00:00Z"};
  const view={...mainline("t1"),available_actions:[{id:"test_pipeline_samples",state:"requires_confirmation",method:"GET",url:readPath,execution_method:"POST",execution_url:executePath,requires_confirmation:true,reason:"exact_saved_journey_sample_requires_confirmation",scope}]};
  const workspace={project_id:"TEST-alpha",project_owner_id:"owner-a",conversation_id:"conversation-a",task:{input:{id:"t1",schema_revision:"schema-1"}},agent_model:{revision:2,model_profile_id:null},actions:{},queue:[],calls:[],mainline:view};
  const consent={id:consentId,task_id:"t1",builder_operation_id:"builder-1",sample_operation_id:sampleId,builder_model_id:"text",previous_grant_id:null,builder_scope_hash:"builder-scope",schema_id:"schema",schema_revision:1,schema_digest:"schema-digest",images,allowed_models,maximum_builder_calls:4,maximum_sample_calls:2,expires_at:scope.expires_at,allow_unknown_cost:true};
  const record={consent,resolved_consent:null,revoked:false,sample:null};
  const calls:{path:string;method:string;body?:unknown}[]=[];
  const reads=mockTransport({[`${root}/t1/workspace`]:workspace,[readPath]:record});
  const transport:Transport=async<T>(path:string,init?:RequestInit)=>{
    calls.push({path,method:init?.method||"GET",...(init?.body?{body:JSON.parse(String(init.body))}:{})});
    if(path===executePath&&init?.method==="POST")return {record,builder:{status:"completed"},sample:{id:sampleId,status:"queued"}} as T;
    return reads.transport<T>(path,init);
  };
  const adapter=new HttpAdapter(transport,memoryStorage());await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");calls.length=0;
  const command={id:"continue-sample",project:"TEST-alpha",task:"t1",revision:"schema-1"};
  await adapter.prepareAction(command,"sample");
  expect(calls).toEqual([{path:readPath,method:"GET"}]);
  expect(adapter.snapshot().tasks.find(item=>item.id==="t1")?.approval).toMatchObject({title:"测试当前方案样例",revision:`Draft 7 · ${"d".repeat(8)}`});
  await adapter.approveAction(command);
  expect(calls.filter(call=>call.method==="POST")).toEqual([{path:executePath,method:"POST",body:{}}]);
  expect(calls.some(call=>call.path.includes("journey-preview")||call.path.endsWith("/journey-consents")&&call.method==="POST")).toBe(false);
});
it("uses the server-derived exact delivery processing action and only prepares confirmation",async()=>{
  const previewPath="/api/projects/TEST-alpha/processing-preview?draft_id=draft-current&sample_test_id=sample-current";
  const view={...mainline("t1"),available_actions:[{id:"start_delivery_processing",state:"requires_confirmation",method:"GET",url:previewPath,requires_confirmation:true,reason:"exact_delivery_processing_scope_requires_confirmation",scope:{delivery_revision:3,delivery_sha256:"frozen-delivery",images:[{image_id:"image-uuid",content_sha256:"pixels"}],draft:{draft_id:"draft-current",draft_revision:7,sample_test_id:"sample-current"}}}]};
  const workspace={project_id:"TEST-alpha",project_owner_id:"owner-a",conversation_id:"conversation-a",task:{input:{id:"t1",schema_revision:"schema-1"}},agent_model:{revision:2,model_profile_id:null},actions:{},queue:[],calls:[],mainline:view};
  const preview={revision:7,authorization_fingerprint:"authorization",image_count:1,available_images:1,maximum_model_calls:2,sample_feedback_count:0,plan_name:"TEST exact plan",goal:{},models:[{model_profile_id:"vision",remote_model_id:"vision-remote",provider_base_url:"https://TEST.invalid"}],native_models:[]};
  const calls:{path:string;method:string}[]=[];
  const reads=mockTransport({[`${root}/t1/workspace`]:workspace,[previewPath]:preview});
  const transport:Transport=async<T>(path:string,init?:RequestInit)=>{calls.push({path,method:init?.method||"GET"});return reads.transport<T>(path,init);};
  const adapter=new HttpAdapter(transport,memoryStorage());await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");calls.length=0;
  await adapter.prepareAction({id:"process-command",project:"TEST-alpha",task:"t1",revision:"schema-1"},"process");
  expect(calls).toEqual([{path:previewPath,method:"GET"}]);
  const approval=adapter.snapshot().tasks.find(item=>item.id==="t1")?.approval;
  expect(approval).toMatchObject({title:"确认方案并开始处理",revision:"交付 3 · Draft 7"});
  expect(approval?.scope).toEqual(expect.arrayContaining(["TEST exact plan",expect.stringContaining("服务器冻结 1 个内容哈希")]));
});
describe("HTTP UI read boundary (synthetic transport tests, not HTTP E2E)", () => {
  it("sends an exact frozen SampleCandidate reference and refuses stale Schema before POST",async()=>{
    const sendPath="/api/projects/TEST-alpha/conversations/conversation-a/send";
    const reads=mockTransport({"/api/projects/TEST-alpha/goal":{revision:"schema-1"},"/api/projects/TEST-alpha/conversations/conversation-a/agent-model":{revision:2,model_profile_id:null}});
    const posts:unknown[]=[];
    const transport:Transport=async<T>(path:string,init?:RequestInit)=>{
      if(path===sendPath&&init?.method==="POST"){
        const input=JSON.parse(String(init.body));posts.push(input);
        return {message:{conversation_id:"conversation-a",input:input.message},task_id:"t1",disposition:"candidate_feedback"} as T;
      }
      return reads.transport<T>(path,init);
    };
    const adapter=new HttpAdapter(transport,memoryStorage());await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");
    const selection={project_id:"TEST-alpha",conversation_id:"conversation-a",task_id:"t1",project_schema_revision:"schema-1",image:{image_id:"image-uuid",sha256:"pixels"},sample:{draft_id:"draft",draft_revision:2,sample_test_id:"sample"},candidate:{candidate_id:"candidate",source_artifact_id:"artifact"},annotation:{kind:"bounding_box" as const,label:"cup"},result_revision:"sample:feedback:3"};
    await adapter.sendMessage({id:"command",project:"TEST-alpha",task:"t1",revision:"schema-1",selection},"这个框太大","execute","");
    expect(posts).toHaveLength(1);
    expect(posts[0]).toMatchObject({message:{id:"command",text:"这个框太大",image:{image_id:"image-uuid",sha256:"pixels"},reference:{scope:"sample_candidate",task_id:"t1",project_schema_revision:"schema-1",draft_id:"draft",draft_revision:2,sample_test_id:"sample",candidate_id:"candidate",source_artifact_id:"artifact"}},task_id:"t1",schema_revision:"schema-1"});
    const stale={...selection,project_schema_revision:"schema-old"};
    await expect(adapter.sendMessage({id:"stale",project:"TEST-alpha",task:"t1",revision:"schema-1",selection:stale},"不要发送","execute","")).rejects.toThrow("Schema 已变化");
    expect(posts).toHaveLength(1);
    const preview={preview:true as const,task:"t1",image:"image-uuid",candidate:"candidate",revision:"schema-1"};
    await expect(adapter.sendMessage({id:"preview",project:"TEST-alpha",task:"t1",revision:"schema-1",selection:preview},"不要发送","execute","")).rejects.toThrow("演示候选");
    expect(posts).toHaveLength(1);
  });
  it("sends the canonical formal annotation reference as context without inventing geometry",async()=>{
    const sendPath="/api/projects/TEST-alpha/conversations/conversation-a/send";
    const reads=mockTransport({"/api/projects/TEST-alpha/goal":{revision:"schema-1"},"/api/projects/TEST-alpha/conversations/conversation-a/agent-model":{revision:2,model_profile_id:null}});
    const posts:unknown[]=[];
    const transport:Transport=async<T>(path:string,init?:RequestInit)=>{
      if(path===sendPath&&init?.method==="POST"){
        const input=JSON.parse(String(init.body));posts.push(input);
        return {message:{conversation_id:"conversation-a",input:input.message},task_id:"t1",disposition:"formal_feedback"} as T;
      }
      return reads.transport<T>(path,init);
    };
    const adapter=new HttpAdapter(transport,memoryStorage());await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");
    const reference={scope:"formal_annotation" as const,task_id:"t1",project_schema_revision:"schema-1",intent_revision:3,intent_sha256:"intent",processing_operation_id:"operation",batch_id:"batch",source_run_id:"run",annotation_id:"annotation",annotation_revision_id:"annotation-revision",expected_snapshot_sha256:"snapshot"};
    const selection={project_id:"TEST-alpha",conversation_id:"conversation-a",task_id:"t1",project_schema_revision:"schema-1",image:{image_id:"image-uuid",sha256:"pixels"},reference,annotation:{kind:"bounding_box" as const,label:"cup"},result_revision:"annotation-revision"};
    await adapter.sendMessage({id:"formal-message",project:"TEST-alpha",task:"t1",revision:"schema-1",selection},"这个正式框右边太宽","execute","");
    expect(posts).toEqual([expect.objectContaining({message:{id:"formal-message",text:"这个正式框右边太宽",image:{image_id:"image-uuid",sha256:"pixels"},reference}})]);
  });
  it("freezes a lost stop selection across reload and refuses another target",async()=>{
    const targets=[{kind:"builder",id:"one",task_id:"t1",state:"running",parent_journey_ids:[]},{kind:"call",id:"two",task_id:"t1",state:"running",parent_journey_ids:[]}];
    let record={message:{conversation_id:"conversation-a",input:{id:"stop",text:"停止",image:null,reference:{scope:"stop_request",task_id:"t1"}}},status:"needs_selection",targets,selected_target:null as unknown,normalized_state:null};
    const stopPath="/api/projects/TEST-alpha/conversations/conversation-a/stop-requests/stop";
    const {transport:base}=mockTransport({[stopPath]:async()=>record});const bodies:string[]=[];
    const transport:Transport=async<T>(path:string,init?:RequestInit)=>{if(path===`${stopPath}/select`){bodies.push(String(init?.body));if(bodies.length===1)throw new Error("TEST lost before acknowledgement");record={...record,status:"cancel_requested",selected_target:JSON.parse(String(init?.body)).target};return record as T;}return base<T>(path,init);};
    const values=new Map<string,string>([["annotagent.http-ui.TEST-workspace.stop.t1",JSON.stringify({id:"stop"})]]);const storage={getItem:(k:string)=>values.get(k)||null,setItem:(k:string,v:string)=>values.set(k,v)} as unknown as Storage;
    const adapter=new HttpAdapter(transport,storage);await adapter.refresh();await adapter.loadTask("TEST-alpha","t1");const c={id:"selection",task:"t1",project:"TEST-alpha",revision:"schema-1"};
    await expect(adapter.selectStop(c,"builder:one")).rejects.toThrow("TEST lost");await adapter.loadTask("TEST-alpha","t1");expect(adapter.snapshot().tasks.find(t=>t.id==="t1")?.stopTargets?.map(t=>t.id)).toEqual(["builder:one"]);
    await expect(adapter.selectStop(c,"call:two")).rejects.toThrow();expect(bodies).toHaveLength(1);await adapter.selectStop(c,"builder:one");expect(bodies[1]).toBe(bodies[0]);expect(adapter.snapshot().tasks.find(t=>t.id==="t1")?.stopTargets).toEqual([]);
  });
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
    expect(adapter.snapshot().tasks.find(t => t.id === "t1")?.items).toEqual([{ id: "message-t1", role: "user", kind:"input", text: "真实已存 t1",source:{kind:"message",id:"message-t1"} }]);
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
