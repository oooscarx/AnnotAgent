import { test, expect } from "@playwright/test";
import { resolve } from "node:path";
import { randomUUID, createHash } from "node:crypto";
import {readFileSync} from "node:fs";
test("native feedback scope recovers a real saved choice without changing sample results",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const m=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8"));expect(m.fixture).toBe("external-model-only");const root=m.task_root.split("/tasks/")[0];const tr=m.task_root;const headers={"x-annotagent-csrf":(await(await request.get("/api/session")).json()).csrf_token};
  // Only the marked TEST seed's blocking requests are cancelled, never user records.
  for(const h of await(await request.get(`${tr}/human-requests`)).json()){if(h.status==="pending"){const response=await request.post(`${tr}/human-requests/${h.input.id}/cancel`,{headers,data:{}});expect(response.ok(),await response.text()).toBeTruthy();}}
  const execution=await(await request.get(m.execution_url)).json();const record=(await(await request.get(`/api/workflow-drafts/${execution.sample.draft_id}/sample-test?test_id=${execution.sample.id}`)).json()).sample_test;expect(record).toBeTruthy();const projection=record.report.samples[0].projection;const candidate=projection.final_candidates[0]||projection.review_candidates[0].candidate;const image=record.inputs[0];const tasks=await(await request.get(`${root}/tasks`)).json();const schema=tasks.find((t:any)=>t.input.id===m.task_id).input.schema_revision;
  const message={id:randomUUID(),text:`TEST native feedback scope ${randomUUID()}`,image:{image_id:image.image_id,sha256:image.content_hash},reference:{scope:"sample_candidate",task_id:m.task_id,project_schema_revision:schema,draft_id:record.draft_id,draft_revision:record.draft_revision,sample_test_id:record.id,candidate_id:candidate.outcome.id,source_artifact_id:candidate.source_artifact_id}};const added=await request.post(`${root}/messages`,{headers,data:message});expect(added.ok(),await added.text()).toBeTruthy();
  const previewResponse=await request.get(`${tr}/feedback-preview?${new URLSearchParams({message_id:message.id,call_id:randomUUID(),model_id:m.model_profile_id})}`);expect(previewResponse.ok(),await previewResponse.text()).toBeTruthy();const preview=await previewResponse.json();expect(preview.destination).toContain("127.0.0.1");const consent={...preview.consent,allow_unknown_cost:true};const granted=await request.post(`${tr}/feedback-authorizations`,{headers,data:consent});expect(granted.ok(),await granted.text()).toBeTruthy();const path=`${tr}/feedback/${consent.call_id}`;const executed=await request.post(`${path}/execute`,{headers,data:{}});expect(executed.ok(),await executed.text()).toBeTruthy();const status=await(await request.get(path)).json();expect(status.decision.Ok.decision).toBe("clarify_scope");
  const sent:any[]=[];await page.route(`**${path}/scope-answer`,async route=>{sent.push(route.request().postDataJSON());const response=await route.fetch();expect(response.ok(),await response.text()).toBeTruthy();await route.abort();});const writes:string[]=[];page.on("request",r=>{if(r.method()==="POST")writes.push(new URL(r.url()).pathname);});
const open=async()=>{await page.locator("summary").filter({hasText:"反馈记录与修改范围"}).click();await page.getByRole("region",{name:"任务反馈记录",exact:true}).locator("summary").filter({hasText:message.text}).click();return page.getByRole("region",{name:"任务反馈记录",exact:true}).locator("details").filter({has:page.locator("summary").filter({hasText:message.text})});};await page.goto(`/projects/${m.project}/work?task=${m.task_id}`);let scope=await open();await scope.getByRole("radio",{name:"后续任务的标签规则",exact:true}).check();await scope.getByRole("button",{name:"保存反馈范围",exact:true}).click();await expect(scope.getByRole("alert")).toBeVisible();await page.reload();scope=await open();await expect(scope).toContainText("已保存范围：后续任务的标签规则");expect(sent).toHaveLength(1);expect(writes).toEqual([`${path}/scope-answer`]);expect(sent[0].expected_context_digest).toBe(status.scope_context_digest);expect((await(await request.get(path)).json()).scope_answer.input).toEqual(sent[0]);expect((await(await request.get(`/api/workflow-drafts/${record.draft_id}/sample-test?test_id=${record.id}`)).json()).sample_test).toEqual(record);
  const ruleInputs:any[]=[];await page.route(`**${path}/future-schema`,async route=>{if(route.request().method()!=="POST"){await route.continue();return;}ruleInputs.push(route.request().postDataJSON());const response=await route.fetch();expect(response.ok(),await response.text()).toBeTruthy();await route.abort();});
  await scope.locator("summary").filter({hasText:"后续任务规则草稿"}).click();let rules=scope.getByRole("region",{name:"后续任务规则编辑",exact:true});await rules.getByLabel("未来规则边界",{exact:true}).fill("TEST use visible boundaries only");await expect(rules.getByRole("region",{name:"与已测试规则的差异",exact:true})).toContainText("TEST use visible boundaries only");await rules.getByRole("button",{name:"保存未来规则草稿",exact:true}).click();await expect(rules.getByRole("alert")).toBeVisible();const savedRules=await(await request.get(`${path}/future-schema`)).json();expect(savedRules.record).toBeTruthy();
  await page.reload();scope=await open();await scope.locator("summary").filter({hasText:"后续任务规则草稿"}).click();rules=scope.getByRole("region",{name:"后续任务规则编辑",exact:true});await expect(rules).toContainText("未来规则草稿已保存");await expect(rules).toContainText(savedRules.record.schema_id);expect(ruleInputs).toHaveLength(1);expect(savedRules.record.input.command_id).toBe(ruleInputs[0].command_id);expect(savedRules.schema.definition.boundary_rules).toEqual(["TEST use visible boundaries only"]);expect((await(await request.get(`/api/workflow-drafts/${record.draft_id}/sample-test?test_id=${record.id}`)).json()).sample_test).toEqual(record);
  await rules.screenshot({path:"/tmp/annotagent-native-future-rules.png"});
  await rules.getByRole("button",{name:"使用此规则构建新方案",exact:true}).click();const futureBuilder=rules.getByRole("region",{name:"从语义草稿构建方案",exact:true});const previewRequest=page.waitForRequest(r=>r.url().includes("/builder-preview?"));await futureBuilder.getByRole("button",{name:"检查此草稿的构建授权…",exact:true}).click();const selected=new URL((await previewRequest).url());expect(selected.searchParams.get("schema_id")).toBe(savedRules.schema.id);expect(selected.searchParams.get("schema_revision")).toBe(String(savedRules.schema.revision));const dialog=page.getByRole("dialog",{name:"确认语义草稿构建范围",exact:true});await expect(dialog).toContainText("费用未知");await expect(dialog.getByRole("button",{name:"生成方案草稿",exact:true})).toBeDisabled();await dialog.getByRole("button",{name:"取消",exact:true}).click();expect(writes).toEqual([`${path}/scope-answer`,`${path}/future-schema`]);
  for(let i=0;i<101;i++){const response=await request.post(`${root}/messages`,{headers,data:{id:randomUUID(),text:`TEST passive history filler ${i}`,image:null}});expect(response.ok(),await response.text()).toBeTruthy();}
  await page.reload();await page.locator("summary").filter({hasText:"反馈记录与修改范围"}).click();const history=page.getByRole("region",{name:"任务反馈记录",exact:true});await expect(history).toContainText("当前历史页没有此任务的已授权反馈");await history.getByRole("button",{name:"较早历史",exact:true}).click();await history.locator("summary").filter({hasText:message.text}).click();await expect(history).toContainText("已保存范围：后续任务的标签规则");expect(writes).toEqual([`${path}/scope-answer`,`${path}/future-schema`]);
});
test("native clarification answer and cancel survive lost HTTP responses without continuation",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const nav=await(await request.get("/api/navigation")).json();const p=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture");const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;const headers={"x-annotagent-csrf":(await(await request.get("/api/session")).json()).csrf_token};
  const source=(await(await request.get("/api/model-profiles")).json()).models.find((m:{remote_model_id:string})=>m.remote_model_id==="e2e-conversation-classification-feedback-clarify");const made=await request.post("/api/model-profiles",{headers,data:{provider_id:source.provider_id,display_name:"TEST native clarification",remote_model_id:"e2e-conversation-clarify",input_modalities:["text","image"],task_capabilities:["text_generation","vision_language","image_classification"],protocol_features:{tool_calls:true,structured_output:true}}});expect(made.ok(),await made.text()).toBeTruthy();const model=await made.json();
  const permission=await request.post("/api/session/privileged-confirmation",{headers,data:{action:`POST /api/providers/${source.provider_id}/active-probe`,confirmed:true}});expect(permission.ok()).toBeTruthy();const probe=await request.post(`/api/providers/${source.provider_id}/active-probe`,{headers:{...headers,"x-annotagent-privileged-confirmation":(await permission.json()).confirmation_token},data:{model_profile_id:model.id,confirmed_billable:true}});expect(probe.ok(),await probe.text()).toBeTruthy();
  const create=async()=>{const original=await(await request.get(`${root}/agent-model`)).json();const selected=await request.post(`${root}/agent-model`,{headers,data:{request_id:randomUUID(),expected_revision:original.revision,model_profile_id:model.id}});expect(selected.ok()).toBeTruthy();const sent=await request.post(`${root}/send`,{headers,data:{message:{id:randomUUID(),text:"TEST ambiguous target; ask me for labels",image:null},schema_revision:(await(await request.get(`/api/projects/${p.project_id}/goal`)).json()).revision,mode:"plan"}});const restored=await request.post(`${root}/agent-model`,{headers,data:{request_id:randomUUID(),expected_revision:(await selected.json()).revision,model_profile_id:original.model_profile_id}});expect(restored.ok()).toBeTruthy();expect(sent.ok()).toBeTruthy();const task=(await sent.json()).task_id;const tr=`${root}/tasks/${task}`;const previewResponse=await request.get(`${tr}/schema-preview?model_id=${model.id}`);expect(previewResponse.ok(),await previewResponse.text()).toBeTruthy();const preview=await previewResponse.json();expect(preview.destination).toContain("127.0.0.1");const call=randomUUID();const proposed=await request.post(`${tr}/schema-proposals`,{headers,data:{call_id:call,model_id:model.id,scope_hash:preview.scope_hash,expires_at:preview.expires_at,allow_unknown_cost:true}});expect(proposed.ok(),await proposed.text()).toBeTruthy();const endpoint=`${tr}/calls/${call}/clarification`;const state=await(await request.get(endpoint)).json();expect(state.status).toBe("pending");return {task,tr,call,endpoint,state};};
  const first=await create();const second=await create();const before=await(await request.get(`/api/projects/${p.project_id}/export-readiness`)).json();const calls=await(await request.get(`${first.tr}/calls`)).json();const writes:{path:string;body:any}[]=[];
  page.on("request",r=>{if(r.method()==="POST")writes.push({path:new URL(r.url()).pathname,body:r.postDataJSON()});});
  await page.route(`**${first.tr}/human-schema-drafts`,async route=>{if(route.request().method()!=="POST"){await route.continue();return;}const response=await route.fetch();expect(response.ok(),await response.text()).toBeTruthy();await route.abort();});
const open=async(task:string)=>{await page.goto(`/projects/${p.project_id}/work?task=${task}`);await page.locator("summary").filter({hasText:"标签与边界规则草稿"}).click();return page.getByRole("region",{name:"任务澄清问题",exact:true});};let region=await open(first.task);await expect(region).toContainText(first.state.question);await region.locator("summary").filter({hasText:"回答这个澄清问题"}).click();const form=region.getByRole("region",{name:"澄清回答",exact:true});await form.getByLabel("新草稿标签（每行一个）",{exact:true}).fill("TEST cup");await expect(region.getByRole("button",{name:"取消此澄清问题",exact:true})).toBeDisabled();await form.getByRole("button",{name:"保存澄清回答",exact:true}).click();await expect(form.getByRole("alert")).toBeVisible();expect((await(await request.get(first.endpoint)).json()).status).toBe("applied");
  await page.reload();await page.locator("summary").filter({hasText:"标签与边界规则草稿"}).click();region=page.getByRole("region",{name:"任务澄清问题",exact:true});await expect(region).toContainText("回答已保存；不会自动续跑");expect(writes).toHaveLength(1);expect(writes[0].body.clarification).toEqual({call_id:first.call,expected_schema_revision:first.state.expected_schema_revision});expect(writes[0].body.journey_consent_id).toBeUndefined();expect(await(await request.get(`${first.tr}/calls`)).json()).toEqual(calls);
  await page.route(`**${second.endpoint}/cancel`,async route=>{const response=await route.fetch();expect(response.ok(),await response.text()).toBeTruthy();await route.abort();});region=await open(second.task);page.once("dialog",d=>d.accept());await region.getByRole("button",{name:"取消此澄清问题",exact:true}).click();await expect(region.getByRole("alert")).toContainText("结果待核实");await page.reload();await page.locator("summary").filter({hasText:"标签与边界规则草稿"}).click();await expect(page.getByRole("region",{name:"任务澄清问题",exact:true})).toContainText("问题已取消");expect(writes).toHaveLength(2);expect(writes[1].path).toBe(`${second.endpoint}/cancel`);expect(await(await request.get(`/api/projects/${p.project_id}/export-readiness`)).json()).toEqual(before);
});
test("native schema inventory restores the server-materialized proposal without writes",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const nav=await(await request.get("/api/navigation")).json();const p=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture");
  const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;const headers={"x-annotagent-csrf":(await(await request.get("/api/session")).json()).csrf_token};
  const models=(await(await request.get("/api/model-profiles")).json()).models;const model=models.find((m:{remote_model_id:string})=>m.remote_model_id==="e2e-conversation-classification-feedback-clarify");const original=await(await request.get(`${root}/agent-model`)).json();const selected=await request.post(`${root}/agent-model`,{headers,data:{request_id:randomUUID(),expected_revision:original.revision,model_profile_id:model.id}});expect(selected.ok()).toBeTruthy();
  const sent=await request.post(`${root}/send`,{headers,data:{message:{id:randomUUID(),text:"TEST classify the whole image as indoor or outdoor",image:null},schema_revision:(await(await request.get(`/api/projects/${p.project_id}/goal`)).json()).revision,mode:"plan"}});
  const restored=await request.post(`${root}/agent-model`,{headers,data:{request_id:randomUUID(),expected_revision:(await selected.json()).revision,model_profile_id:original.model_profile_id}});expect(restored.ok()).toBeTruthy();expect(sent.ok(),await sent.text()).toBeTruthy();
  const task=(await sent.json()).task_id;const tr=`${root}/tasks/${task}`;
  const previewResponse=await request.get(`${tr}/schema-preview?model_id=${model.id}`);expect(previewResponse.ok(),await previewResponse.text()).toBeTruthy();const preview=await previewResponse.json();expect(preview.destination).toContain("127.0.0.1");
  const call=randomUUID();const proposed=await request.post(`${tr}/schema-proposals`,{headers,data:{call_id:call,model_id:model.id,scope_hash:preview.scope_hash,expires_at:preview.expires_at,allow_unknown_cost:true}});expect(proposed.ok(),await proposed.text()).toBeTruthy();
  await expect.poll(async()=>{const calls=await(await request.get(`${tr}/calls`)).json();return calls.find((c:{id:string})=>c.id===call)?.status;}).toBe("completed");
  const calls=await(await request.get(`${tr}/calls`)).json();expect(calls.find((c:{id:string})=>c.id===call).evidence.decision.Ok.decision).toBe("draft");
  const endpoint=`${tr}/calls/${call}/schema-draft`;const draft=await(await request.get(endpoint)).json();expect(draft.source_call_id).toBe(call);expect(draft.task_id).toBe(task);const before=await(await request.get(`/api/projects/${p.project_id}/export-readiness`)).json();const writes:string[]=[];
  page.on("request",r=>{if(r.method()==="POST")writes.push(new URL(r.url()).pathname);});
  await page.goto(`/projects/${p.project_id}/work?task=${task}`);await page.locator("summary").filter({hasText:"标签与边界规则草稿"}).click();await expect(page.getByRole("region",{name:"任务语义草稿",exact:true})).toContainText(draft.definition.task.labels.join("、"));expect(writes).toEqual([]);
  await page.reload();await page.locator("summary").filter({hasText:"标签与边界规则草稿"}).click();await expect(page.locator("summary").filter({hasText:"尚未保存为草稿的建议"})).toHaveCount(0);const summary=page.getByRole("region",{name:"任务语义草稿",exact:true}).locator("summary").filter({hasText:`${draft.definition.task.labels.join("、")} · revision ${draft.revision}`});await expect(summary).toBeVisible();
  expect(writes).toEqual([]);expect(await(await request.get(endpoint)).json()).toEqual(draft);expect(await(await request.get(`${tr}/calls`)).json()).toEqual(calls);expect(await(await request.get(`/api/projects/${p.project_id}/export-readiness`)).json()).toEqual(before);
});
test("native Builder stop persists its receipt across refresh without redispatch",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const nav=await(await request.get("/api/navigation")).json();const p=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture");const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;const headers={"x-annotagent-csrf":(await(await request.get("/api/session")).json()).csrf_token};const models=(await(await request.get("/api/model-profiles")).json()).models;const source=models.find((m:{remote_model_id:string})=>m.remote_model_id==="e2e-conversation-classification-feedback-clarify");
const made=await request.post("/api/model-profiles",{headers,data:{provider_id:source.provider_id,display_name:"TEST slow Builder stop",remote_model_id:"e2e-conversation-classification-background",input_modalities:["text","image"],task_capabilities:["text_generation","vision_language","image_classification"],protocol_features:{tool_calls:true,structured_output:true}}});expect(made.ok(),await made.text()).toBeTruthy();const model=await made.json();const permission=await request.post("/api/session/privileged-confirmation",{headers,data:{action:`POST /api/providers/${source.provider_id}/active-probe`,confirmed:true}});expect(permission.ok()).toBeTruthy();const probe=await request.post(`/api/providers/${source.provider_id}/active-probe`,{headers:{...headers,"x-annotagent-privileged-confirmation":(await permission.json()).confirmation_token},data:{model_profile_id:model.id,confirmed_billable:true}});expect(probe.ok(),await probe.text()).toBeTruthy();
  const sentTask=await request.post(`${root}/send`,{headers,data:{message:{id:randomUUID(),text:"TEST stop a slow classification Builder",image:null},schema_revision:(await(await request.get(`/api/projects/${p.project_id}/goal`)).json()).revision,mode:"plan"}});expect(sentTask.ok(),await sentTask.text()).toBeTruthy();const task=(await sentTask.json()).task_id;const tr=`${root}/tasks/${task}`;const label=`TEST-stop-${randomUUID()}`;const created=await request.post(`${tr}/human-schema-drafts`,{headers,data:{request_id:randomUUID(),decision:{decision:"draft",kind:"classification",labels:[label],multi_label:false,attributes:{},boundary_rules:[],rationale:"TEST stop verification"}}});expect(created.ok()).toBeTruthy();
  // Route only the preview's explicit model selection to an actual registered loopback TEST model; no fake responses.
  await page.route(`**${tr}/builder-preview?*`,async route=>{const url=new URL(route.request().url());url.searchParams.set("model_id",model.id);await route.continue({url:url.href});});const launches:string[]=[];let cancelCount=0;page.on("request",r=>{if(r.method()==="POST"&&new URL(r.url()).pathname===`${tr}/builder-operations`)launches.push(r.postData()!);});
  await page.route(`**${tr}/calls/*/cancel`,async route=>{cancelCount++;const response=await route.fetch();expect(response.ok(),await response.text()).toBeTruthy();await route.abort();});
  const open=async()=>{await page.locator("summary").filter({hasText:"标签与边界规则草稿"}).click();const group=page.getByRole("region",{name:"任务语义草稿",exact:true});await group.locator("summary").filter({hasText:label}).click();const editor=group.locator("details").filter({has:page.locator("summary").filter({hasText:label})});await editor.getByRole("button",{name:"从此草稿构建方案",exact:true}).click();return editor.getByRole("region",{name:"从语义草稿构建方案",exact:true});};
await page.goto(`/projects/${p.project_id}/work?task=${task}`);let builder=await open();await builder.getByRole("button",{name:"检查此草稿的构建授权…",exact:true}).click();const dialog=page.getByRole("dialog");await expect(dialog).toContainText("e2e-conversation-classification-background");await dialog.getByRole("checkbox").check();await dialog.getByRole("button",{name:"生成方案草稿",exact:true}).click();await builder.getByRole("button",{name:"停止此构建",exact:true}).click();await expect.poll(()=>cancelCount).toBe(1);await expect(builder).toContainText(/停止结果未知|取消请求已保存/);await page.reload();builder=await open();await expect(builder).toContainText("取消请求已保存");expect(launches).toHaveLength(1);const operation=JSON.parse(launches[0]).selection.operation_id;const cancellations=await(await request.get(`${tr}/cancellations`)).json();expect(cancellations.some((v:{call_id:string;task_id:string})=>v.call_id===operation&&v.task_id===task)).toBe(true);await expect(builder.getByRole("button",{name:"核实同一构建请求",exact:true})).toHaveCount(0);expect(cancelCount).toBe(1);
});
test("native Schema Builder executes TEST HTTP once and recovers a lost launch response",async({page,request})=>{
expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const nav=await(await request.get("/api/navigation")).json();const p=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture");const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;const headers={"x-annotagent-csrf":(await(await request.get("/api/session")).json()).csrf_token};const sentTask=await request.post(`${root}/send`,{headers,data:{message:{id:randomUUID(),text:"TEST classify whole image as day",image:null},schema_revision:(await(await request.get(`/api/projects/${p.project_id}/goal`)).json()).revision,mode:"plan"}});expect(sentTask.ok(),await sentTask.text()).toBeTruthy();const task=(await sentTask.json()).task_id;expect(task).toBeTruthy();const tr=`${root}/tasks/${task}`;const label=`TEST-execute-${randomUUID()}`;
  const created=await request.post(`${tr}/human-schema-drafts`,{headers,data:{request_id:randomUUID(),decision:{decision:"draft",kind:"classification",labels:[label],multi_label:false,attributes:{},boundary_rules:[],rationale:"TEST real HTTP Builder"}}});expect(created.ok(),await created.text()).toBeTruthy();const schema=await created.json();const before=await(await request.get(`/api/projects/${p.project_id}/export-readiness`)).json();const sent:Record<string,any>[]=[];
  await page.route(`**${tr}/builder-operations`,async route=>{if(route.request().method()!=="POST"){await route.continue();return;}sent.push(route.request().postDataJSON());const response=await route.fetch();expect(response.ok(),await response.text()).toBeTruthy();await route.abort();});
  const open=async()=>{await page.locator("summary").filter({hasText:"标签与边界规则草稿"}).click();await page.getByRole("region",{name:"任务语义草稿",exact:true}).locator("summary").filter({hasText:label}).click();const editor=page.getByRole("region",{name:"任务语义草稿",exact:true}).locator("details").filter({has:page.locator("summary").filter({hasText:label})});await editor.getByRole("button",{name:"从此草稿构建方案",exact:true}).click();return editor.getByRole("region",{name:"从语义草稿构建方案",exact:true});};
  await page.goto(`/projects/${p.project_id}/work?task=${task}`);let builder=await open();await builder.getByRole("button",{name:"检查此草稿的构建授权…",exact:true}).click();const dialog=page.getByRole("dialog",{name:"确认语义草稿构建范围",exact:true});await expect(dialog).toContainText("127.0.0.1");await dialog.getByRole("checkbox").check();await dialog.getByRole("button",{name:"生成方案草稿",exact:true}).click();await expect.poll(()=>sent.length).toBe(1);
  const operation=sent[0].selection.operation_id;expect(sent[0].selection.schema_id).toBe(schema.id);await expect.poll(async()=>{const history=await(await request.get(`${tr}/builder-operations?operation_id=${operation}`)).json();return history.items[0]?.operation.status;},{timeout:45000}).toBe("completed");
  const history=await(await request.get(`${tr}/builder-operations?operation_id=${operation}`)).json();expect(history.items).toHaveLength(1);const result=history.items[0];expect(result.operation.evidence.draft_id).toBeTruthy();expect(result.operation.evidence.schema_id).toBe(schema.id);
  await page.reload();builder=await open();await builder.locator("summary").filter({hasText:"构建记录 · completed"}).click();await expect(builder.getByRole("link",{name:"查看实际方案草稿",exact:true})).toHaveAttribute("href",`/projects/${p.project_id}/manage/pipelines/${result.operation.evidence.draft_id}`);expect(sent).toHaveLength(1);expect(await(await request.get(`/api/projects/${p.project_id}/export-readiness`)).json()).toEqual(before);
});
test("native Schema Builder previews exact saved revision and cancels without starting models",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const nav=await(await request.get("/api/navigation")).json();const p=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture");const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;const tasks=await(await request.get(`${root}/task-navigation`)).json();const task=tasks.items[0].task_id;const headers={"x-annotagent-csrf":(await(await request.get("/api/session")).json()).csrf_token};const label=`TEST-build-${randomUUID()}`;
  const response=await request.post(`${root}/tasks/${task}/human-schema-drafts`,{headers,data:{request_id:randomUUID(),decision:{decision:"draft",kind:"classification",labels:[label],multi_label:false,attributes:{},boundary_rules:[],rationale:"TEST exact Builder preview"}}});expect(response.ok(),await response.text()).toBeTruthy();const draft=await response.json();const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});await page.goto(`/projects/${p.project_id}/work?task=${task}`);await page.locator("summary").filter({hasText:"标签与边界规则草稿"}).click();await page.getByRole("region",{name:"任务语义草稿",exact:true}).locator("summary").filter({hasText:label}).click();const editor=page.getByRole("region",{name:"任务语义草稿",exact:true}).locator("details").filter({has:page.locator("summary").filter({hasText:label})});await editor.getByRole("button",{name:"从此草稿构建方案",exact:true}).click();const builder=editor.getByRole("region",{name:"从语义草稿构建方案",exact:true});
  const sent=page.waitForRequest(r=>r.url().includes("/builder-preview?"));await builder.getByRole("button",{name:"检查此草稿的构建授权…",exact:true}).click();const url=new URL((await sent).url());expect(url.searchParams.get("schema_id")).toBe(draft.id);expect(url.searchParams.get("schema_revision")).toBe(String(draft.revision));const dialog=page.getByRole("dialog",{name:"确认语义草稿构建范围",exact:true});await expect(dialog).toContainText("费用未知");await expect(dialog.getByRole("button",{name:"生成方案草稿",exact:true})).toBeDisabled();await dialog.getByRole("button",{name:"取消",exact:true}).click();await page.reload();expect(writes).toEqual([]);
});
test("native manual Schema creation is recoverable and sends no execution authority",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const nav=await(await request.get("/api/navigation")).json();const p=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture");const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;const tasks=await(await request.get(`${root}/task-navigation`)).json();const task=tasks.items[0].task_id;const path=`${root}/tasks/${task}/human-schema-drafts`;const label=`TEST-manual-${randomUUID()}`;const sent:Record<string,unknown>[]=[];const writes:string[]=[];
  await page.route(`**${path}`,async r=>{if(r.request().method()!=="POST"){await r.continue();return;}sent.push(r.request().postDataJSON());const response=await r.fetch();expect(response.ok(),await response.text()).toBeTruthy();if(sent.length===1)await r.abort();else await r.fulfill({response});});page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto(`/projects/${p.project_id}/work?task=${task}`);const open=async()=>{await page.locator("summary").filter({hasText:"标签与边界规则草稿"}).click();await page.locator("summary").filter({hasText:"无需模型，手工定义标签"}).click();};await open();const form=page.getByRole("region",{name:"手工定义语义草稿",exact:true});await form.getByLabel("新草稿标签（每行一个）",{exact:true}).fill("cancelled input");await form.getByRole("button",{name:"取消手工输入",exact:true}).click();expect(writes).toEqual([]);
  await form.getByLabel("输出类型",{exact:true}).selectOption("classification");await form.getByLabel("新草稿标签（每行一个）",{exact:true}).fill(label);await form.getByRole("button",{name:"保存手工草稿",exact:true}).click();await expect(form.getByRole("alert")).toBeVisible();expect(sent).toHaveLength(1);page.on("dialog",d=>d.accept());await page.reload();expect(sent).toHaveLength(1);await open();await expect(form.getByLabel("新草稿标签（每行一个）",{exact:true})).toHaveValue(label);await form.getByRole("button",{name:"核实原创建请求",exact:true}).click();await expect(form).toContainText("手工草稿已保存");expect(sent).toHaveLength(2);expect(sent[1]).toEqual(sent[0]);expect(Object.keys(sent[0]).sort()).toEqual(["decision","request_id"]);expect(writes.every(w=>w.endsWith("/human-schema-drafts"))).toBe(true);
  const saved=await(await request.get(path)).json();expect(saved.filter((d:{source_request_id:string})=>d.source_request_id===sent[0].request_id)).toHaveLength(1);
});
test("native task semantics save exact revisions and restore a lost response without another edit",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const nav=await(await request.get("/api/navigation")).json();const p=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture");const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;const tasks=await(await request.get(`${root}/task-navigation`)).json();const task=tasks.items[0].task_id;const headers={"x-annotagent-csrf":(await(await request.get("/api/session")).json()).csrf_token};const label=`TEST-semantic-${randomUUID()}`;
  const created=await request.post(`${root}/tasks/${task}/human-schema-drafts`,{headers,data:{request_id:randomUUID(),decision:{decision:"draft",kind:"bounding_box",labels:[label],multi_label:false,attributes:{},boundary_rules:[],rationale:"TEST native migration"}}});expect(created.ok(),await created.text()).toBeTruthy();const draft=await created.json();const path=`/api/projects/${p.project_id}/conversation-schema-drafts/${draft.id}`;const sent:unknown[]=[];
  await page.route(`**${path}`,async r=>{if(r.request().method()!=="POST"){await r.continue();return;}sent.push(r.request().postDataJSON());const response=await r.fetch();expect(response.ok(),await response.text()).toBeTruthy();if(sent.length===1)await r.abort();else await r.fulfill({response});});
await page.goto(`/projects/${p.project_id}/work?task=${task}`);await page.locator("summary").filter({hasText:"标签与边界规则草稿"}).click();const group=page.getByRole("region",{name:"任务语义草稿",exact:true});await group.locator("summary").filter({hasText:label}).click();const editor=group.locator("details").filter({has:page.locator("summary").filter({hasText:label})});
  await editor.getByLabel("标签（每行一个）").fill(`${label}-edited`);await editor.getByRole("button",{name:"保存语义草稿",exact:true}).click();await expect(group.getByRole("alert")).toBeVisible();expect(sent).toHaveLength(1);page.on("dialog",d=>d.accept());await page.reload();expect(sent).toHaveLength(1);
  await page.locator("summary").filter({hasText:"标签与边界规则草稿"}).click();await group.locator("summary").filter({hasText:`${label}-edited`}).click();await group.getByRole("button",{name:"核实原保存请求",exact:true}).click();await expect(group.getByRole("button",{name:"核实原保存请求",exact:true})).toHaveCount(0);expect(sent).toHaveLength(2);expect(sent[0]).toEqual(sent[1]);const saved=await(await request.get(path)).json();expect(saved.revision).toBe(draft.revision+1);expect(saved.definition.task.labels).toEqual([`${label}-edited`]);
});
test("native image browser uses real previews while preserving original canvas and URL selection",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const nav=await(await request.get("/api/navigation")).json();const p=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture");const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;const tasks=await(await request.get(`${root}/task-navigation`)).json();const task=tasks.items[0].task_id;const images=(await(await request.get(`/api/projects/${p.project_id}/images`)).json()).images;expect(images.length).toBeGreaterThan(0);const image=images[0];
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});await page.goto(`/projects/${p.project_id}/work?task=${task}&pane=image&image=${image.image_id}`);const browser=page.getByRole("region",{name:"图片浏览",exact:true});const selected=browser.getByRole("button",{name:`查看图片 ${image.image_id}`,exact:true});await expect(selected.locator("img")).toHaveAttribute("src",image.thumbnail_url||image.url);await expect(selected).toHaveAttribute("aria-pressed","true");expect(await browser.locator("img").count()).toBeLessThanOrEqual(24);await expect(page.locator(".artifact-pane svg image").first()).toHaveAttribute("href",image.url);await page.reload();await expect(selected).toHaveAttribute("aria-pressed","true");expect(writes).toEqual([]);
});
test("native task export history reads actual owned records and refreshes without mutation",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const nav=await(await request.get("/api/navigation")).json();const p=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture");const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;
  const tasks=await(await request.get(`${root}/task-navigation`)).json();const task=tasks.items[0].task_id;const expected=await(await request.get(`${root}/tasks/${task}/exports?limit=20`)).json();
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});await page.goto(`/projects/${p.project_id}/work?task=${task}`);
  const summary=page.locator("summary").filter({hasText:"导出历史与兼容性报告"});await summary.click();const history=page.getByRole("region",{name:"任务导出历史",exact:true});await expect(history).toContainText("由当前任务请求的项目级导出");
  if(expected.length===0)await expect(history).toContainText("当前页没有导出记录");else await expect(history.locator("article")).toHaveCount(expected.length);
  await history.getByRole("button",{name:"刷新导出记录",exact:true}).click();await expect(history.getByRole("button",{name:"较新记录",exact:true})).toBeDisabled();await page.reload();await expect(history).toHaveCount(0);await summary.click();await expect(history).toBeVisible();expect(writes).toEqual([]);
});
test("controlled task export pages retain reports and show inactive work as unknown",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const nav=await(await request.get("/api/navigation")).json();const p=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture");const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;const tasks=await(await request.get(`${root}/task-navigation`)).json();const task=tasks.items[0].task_id;const path=`${root}/tasks/${task}/exports`;
  // Explicit read-only browser response fixture, not a backend export or file.
  await page.route(`**${path}*`,async r=>{const before=new URL(r.request().url()).searchParams.get("before");await r.fulfill({json:before?[]:Array.from({length:20},(_,i)=>({id:`TEST-export-${i}`,format:"TEST format",created_at:"TEST date",result:i?{report:{exported_count:1,skipped_count:0,warnings:["TEST format omits attributes"]}}:null}))});});
  await page.route(`**${path}/TEST-export-0`,async r=>r.fulfill({json:{job:{id:"TEST-export-0",result:null,error:null},active:false}}));
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});await page.goto(`/projects/${p.project_id}/work?task=${task}`);await page.locator("summary").filter({hasText:"导出历史与兼容性报告"}).click();const history=page.getByRole("region",{name:"任务导出历史",exact:true});await expect(history.locator("article")).toHaveCount(20);await expect(history).toContainText("完成状态未确认");
  await history.locator("summary").filter({hasText:"兼容性报告"}).first().click();await expect(history.getByText("TEST format omits attributes",{exact:true}).first()).toBeVisible();await expect(history.getByRole("link",{name:"下载标注文件",exact:true})).toHaveCount(0);
  await history.getByRole("button",{name:"较早记录",exact:true}).click();await expect(history).toContainText("当前页没有导出记录");await history.getByRole("button",{name:"较新记录",exact:true}).click();await expect(history.locator("article")).toHaveCount(20);expect(writes).toEqual([]);
});
test("production rejects every old page through native UI without loading legacy modules or making writes",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const writes:string[]=[];const loaded:string[]=[];page.on("request",r=>{loaded.push(r.url());if(r.method()!=="GET")writes.push(r.url());});
  for(const path of ["/runs","/review/old","/trash","/workflows","/projects/old","/projects/old/build/test","/projects/old/build/pipeline","/projects/old/runs/r","/settings/models","/settings","/unknown?settings=providers&workspace_return=https://example.invalid","/projects/p/task/goal","/projects/p/task/images","/projects/p/task/model","/projects/p/task/samples","/projects/p/task/confirm","/projects/p/task/revise","/settings/registry","/settings/capabilities","/projects/p/export","/projects/p/review/r","/projects/p/batches/b"]){
    await page.goto(path);await expect(page.getByRole("heading",{name:"页面不存在",exact:true})).toBeVisible();await expect(page.locator(".ui-app")).toHaveCount(1);await expect(page.locator(".sidebar")).toHaveCount(0);await expect(page.locator(".composer textarea")).toHaveCount(0);
  }
  await page.reload();await expect(page.getByRole("heading",{name:"页面不存在",exact:true})).toBeVisible();await page.getByRole("link",{name:"返回项目列表",exact:true}).click();await expect(page.getByRole("heading",{name:"我的项目",exact:true})).toBeVisible();expect(writes).toEqual([]);
  expect(loaded.filter(u=>/\/src\/App\.tsx|\/src\/styles\.css|\/assets\/styles-/.test(u))).toEqual([]);
});
test("native Trash restores a selected Batch and its child as one exact owned operation",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const scope=(await(await request.get("/api/history-scope")).json()).scope;expect(scope).toBeTruthy();const nav=await(await request.get("/api/navigation")).json();const project=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture").project_id;
  const batches=await(await request.get(`/api/batches?project_id=${project}&history_scope=${scope.id}&limit=100&offset=0`)).json();const batch=batches.batches.find((b:{status:string;child_run_ids:string[]})=>b.status==="completed"&&b.child_run_ids.length);expect(batch,"requires the post-cutover completed TEST Sample Journey Batch").toBeTruthy();
  const before=await(await request.get(`/api/runs/${batch.child_run_ids[0]}/annotations`)).json();const headers={"x-annotagent-csrf":(await(await request.get("/api/session")).json()).csrf_token};
  const body={project_id:project,history_scope:scope.id,action:"move_to_trash",objects:[{kind:"batch",id:batch.id,expected_revision:batch.lifecycle_revision}],idempotency_key:randomUUID()};const base=`/api/projects/${project}/management`;
  const preview=await(await request.post(`${base}/preview?history_scope=${scope.id}`,{headers,data:body})).json();expect(preview.can_execute).toBe(true);
  const confirmation=await request.post("/api/session/privileged-confirmation",{headers,data:{action:`POST ${base}/actions`,confirmed:true}});expect(confirmation.ok()).toBeTruthy();
  const deleted=await request.post(`${base}/actions?history_scope=${scope.id}`,{headers:{...headers,"x-annotagent-privileged-confirmation":(await confirmation.json()).confirmation_token},data:{...body,confirmation_token:preview.confirmation_token}});expect(deleted.ok(),await deleted.text()).toBeTruthy();
  const trash=await(await request.get(`/api/projects/${project}/trash?history_scope=${scope.id}&limit=50&offset=0`)).json();const entries=trash.items.filter((e:{object:{id:string}})=>e.object.id===batch.id||batch.child_run_ids.includes(e.object.id));expect(entries.length).toBeGreaterThan(1);
  await page.goto(`/projects/${project}/manage/trash`);for(const e of entries)await page.getByRole("checkbox",{name:`选择 ${e.display_name}`,exact:true}).check();
  const sent=page.waitForRequest(r=>r.url().includes("/management/preview")&&r.method()==="POST");await page.getByRole("button",{name:"恢复选中项…",exact:true}).click();const normalized=(await sent).postDataJSON();expect(normalized.objects).toHaveLength(1);expect(normalized.objects[0]).toMatchObject({kind:"batch",id:batch.id});
  await page.getByRole("dialog").getByRole("button",{name:"恢复",exact:true}).click();await expect(page.getByRole("dialog")).toHaveCount(0);expect((await(await request.get(`/api/batches/${batch.id}`)).json()).batch.in_trash).toBe(false);expect(await(await request.get(`/api/runs/${batch.child_run_ids[0]}/annotations`)).json()).toEqual(before);
});
test("native Replay reads real unsupported model scope and never posts",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const runs=await(await request.get("/api/runs?limit=50")).json();const run=runs.runs.find((r:{project_name:string})=>r.project_name==="TEST Agent UI HTTP fixture");expect(run).toBeTruthy();
  const inspection=await(await request.get(`/api/runs/${run.id}/pipeline-artifacts`)).json();const node=inspection.nodes[0].node_id;
  const preview=await(await request.get(`/api/runs/${run.id}/replay/${encodeURIComponent(node)}?project_id=${run.project_id}`)).json();expect(preview.available).toBe(false);expect(preview.refusal_reasons).toContain("current_binding_replay_unsupported");
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});await page.goto(`/projects/${run.project_id}/manage/runs/${run.id}?view=debug&node=${encodeURIComponent(node)}`);
  const replay=page.getByRole("region",{name:"节点 Replay",exact:true});await replay.getByRole("button",{name:"检查重放范围",exact:true}).click();await expect(replay).toContainText("current_binding_replay_unsupported");await expect(replay.getByRole("button",{name:"确认重放范围…",exact:true})).toBeDisabled();await page.reload();await expect(replay.getByRole("button",{name:"检查重放范围",exact:true})).toBeVisible();expect(writes).toEqual([]);
});
test("controlled Replay transport restores a lost receipt without repeating execution",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const runs=await(await request.get("/api/runs?limit=50")).json();const run=runs.runs.find((r:{project_name:string})=>r.project_name==="TEST Agent UI HTTP fixture");const inspection=await(await request.get(`/api/runs/${run.id}/pipeline-artifacts`)).json();const node=inspection.nodes[0].node_id;const path=`/api/runs/${run.id}/replay/${encodeURIComponent(node)}`;
  const original=await(await request.get(`${path}?project_id=${run.project_id}`)).json();let saved:Record<string,unknown>|undefined;const sent:unknown[]=[];
  // Explicit browser transport fixture only. It does not invoke backend Replay.
  await page.route(`**${path}**`,async route=>{
    if(route.request().method()==="POST"){const body=route.request().postDataJSON();sent.push(body);saved={command_id:body.command_id,project_id:run.project_id,run_id:run.id,node_id:node,request:body,status:"completed",started_at:"TEST-time",completed_at:"TEST-time",failure:null,result:{source_run_id:run.id,replayed_from:node,sandbox:true,reexecuted_nodes:[node],preserved_upstream_nodes:[],inspection}};await route.abort();return;}
    if(route.request().url().includes("/commands/")){if(saved)await route.fulfill({json:saved});else await route.fulfill({status:404,json:{error:"TEST receipt not yet created"}});return;}
    await route.fulfill({json:{...original,available:true,refusal_reasons:[],limits:{maximum_model_requests:0,timeout_seconds:30,unknown_cost:false}}});
  });
  await page.goto(`/projects/${run.project_id}/manage/runs/${run.id}?view=debug&node=${encodeURIComponent(node)}`);const replay=page.getByRole("region",{name:"节点 Replay",exact:true});await replay.getByRole("button",{name:"检查重放范围",exact:true}).click();await replay.getByRole("button",{name:"确认重放范围…",exact:true}).click();await page.getByRole("dialog").getByRole("button",{name:"取消",exact:true}).click();expect(sent).toEqual([]);
  await replay.getByRole("button",{name:"确认重放范围…",exact:true}).click();await page.getByRole("dialog").getByRole("button",{name:"开始 Sandbox 重放",exact:true}).click();await expect.poll(()=>sent.length).toBe(1);await page.reload();await expect(replay).toContainText("已保存重放报告；不代表所有节点成功或标注被接受");expect(sent).toHaveLength(1);await replay.getByRole("button",{name:"读取原命令回执",exact:true}).click();await expect(replay).toContainText("实际执行");expect(sent).toHaveLength(1);
});
test("native Project call limit recovers its command without resetting newer quota or invoking models",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  await page.goto("/projects/new");await page.getByLabel("项目名称",{exact:true}).fill(`TEST call limit ${Date.now()}`);await page.getByRole("button",{name:"创建项目",exact:true}).click();await expect(page).toHaveURL(/\/manage\/data$/);
  const project=new URL(page.url()).pathname.split("/")[2];const url=`/api/projects/${project}/conversation-call-limit`;const original=await(await request.get(url)).json();const csrf=(await(await request.get("/api/session")).json()).csrf_token;
  const calls:string[]=[];const bodies:Record<string,unknown>[]=[];page.on("request",r=>{if(r.method()!=="GET")calls.push(r.url());});
  await page.getByRole("button",{name:"管理项目调用额度",exact:true}).click();const limit=page.getByRole("region",{name:"项目调用额度",exact:true});await expect(limit).toContainText("尚未配置累计上限");await limit.getByLabel("累计调用次数上限",{exact:true}).fill("10");await limit.getByRole("button",{name:"检查额度修改…",exact:true}).click();await page.getByRole("dialog").getByRole("button",{name:"取消",exact:true}).click();expect(calls).toEqual([]);
  await page.route(`**${url}`,async route=>{if(route.request().method()!=="POST"){await route.continue();return;}bodies.push(route.request().postDataJSON());const response=await route.fetch();expect(response.ok(),await response.text()).toBeTruthy();if(bodies.length===1){const value=await response.json();const newer=await request.post(url,{headers:{"x-annotagent-csrf":csrf},data:{id:randomUUID(),expected_revision:value.revision,maximum_calls:20}});expect(newer.ok(),await newer.text()).toBeTruthy();await route.abort();}else await route.fulfill({response});});
  await limit.getByRole("button",{name:"检查额度修改…",exact:true}).click();await page.getByRole("dialog").getByRole("button",{name:"保存累计上限",exact:true}).click();await expect(limit.getByRole("alert")).toBeVisible();expect(bodies).toHaveLength(1);
  page.on("dialog",d=>d.accept());await page.reload();await page.getByRole("button",{name:"管理项目调用额度",exact:true}).click();await expect(limit.getByRole("button",{name:"核实原额度修改",exact:true})).toBeVisible();expect(bodies).toHaveLength(1);await limit.getByRole("button",{name:"核实原额度修改",exact:true}).click();await expect(limit.getByLabel("累计调用次数上限",{exact:true})).toHaveValue("20");expect(bodies).toHaveLength(2);expect(bodies[1]).toEqual(bodies[0]);
  const after=await(await request.get(url)).json();expect(after).toEqual({revision:original.revision+2,maximum_calls:20,reserved_calls:original.reserved_calls});expect(calls.every(c=>c.endsWith("/conversation-call-limit"))).toBe(true);
});
test("native exact clone recovers lost receipt without overwriting edited copy or frozen source",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const nav=await(await request.get("/api/navigation")).json();const project=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture").project_id;const summary=await(await request.get(`/api/projects/${project}/summary`)).json();const version=summary.project.available_workflow_versions[0];const frozenUrl=`/api/projects/${project}/workflows/${version.workflow_id}/versions/${version.version}`;const frozen=await(await request.get(frozenUrl)).json();
  const csrf=(await(await request.get("/api/session")).json()).csrf_token;const bodies:unknown[]=[];let copyId="";
  await page.route(`**/api/workflows/${version.workflow_id}/versions/${version.version}/clone`,async route=>{
    bodies.push(route.request().postDataJSON());const response=await route.fetch();expect(response.status()).toBe(201);const receipt=await response.json();copyId=receipt.id;
    if(bodies.length===1){const edited=await request.patch(`/api/workflow-drafts/${copyId}`,{headers:{"x-annotagent-csrf":csrf,"if-match":String(receipt.revision)},data:{...receipt,name:"TEST copy edited after uncertain response"}});expect(edited.ok(),await edited.text()).toBeTruthy();await route.abort();}else await route.fulfill({response});
  });
  await page.goto(`/projects/${project}/manage/pipelines/${version.workflow_id}?version=${version.version}`);const clone=page.getByRole("region",{name:"复制 Workflow 版本",exact:true});await clone.getByRole("button",{name:"复制此版本…",exact:true}).click();await page.getByRole("dialog").getByRole("button",{name:"取消",exact:true}).click();expect(bodies).toHaveLength(0);
  await clone.getByRole("button",{name:"复制此版本…",exact:true}).click();await page.getByRole("dialog").getByRole("button",{name:"确认复制",exact:true}).click();await expect(clone.getByRole("alert")).toBeVisible();expect(bodies).toHaveLength(1);
  await page.reload();await expect(clone.getByRole("button",{name:"核实原复制请求",exact:true})).toBeVisible();expect(bodies).toHaveLength(1);await clone.getByRole("button",{name:"核实原复制请求",exact:true}).click();await expect(clone.getByRole("status")).toContainText("TEST copy edited after uncertain response");expect(bodies).toHaveLength(2);expect(bodies[1]).toEqual(bodies[0]);expect(bodies[0]).toMatchObject({project_id:project,source_snapshot_hash:frozen.content_hash});
  const current=await(await request.get(`/api/workflow-drafts/${copyId}?project_id=${project}`)).json();expect(current.name).toBe("TEST copy edited after uncertain response");expect(current.revision).toBeGreaterThan(1);expect(await(await request.get(frozenUrl)).json()).toEqual(frozen);
  await clone.getByRole("link",{name:"打开当前副本",exact:true}).click();await expect(page).toHaveURL(new RegExp(`/manage/pipelines/${copyId}$`));
});
test("native human geometry creation freezes requests and recovers without duplicate annotations",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const runs=await(await request.get("/api/runs?limit=50")).json();const run=runs.runs.find((r:{project_name:string})=>r.project_name==="TEST Agent UI HTTP fixture");expect(run).toBeTruthy();
  const headers={"x-annotagent-csrf":(await(await request.get("/api/session")).json()).csrf_token};
  let summary=await(await request.get(`/api/projects/${run.project_id}/summary`)).json();
  if(!summary.project.annotation_schema.some((t:{display_name:string})=>t.display_name==="TEST Manual Box")){
    const task=await request.post(`/api/projects/${run.project_id}/schema/tasks`,{headers,data:{display_name:"TEST Manual Box",kind:"bounding_box",labels:["TEST_manual_box"],attributes:{}}});expect(task.ok(),await task.text()).toBeTruthy();
    summary=await(await request.get(`/api/projects/${run.project_id}/summary`)).json();
  }
  const base=(await(await request.get(`/api/runs/${run.id}/annotations`)).json()).annotations[0];expect(base).toBeTruthy();
  const seedId=randomUUID();const seed=await request.post(`/api/runs/${run.id}/annotations`,{headers,data:{annotation:{...base,id:seedId,label:`${base.label} TEST`,source:"human",review_status:"needs_review",created_at:new Date().toISOString()}}});expect(seed.ok(),await seed.text()).toBeTruthy();
  await page.goto(`/projects/${run.project_id}/manage/review/${seedId}`);await page.getByRole("button",{name:"补充遗漏标注…",exact:true}).click();const creation=page.locator(".native-human-annotation");await creation.getByRole("button",{name:/新增 TEST Manual Box/}).click();
  await expect(creation.getByLabel("新增标注类别",{exact:true})).toHaveValue("TEST_manual_box");
  const move=creation.getByRole("button",{name:"Move box with arrow keys",exact:true});await move.focus();await move.press("ArrowRight");
  await creation.screenshot({path:"/tmp/annotagent-native-human-creation.png"});
  const bodies:unknown[]=[];await page.route(`**/api/runs/${run.id}/annotations`,async route=>{if(route.request().method()!=="POST")return route.continue();bodies.push(route.request().postDataJSON());const response=await route.fetch();expect(response.ok()).toBeTruthy();if(bodies.length===1)await route.abort();else await route.fulfill({response});});
  await creation.getByRole("button",{name:"保存新增标注",exact:true}).click();await expect(creation.getByRole("alert")).toBeVisible();await expect(creation.getByLabel("新增标注类别",{exact:true})).toBeDisabled();
  page.on("dialog",d=>d.accept());await page.reload();await page.getByRole("button",{name:"补充遗漏标注…",exact:true}).click();await expect(creation.getByRole("button",{name:"使用原请求重试保存",exact:true})).toBeVisible();expect(bodies).toHaveLength(1);
  await creation.getByRole("button",{name:"使用原请求重试保存",exact:true}).click();await expect(creation.getByRole("status")).toContainText("新增标注已保存");expect(bodies).toHaveLength(2);expect(bodies[1]).toEqual(bodies[0]);
  const body=bodies[0] as {annotation:{id:string;value:{rect:number[]}}};expect(body.annotation.value.rect[0]).toBeGreaterThan(0.35);const all=(await(await request.get(`/api/runs/${run.id}/annotations`)).json()).annotations;expect(all.filter((a:{id:string})=>a.id===body.annotation.id)).toHaveLength(1);
  const original=await(await request.get(`/api/projects/${run.project_id}/reviews/${seedId}`)).json();expect(original.annotation.review_status).toBe("needs_review");
  await creation.getByRole("link",{name:"审核新增对象",exact:true}).click();await expect(page).toHaveURL(new RegExp(`/manage/review/${body.annotation.id}[?]queue_offset=0$`));await expect(page.getByLabel("标签",{exact:true})).toHaveValue("TEST_manual_box");
  // Leave this isolated fixture's queue clean for the other Review scenario.
  for(const id of [seedId,body.annotation.id]){const result=await request.post(`/api/projects/${run.project_id}/reviews/${id}/accept-and-next`,{headers,data:{decision:"accept",reason_code:"accepted_as_is",note:"TEST human creation verified"}});expect(result.ok(),await result.text()).toBeTruthy();}
});
test("native publication cancels without writes and rejects a changed confirmed revision",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const nav=await(await request.get("/api/navigation")).json();const project=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture").project_id;
  const session=await(await request.get("/api/session")).json();const headers={"x-annotagent-csrf":session.csrf_token};const created=await request.post("/api/workflow-drafts",{headers,data:{project_id:project,from_template:false}});expect(created.ok()).toBeTruthy();const draft=await created.json();
  const publications:Record<string,unknown>[]=[];page.on("request",r=>{if(new URL(r.url()).pathname.endsWith("/publish"))publications.push(r.postDataJSON());});await page.goto(`/projects/${project}/manage/pipelines/${draft.id}`);const section=page.getByRole("region",{name:"发布 Workflow",exact:true});await section.getByRole("button",{name:"预览发布确认…"}).click();await expect(page.getByRole("dialog")).toContainText("不创建 Run");await page.getByRole("dialog").getByRole("button",{name:"取消",exact:true}).click();expect(publications).toEqual([]);
  await section.getByRole("button",{name:"预览发布确认…"}).click();const changed=await request.patch(`/api/workflow-drafts/${draft.id}`,{headers:{...headers,"if-match":String(draft.revision)},data:{...draft,name:"TEST newer revision"}});expect(changed.ok(),await changed.text()).toBeTruthy();await page.getByRole("dialog").getByRole("button",{name:"确认发布",exact:true}).click();await expect(section.getByRole("alert")).toContainText("changed");expect(publications).toHaveLength(1);expect(publications[0]).toMatchObject({project_id:project,expected_revision:draft.revision,expected_content_hash:draft.content_hash});await page.reload();await expect(section.getByRole("button",{name:"预览发布确认…"})).toBeVisible();expect(publications).toHaveLength(1);
});
test("native probe usage is lazy and does not generate billable requests",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto("/settings/usage");const region=page.getByRole("region",{name:"模型探测用量",exact:true});await expect(region).toHaveCount(0);await page.locator("summary").filter({hasText:"模型探测用量记录"}).click();await expect(region).toContainText("不是全系统费用");await expect(region).toContainText("当前匹配");await expect(region).toContainText("TEST");await region.getByRole("textbox",{name:"搜索模型",exact:true}).fill("TEST-NO-SUCH-MODEL");await expect(region).toContainText("当前匹配 0 条");await page.reload();await expect(region).toHaveCount(0);expect(writes).toEqual([]);
});
test("native Provider controls read status and cancel discovery without remote calls",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const providers=(await(await request.get("/api/providers")).json()).providers;const provider=providers.find((p:{adapter:string;enabled:boolean})=>p.adapter==="open_ai_compatible"&&p.enabled);expect(provider).toBeTruthy();
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto("/settings/providers");await page.locator(".settings-row").filter({has:page.getByText(provider.display_name,{exact:true})}).getByRole("button",{name:"编辑",exact:true}).click();
  await page.getByRole("combobox",{name:"凭证存储",exact:true}).selectOption("environment_variable");await page.getByRole("textbox",{name:"环境变量名称",exact:true}).fill("not-a-variable-name");await expect(page.getByRole("button",{name:"保存新凭证",exact:true})).toBeDisabled();await page.getByRole("textbox",{name:"环境变量名称",exact:true}).fill("TEST_EXISTING_SERVER_VARIABLE");await expect(page.getByRole("button",{name:"保存新凭证",exact:true})).toBeEnabled();await page.getByRole("combobox",{name:"凭证存储",exact:true}).selectOption("workspace_file");await expect(page.getByLabel("替换 API Key（只写）",{exact:true})).toHaveValue("");
  await page.locator("summary").filter({hasText:"连接状态与模型发现"}).click();const region=page.getByRole("region",{name:"Provider 高级控制"});await expect(region).toContainText(provider.endpoint_summary);
  await region.getByRole("button",{name:"发现模型…"}).click();await expect(page.getByRole("dialog")).toContainText("不发送图片");await page.getByRole("dialog").getByRole("button",{name:"取消",exact:true}).click();
  await region.getByRole("button",{name:"停用连接…"}).click();await page.getByRole("dialog").getByRole("button",{name:"取消",exact:true}).click();
  await region.getByRole("button",{name:"刷新连接状态"}).click();await expect(region.getByRole("button",{name:"发现模型…"})).toBeEnabled();expect(writes).toEqual([]);
  await page.screenshot({path:"/tmp/annotagent-native-provider-controls.png",fullPage:true});
});
test("native Provider presets fill only the unsaved account form",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const presets=(await(await request.get("/api/provider-presets")).json()).presets;const preset=presets.find((p:{adapter:string})=>p.adapter==="open_ai_compatible");expect(preset).toBeTruthy();
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});await page.goto("/settings/providers");await page.getByRole("button",{name:"添加 Provider",exact:true}).click();await page.getByRole("combobox",{name:"连接预设",exact:true}).selectOption(preset.id);await expect(page.getByRole("textbox",{name:"显示名称",exact:true})).toHaveValue(preset.display_name);await expect(page.getByRole("textbox",{name:"Endpoint",exact:true})).toHaveValue(preset.base_url);await page.getByRole("button",{name:"取消账户编辑",exact:true}).click();expect(writes).toEqual([]);
});
test("native global default editor reads real compatible models and cancels without writes",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const base=await(await request.get("/api/agent-model-bindings")).json();
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});await page.goto("/settings/agent-models");await page.locator("summary").filter({hasText:"全局默认模型"}).click();
  const region=page.getByRole("region",{name:"全局默认模型配置",exact:true});const select=region.getByRole("combobox",{name:"Pipeline Builder 默认模型",exact:true});await expect(select).toHaveValue(base.pipeline_builder||"");
  const options=await select.locator("option").evaluateAll(nodes=>nodes.map(n=>(n as HTMLOptionElement).value));const different=options.find(value=>value!==(base.pipeline_builder||""));expect(different).toBeDefined();await select.selectOption(different!);await expect(region.getByRole("button",{name:"保存全局默认",exact:true})).toBeEnabled();await region.getByRole("button",{name:"取消默认模型修改",exact:true}).click();await expect(select).toHaveValue(base.pipeline_builder||"");expect(writes).toEqual([]);await select.selectOption(different!);await region.getByRole("button",{name:"保存全局默认",exact:true}).click();await expect(region.getByRole("status")).toContainText("全局默认已保存");expect((await(await request.get("/api/agent-model-bindings")).json()).pipeline_builder||"").toBe(different);await select.selectOption(base.pipeline_builder||"");await region.getByRole("button",{name:"保存全局默认",exact:true}).click();await expect(region.getByRole("button",{name:"保存全局默认",exact:true})).toBeDisabled();await expect.poll(async()=>await(await request.get("/api/agent-model-bindings")).json()).toEqual(base);await page.reload();expect(writes).toHaveLength(2);expect(writes.every(url=>url.endsWith("/api/agent-model-bindings"))).toBe(true);
});
test("native frozen version restores exact snapshot and rejects missing versions without writes",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const nav=await(await request.get("/api/navigation")).json();const project=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture").project_id;const summary=await(await request.get(`/api/projects/${project}/summary`)).json();const version=summary.project.available_workflow_versions[0];expect(version).toBeTruthy();const frozen=await(await request.get(`/api/projects/${project}/workflows/${version.workflow_id}/versions/${version.version}`)).json();
  const url=`/projects/${project}/manage/pipelines/${encodeURIComponent(version.workflow_id)}?version=${version.version}`;const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto(url);await expect(page.getByRole("heading",{name:`${frozen.draft.name} · v${version.version}`,exact:true})).toBeVisible();await page.reload();await expect(page.getByRole("region",{name:"Workflow 版本详情",exact:true})).toContainText("不是当前可编辑 Draft");await page.locator("summary").filter({hasText:"完整发布对象（只读）"}).click();await expect(page.locator("pre").filter({hasText:frozen.published_at})).toHaveText(JSON.stringify(frozen,null,2));
  await page.locator("summary").filter({hasText:"比较另一个已发布版本"}).click();const comparison=page.getByRole("region",{name:"冻结版本比较",exact:true});await comparison.getByRole("textbox",{name:"比较版本",exact:true}).fill(String(version.version));await comparison.getByRole("button",{name:"读取并比较"}).click();await expect(comparison.getByRole("status")).toContainText("0 个字段组不同");await page.reload();await expect(comparison.getByRole("status")).toContainText("0 个字段组不同");await comparison.getByRole("textbox",{name:"比较版本",exact:true}).fill("999999");await comparison.getByRole("button",{name:"读取并比较"}).click();await expect(comparison.getByRole("alert")).toBeVisible();await expect(comparison.getByRole("status")).toHaveCount(0);
  await page.goto(url.replace(`version=${version.version}`,"version=0001"));await expect(page.getByRole("alert")).toContainText("无效的 Workflow 版本号");expect(writes).toEqual([]);
});
test("native Skill registry is lazy, searchable, server-backed and read-only",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");const skills=await(await request.get("/api/skills")).json();expect(skills.length).toBeGreaterThan(0);
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});await page.goto("/settings/plugins");
  const region=page.getByRole("region",{name:"Skill 注册表",exact:true});await expect(region).toHaveCount(0);
  const toggle=page.locator("summary").filter({hasText:"Skills、工具与领域规则"});await toggle.click();await expect(region.getByRole("textbox",{name:"搜索 Skill",exact:true})).toBeVisible();
  await region.getByRole("textbox",{name:"搜索 Skill",exact:true}).fill(skills[0].id);await expect(region.locator("summary").filter({hasText:skills[0].display_name}).first()).toBeVisible();await region.locator("summary").filter({hasText:skills[0].display_name}).first().click();await expect(region).toContainText(skills[0].description);
  await region.getByRole("textbox",{name:"搜索 Skill",exact:true}).fill("TEST-no-such-skill");await expect(region).toContainText("没有匹配的 Skill");await toggle.click();await expect(region).toHaveCount(0);await page.reload();await expect(region).toHaveCount(0);expect(writes).toEqual([]);
});
test("native image removal requires explicit filename and only deletes an isolated TEST copy",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  await page.goto("/projects/new");await page.getByLabel("项目名称",{exact:true}).fill(`TEST removal ${randomUUID()}`);await page.getByRole("button",{name:"创建项目",exact:true}).click();await expect(page).toHaveURL(/\/manage\/data$/);
  await page.getByLabel("选择图片",{exact:true}).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));await page.getByRole("button",{name:"上传 1 张图片",exact:true}).click();await expect(page.getByRole("heading",{name:"已保存图片 · 1",exact:true})).toBeVisible();
  const project=new URL(page.url()).pathname.split("/")[2];const image=(await(await request.get(`/api/projects/${project}/images`)).json()).images[0];
  const deletes:string[]=[];page.on("request",r=>{if(r.method()==="DELETE")deletes.push(r.url());});
  await page.getByRole("button",{name:`移除 ${image.name}…`,exact:true}).click();const dialog=page.getByRole("dialog",{name:"永久移除项目图片文件",exact:true});await expect(dialog.getByRole("button",{name:"永久移除文件",exact:true})).toBeDisabled();await dialog.getByRole("button",{name:"取消",exact:true}).click();expect(deletes).toEqual([]);
  await page.getByRole("button",{name:`移除 ${image.name}…`,exact:true}).click();await dialog.getByRole("textbox",{name:"输入完整文件名确认",exact:true}).fill(image.name);await dialog.getByRole("button",{name:"永久移除文件",exact:true}).click();await expect(dialog).toHaveCount(0);await expect(page.getByRole("heading",{name:"已保存图片 · 0",exact:true})).toBeVisible();await page.reload();expect(deletes).toHaveLength(1);expect(deletes[0]).toContain(`expected_content_hash=${image.content_hash}`);
});
test("controlled mask response fixture renders exact RLE pixels and original toggle removes overlay",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const run="4d348027-ff6e-4241-a46c-1c2b6c7aaaef";const endpoint=`/api/runs/${run}/pipeline-artifacts`;
  const inspection=await(await request.get(endpoint)).json();const node=inspection.nodes[0];const source=node.outputs[0].artifact;
  node.outputs=[{kind:"mask_set",artifact:{image_id:inspection.image_id,reference:{artifact_id:"TEST-mask"},masks:[{mask_id:"TEST-pixel",mask:{encoding:"coco_rle",width:source.width,height:source.height,counts:`0 1 ${source.width*source.height-1}`}}]}}];
  await page.route(`**${endpoint}`,route=>route.fulfill({json:inspection}));const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto(`/projects/${inspection.project_id}/manage/runs/${run}?view=debug&node=${encodeURIComponent(node.node_id)}&artifact=TEST-mask`);
  const region=page.getByRole("region",{name:"中间产物预览",exact:true});const canvas=region.locator("canvas");await expect(canvas).toBeVisible();
  // Browser canvas round-trips premultiplied alpha; tolerate one RGB level, not geometry/alpha drift.
  const expected=[24,153,171,82,0,0,0,0];
  await expect.poll(async()=>{const pixels=await canvas.evaluate((c:HTMLCanvasElement)=>Array.from(c.getContext("2d")!.getImageData(0,0,2,1).data));return pixels.every((v,i)=>Math.abs(v-expected[i])<=(i<3?1:0));}).toBe(true);
  await region.getByRole("button",{name:"只看原图",exact:true}).click();await expect(canvas).toHaveCount(0);expect(writes).toEqual([]);
});
test("native Artifact deep link shows its actual input image and refresh remains passive",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const run="4d348027-ff6e-4241-a46c-1c2b6c7aaaef";
  const inspection=await(await request.get(`/api/runs/${run}/pipeline-artifacts`)).json();const node=inspection.nodes.find((n:{operation:string})=>n.operation==="core.image_input");
  const artifact=node.outputs[0].artifact.reference.artifact_id;
  const url=`/projects/${inspection.project_id}/manage/runs/${run}?view=debug&node=${encodeURIComponent(node.node_id)}&artifact=${encodeURIComponent(artifact)}`;
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});await page.goto(url);
  const region=page.getByRole("region",{name:"中间产物预览",exact:true});await expect(region.getByRole("img")).toHaveAttribute("src",`/api/projects/${inspection.project_id}/images/${inspection.image_id}/content`);
  await page.reload();await expect(region).toBeVisible();expect(writes).toEqual([]);
  await page.goto(url.replace(encodeURIComponent(artifact),"missing-artifact"));await expect(page.getByText("所链接的产物不在该节点中，没有自动替换。",{exact:true})).toBeVisible();await expect(region).toHaveCount(0);
});
test("native static validation checks the saved revision without execution and invalidates evidence on edits",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const project="TEST-agent-ui-15eb0549-f44e-4ae1-81dd-0ebf67714eb2";
  const session=await(await request.get("/api/session")).json();
  const created=await request.post("/api/workflow-drafts",{headers:{"x-annotagent-csrf":session.csrf_token},data:{project_id:project,from_template:false}});expect(created.ok()).toBeTruthy();const draft=await created.json();
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto(`/projects/${project}/manage/pipelines/${draft.id}`);
  const check=page.getByRole("button",{name:"校验已保存草稿",exact:true});await expect(check).toBeEnabled();expect(writes).toEqual([]);
  const responsePromise=page.waitForResponse(r=>r.url().endsWith(`/api/workflow-drafts/${draft.id}/validate`));await check.click();const response=await responsePromise;expect(response.ok()).toBeTruthy();const report=await response.json();
  const region=page.getByRole("region",{name:"静态校验",exact:true});await expect(region.getByRole("status")).toContainText(report.validation.valid?"静态校验通过":"静态校验未通过");
  expect(report.revision).toBe(draft.revision);expect(report.content_hash).toBe(draft.content_hash);
  await page.getByLabel("方案名称",{exact:true}).fill("TEST unsaved static edit");await expect(check).toBeDisabled();await expect(region.getByRole("status")).toHaveCount(0);
  await page.getByRole("button",{name:"取消修改",exact:true}).click();await page.reload();await expect(check).toBeEnabled();await expect(region.getByRole("status")).toHaveCount(0);
  expect(writes).toEqual([`http://127.0.0.1:8794/api/workflow-drafts/${draft.id}/validate`]);
  const saved=await(await request.get(`/api/workflow-drafts/${draft.id}?project_id=${project}`)).json();expect(saved).toEqual(draft);
});
test("native Pipeline step dialog preserves invalid input then saves only a cloned TEST draft",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const project="TEST-agent-ui-15eb0549-f44e-4ae1-81dd-0ebf67714eb2";
  const original=(await(await request.get(`/api/workflow-drafts?project_id=${project}`)).json()).drafts.find((d:{status:string;label_pipeline:unknown})=>d.status==="published"&&d.label_pipeline);expect(original).toBeTruthy();
  const session=await(await request.get("/api/session")).json();const clone=await request.post(`/api/workflows/${original.id}/versions/1/clone`,{headers:{"x-annotagent-csrf":session.csrf_token},data:{}});expect(clone.ok(),await clone.text()).toBeTruthy();const draft=await clone.json();
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto(`/projects/${project}/manage/pipelines/${draft.id}`);
  const group=draft.label_pipeline.label_pipelines[0];await page.locator("summary").filter({hasText:`${group.target_task_id} · ${group.target_label}`}).click();
  await page.getByRole("button",{name:"编辑步骤",exact:true}).first().click();
  const dialog=page.getByRole("dialog",{name:"编辑 Pipeline 步骤",exact:true});
  const parameters=dialog.getByRole("textbox",{name:"步骤参数 JSON",exact:true});await parameters.fill("{");
  await dialog.getByRole("button",{name:"应用到未保存草稿",exact:true}).click();await expect(dialog.getByRole("alert")).toBeVisible();await expect(parameters).toHaveValue("{");expect(writes).toEqual([]);
  const changed={...group.steps[0].parameters,TEST_edit_marker:"native-dialog"};await parameters.fill(JSON.stringify(changed));
  await dialog.getByRole("button",{name:"应用到未保存草稿",exact:true}).click();await expect(dialog).toHaveCount(0);expect(writes).toEqual([]);
  await page.getByRole("button",{name:"保存草稿",exact:true}).click();await expect(page.getByRole("status")).toContainText("草稿已保存");await page.reload();
  await expect(page.getByRole("textbox",{name:"执行配置 JSON",exact:true})).toHaveValue(/native-dialog/);
  expect(writes).toEqual([`http://127.0.0.1:8794/api/workflow-drafts/${draft.id}`]);
  const after=(await(await request.get(`/api/workflow-drafts?project_id=${project}`)).json()).drafts.find((d:{id:string})=>d.id===original.id);expect(after).toEqual(original);
});
test("native workflow editor saves an exact TEST draft and never publishes or runs",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const projects=await(await request.get("/api/projects")).json();const project=projects.projects.find((p:{name:string})=>p.name.startsWith("TEST clean-cut"));expect(project).toBeTruthy();
  const session=await(await request.get("/api/session")).json();const response=await request.post("/api/workflow-drafts",{headers:{"x-annotagent-csrf":session.csrf_token},data:{project_id:project.id,from_template:false}});expect(response.ok()).toBeTruthy();const draft=await response.json();
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto(`/projects/${project.id}/manage/pipelines/${draft.id}`);
  const name=`TEST native editor ${randomUUID()}`;await page.getByLabel("方案名称",{exact:true}).fill(name);
  await page.getByRole("combobox",{name:"添加 Registry 节点",exact:true}).selectOption("core.image_input");
  await page.getByRole("button",{name:"加入未连接的步骤",exact:true}).click();
  await expect(page.getByRole("textbox",{name:"执行配置 JSON",exact:true})).toHaveValue(/core.image_input/);
  await page.getByRole("button",{name:"保存草稿",exact:true}).click();await expect(page.getByRole("status")).toContainText("草稿已保存");
  await page.reload();await expect(page.getByLabel("方案名称",{exact:true})).toHaveValue(name);
  expect(writes).toEqual([`http://127.0.0.1:8794/api/workflow-drafts/${draft.id}`]);
  await page.goto(`/projects/wrong-owner/manage/pipelines/${draft.id}`);await expect(page.getByRole("alert")).toBeVisible();await expect(page.getByLabel("方案名称",{exact:true})).toHaveCount(0);
});
test("native bundle import rejects a real invalid TEST package without installing",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto("/settings/vision-models");await page.getByText("导入本地模型包",{exact:true}).first().click();
  const area=page.getByRole("region",{name:"导入本地模型包",exact:true});
  await area.getByLabel("选择模型包",{exact:true}).setInputFiles({name:"TEST-invalid-model-bundle.zip",mimeType:"application/zip",buffer:Buffer.from("TEST invalid bundle; intentionally not a zip")});
  expect(writes).toEqual([]);
  await area.getByRole("button",{name:"检查模型包…",exact:true}).click();
  let dialog=page.getByRole("dialog",{name:"确认上传检查模型包",exact:true});await dialog.getByRole("button",{name:"取消",exact:true}).click();expect(writes).toEqual([]);
  await area.getByRole("button",{name:"检查模型包…",exact:true}).click();dialog=page.getByRole("dialog",{name:"确认上传检查模型包",exact:true});await dialog.getByRole("button",{name:"确认操作",exact:true}).click();
  await expect(area.getByRole("alert")).toContainText("未自动重试");
  await expect(area).toContainText("TEST-invalid-model-bundle.zip");
  expect(writes.length).toBe(1);expect(writes[0]).toContain("/api/model-bundles/packages/inspect");
  await expect(area.getByRole("button",{name:"导入已检查的模型包…",exact:true})).toHaveCount(0);
});
test("controlled pending Batch requires confirmation and rechecks ownership before resume",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const batches=await(await request.get("/api/batches?limit=100")).json();const batch=batches.batches[0];const path=`/api/batches/${batch.id}`;const original=await(await request.get(path)).json();let foreign=false;const writes:string[]=[];
  // Explicit read-response fixture only; no real batch is resumed or mutated.
  await page.route(`**${path}`,async route=>route.fulfill({json:{...original,batch:{...original.batch,status:"pending",in_trash:false,project_id:foreign?"TEST-foreign":batch.project_id}}}));
  page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});await page.goto(`/projects/${batch.project_id}/manage/batches/${batch.id}`);
  const resume=page.getByRole("button",{name:"继续批次",exact:true});await expect(resume).toBeEnabled();page.once("dialog",dialog=>dialog.dismiss());await resume.click();expect(writes).toEqual([]);
  foreign=true;page.once("dialog",dialog=>dialog.accept());await resume.click();await expect(page.getByRole("alert")).toContainText("不属于当前项目");expect(writes).toEqual([]);
});
test("native Batch detail keeps project ownership and passive refresh",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const batches=await(await request.get("/api/batches?limit=100")).json();const batch=batches.batches[0];expect(batch).toBeTruthy();
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto(`/projects/${batch.project_id}/manage/batches/${batch.id}`);
  await expect(page.getByRole("heading",{name:"批量处理",exact:true})).toBeVisible();
  await expect(page.getByLabel("筛选图片状态")).toBeVisible();
  await page.getByLabel("筛选图片状态").selectOption("all");await page.reload();
  await expect(page.getByLabel("筛选图片状态")).toHaveValue("all");
  const links=page.getByRole("link",{name:"查看图片结果",exact:true});
  if(await links.count())await expect(links.first()).toHaveAttribute("href",new RegExp(`/projects/${batch.project_id}/manage/runs/`));
  await page.goto(`/projects/wrong-owner/manage/batches/${batch.id}`);await expect(page.getByRole("alert")).toContainText("不属于当前项目");
  await expect(page.getByLabel("筛选图片状态")).toHaveCount(0);expect(writes).toEqual([]);
});
test("native HTTP worker area reads the real empty registry without discovery", async ({page,request}) => {
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto("/settings/vision-models");
  await page.getByText("HTTP Vision 协议绑定",{exact:true}).click();
  const area=page.getByRole("region",{name:"HTTP Vision 模型绑定",exact:true});
  await expect(area).toContainText("尚无 HTTP Vision 模型绑定");
  await area.getByRole("button",{name:"重新读取模型绑定",exact:true}).click();
  await expect(area).toContainText("尚无 HTTP Vision 模型绑定");expect(writes).toEqual([]);
});
test("controlled Worker response fixture verifies confirmation, failure and no automatic retry", async ({page,request}) => {
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  // UI-only response fixture: deliberately not evidence of real worker discovery.
  const worker={id:"TEST-worker-ui",model:"TEST response fixture",scope:"workspace_worker",role:"detector",endpoint:"http://127.0.0.1:1/TEST-only",health_status:"unknown",availability_group:"configured_unavailable"};
  let calls=0;
  await page.route("**/api/models",r=>r.fulfill({json:{models:[worker]}}));
  await page.route("**/api/models/TEST-worker-ui/test",r=>{calls++;return r.fulfill({json:{model_id:worker.id,passed:false,availability:"unreachable",failed_stage:"health",error:"TEST explicit discovery failure"}});});
  await page.goto("/settings/vision-models");await page.getByText("HTTP Vision 协议绑定",{exact:true}).click();
  const area=page.getByRole("region",{name:"HTTP Vision 模型绑定",exact:true});
  await area.getByRole("button",{name:"发现检查…",exact:true}).click();
  let dialog=page.getByRole("dialog",{name:"确认 HTTP Vision 发现检查",exact:true});
  await expect(dialog.getByRole("button",{name:"确认发现检查",exact:true})).toBeDisabled();
  await dialog.getByRole("button",{name:"取消",exact:true}).click();expect(calls).toBe(0);
  await area.getByRole("button",{name:"发现检查…",exact:true}).click();dialog=page.getByRole("dialog",{name:"确认 HTTP Vision 发现检查",exact:true});
  await dialog.getByRole("checkbox").check();await dialog.getByRole("button",{name:"确认发现检查",exact:true}).click();
  await expect(area.getByRole("status")).toContainText("检查未通过：health");expect(calls).toBe(1);
  await page.reload();expect(calls).toBe(1);
});
test("native model CAS keeps a stale editor without overwriting the winning writer",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const providers=await(await request.get("/api/providers")).json();
  const session=await(await request.get("/api/session")).json();
  const headers={"x-annotagent-csrf":session.csrf_token};
  const name=`TEST CAS ${randomUUID()}`;
  const created=await request.post("/api/model-profiles",{headers,data:{provider_id:providers.providers[0].id,display_name:name,remote_model_id:"test-no-call",input_modalities:["text"],task_capabilities:["text_generation"]}});
  expect(created.ok()).toBeTruthy();const model=await created.json();
  await page.goto("/settings/agent-models");const region=page.getByRole("region",{name:"模型配置",exact:true});
  await region.getByLabel("搜索模型",{exact:true}).fill(name);await region.getByRole("button",{name:"编辑配置",exact:true}).click();
  const dialog=page.getByRole("dialog",{name:"编辑模型配置",exact:true});
  await dialog.getByLabel("显示名称",{exact:true}).fill(`${name} local edit`);
  let patches=0;
  await page.route(`**/api/model-profiles/${model.id}`,async route=>{
    if(route.request().method()!=="PATCH"){await route.continue();return;}
    patches++;expect(route.request().postDataJSON().expected_revision).toBe(model.revision);
    const winner=await request.patch(`/api/model-profiles/${model.id}`,{headers,data:{expected_revision:model.revision,display_name:`${name} winner`}});
    expect(winner.ok(),await winner.text()).toBeTruthy();await route.continue();
  });
  await dialog.getByRole("button",{name:"保存配置",exact:true}).click();
  await expect(dialog.getByRole("alert")).toContainText("changed");
  await expect(dialog.getByLabel("显示名称",{exact:true})).toHaveValue(`${name} local edit`);
  expect(patches).toBe(1);
  const latest=await(await request.get(`/api/model-profiles/${model.id}`)).json();expect(latest.model.display_name).toBe(`${name} winner`);expect(latest.model.revision).toBe(model.revision+1);
});
test("native Run deep links preserve ownership and refresh without execution",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const runs=await(await request.get("/api/runs?limit=100")).json();
  const run=runs.runs.find((r:{project_name:string})=>r.project_name==="TEST Agent UI HTTP fixture");expect(run).toBeTruthy();
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  const href=`/projects/${run.project_id}/manage/runs/${run.id}`;
  await page.goto(href);await expect(page.getByRole("heading",{name:"运行结果",exact:true})).toBeVisible();
  await expect(page.locator(".native-review svg image")).toHaveAttribute("href",/^\/api\//);
  await expect(page.getByRole("button",{name:"继续运行",exact:true})).toHaveCount(0);
  await page.getByRole("button",{name:"查看原图",exact:true}).click();
  await expect(page.getByRole("button",{name:"显示标注",exact:true})).toBeVisible();
  await page.reload();await expect(page.locator(".workspace-header")).toContainText(run.project_name);
  await expect(page.getByRole("button",{name:"显示标注",exact:true})).toBeVisible();
  await page.getByRole("button",{name:"查看执行详情",exact:true}).click();
  const inspector=page.getByRole("region",{name:"节点与 Artifact 检查",exact:true});
  await expect(inspector.getByLabel("选择实际执行节点")).toBeVisible();
  const options=await inspector.getByLabel("选择实际执行节点").locator("option").evaluateAll(options=>options.map(o=>(o as HTMLOptionElement).value).filter(Boolean));
  expect(options.length).toBeGreaterThan(0);
  await inspector.getByLabel("选择实际执行节点").selectOption(options[0]);
  await expect(inspector.getByText("节点配置",{exact:true})).toBeVisible();
  await page.reload();await expect(inspector.getByLabel("选择实际执行节点")).toHaveValue(options[0]);
  await page.getByRole("button",{name:"返回结果画布",exact:true}).click();
  await expect(page.locator(".native-review svg image")).toBeVisible();
  const other=runs.runs.find((r:{project_id:string})=>r.project_id!==run.project_id);expect(other).toBeTruthy();
  await page.goto(`/projects/${other.project_id}/manage/runs/${run.id}`);
  await expect(page.getByRole("alert")).toContainText("不属于当前项目");
  await expect(page.locator(".native-review svg image")).toHaveCount(0);expect(writes).toEqual([]);
});
test("native model management locks, unlocks and explicitly deletes only a TEST profile",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const models=await(await request.get("/api/model-profiles")).json();
  const source=models.models.find((m:{locked:boolean})=>!m.locked);expect(source).toBeTruthy();
  const session=await(await request.get("/api/session")).json();
  const name=`TEST lifecycle ${randomUUID()}`;
  const response=await request.post("/api/model-profiles",{headers:{"x-annotagent-csrf":session.csrf_token},data:{provider_id:source.provider_id,display_name:name,remote_model_id:"test-no-probe",input_modalities:["text"],task_capabilities:["text_generation"],protocol_features:source.protocol_features}});
  expect(response.ok(),await response.text()).toBeTruthy();const model=await response.json();
  let probes=0;page.on("request",r=>{if(r.url().includes("active-probe"))probes++;});
  await page.goto("/settings/agent-models");
  const region=page.getByRole("region",{name:"模型配置",exact:true});
  await region.getByLabel("搜索模型",{exact:true}).fill(name);
  const expand=()=>region.getByText("模型状态、质量与管理",{exact:true}).click();
  await expand();await region.getByRole("button",{name:"读取质量契约",exact:true}).click();
  await region.getByRole("button",{name:"收费连接测试…",exact:true}).click();
  let dialog=page.getByRole("dialog",{name:"确认收费连接测试",exact:true});
  await expect(dialog.getByRole("button",{name:"确认操作",exact:true})).toBeDisabled();
  await dialog.getByRole("button",{name:"取消",exact:true}).click();expect(probes).toBe(0);
  await region.getByRole("button",{name:"锁定配置…",exact:true}).click();
  dialog=page.getByRole("dialog",{name:"锁定模型配置",exact:true});
  await dialog.getByRole("checkbox").check();await dialog.getByRole("button",{name:"确认操作",exact:true}).click();await expect(dialog).toHaveCount(0);
  await expect(region.getByRole("button",{name:"已锁定",exact:true})).toBeDisabled();
  await expand();await region.getByRole("button",{name:"解锁配置…",exact:true}).click();
  dialog=page.getByRole("dialog",{name:"解锁模型配置",exact:true});await dialog.getByRole("checkbox").check();await dialog.getByRole("button",{name:"确认操作",exact:true}).click();await expect(dialog).toHaveCount(0);
  await expand();await region.getByRole("button",{name:"删除模型配置…",exact:true}).click();
  dialog=page.getByRole("dialog",{name:"删除模型配置",exact:true});await dialog.getByRole("checkbox").check();await expect(dialog.getByRole("button",{name:"确认操作",exact:true})).toBeDisabled();
  await dialog.getByLabel("输入模型显示名称",{exact:true}).fill(name);await dialog.getByRole("button",{name:"确认操作",exact:true}).click();await expect(dialog).toHaveCount(0);
  const after=await(await request.get("/api/model-profiles")).json();expect(after.models.some((m:{id:string})=>m.id===model.id)).toBe(false);expect(probes).toBe(0);
});
test("native model editor saves declarations without probing or invoking providers",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const providers=await(await request.get("/api/providers")).json();
  const provider=providers.providers.find((p:{endpoint?:string;display_name:string})=>p.display_name.includes("TEST"));expect(provider).toBeTruthy();
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto("/settings/vision-models");
  await page.getByRole("button",{name:"添加模型配置",exact:true}).click();
  let dialog=page.getByRole("dialog",{name:"添加模型配置",exact:true});
  const name=`TEST declaration ${randomUUID()}`;
  await dialog.getByRole("combobox",{name:"Provider",exact:true}).selectOption(provider.id);
  await dialog.getByLabel("显示名称",{exact:true}).fill(name);
  await dialog.getByLabel("远程模型 ID",{exact:true}).fill("test-declared-model-no-inference");
  await dialog.getByLabel("image",{exact:true}).check();
  await dialog.getByLabel("视觉语言",{exact:true}).check();
  await dialog.getByLabel("流式响应",{exact:true}).check();
  await dialog.getByLabel("每次请求",{exact:true}).fill("0.000001");
  await dialog.getByRole("button",{name:"保存配置",exact:true}).click();
  await expect(dialog).toHaveCount(0);
  const region=page.getByRole("region",{name:"模型配置",exact:true});
  await region.getByLabel("搜索模型",{exact:true}).fill(name);
  await region.getByRole("button",{name:"编辑配置",exact:true}).click();
  dialog=page.getByRole("dialog",{name:"编辑模型配置",exact:true});
  await expect(dialog.getByLabel("image",{exact:true})).toBeChecked();
  await expect(dialog.getByLabel("每次请求",{exact:true})).toHaveValue("0.000001");
  await dialog.getByLabel("启用模型",{exact:true}).uncheck();
  await dialog.getByRole("button",{name:"保存配置",exact:true}).click();
  await expect(dialog).toHaveCount(0);await page.reload();
  const models=await(await request.get("/api/model-profiles")).json();
  const model=models.models.find((m:{display_name:string})=>m.display_name===name);
  expect(model.enabled).toBe(false);expect(model.protocol_features.streaming).toBe(true);
  expect(model.pricing.per_request).toBe("0.000001");expect(model.status).not.toBe("available");
  expect(writes.some(url=>/active-probe|discover-models|chat\/completions|journey-consents/.test(url))).toBe(false);
});
test("native export creates a real download and refresh never repeats export",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const nav=await(await request.get("/api/navigation")).json();
  const project=nav.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture");expect(project).toBeTruthy();
  let exports=0;
  page.on("request",r=>{if(r.method()==="POST"&&r.url().endsWith("/export"))exports++;});
  await page.goto(`/projects/${project.project_id}/manage/export`);
  await expect(page.getByRole("heading",{name:"导出标注",exact:true})).toBeVisible();
  const generate=page.getByRole("button",{name:/^导出为 /});
  await expect(generate).toBeEnabled();expect(exports).toBe(0);
  await generate.click();
  const report=page.getByRole("region",{name:"已保存导出报告"});
  const link=report.getByRole("link",{name:/下载标注文件/});
  await expect(link).toBeVisible();expect(exports).toBe(1);
  const href=await link.getAttribute("href");expect(href).toMatch(/\/exports\/[^/]+\/download$/);
  const response=await request.get(href!);expect(response.ok()).toBeTruthy();
  const bytes=await response.body();expect(bytes.length).toBeGreaterThan(0);
  const latest=await(await request.get(`/api/projects/${project.project_id}/export-readiness`)).json();
  expect(createHash("sha256").update(bytes).digest("hex")).toBe(latest.last_export.delivery.sha256);
  await page.reload();await expect(link).toHaveAttribute("href",href!);expect(exports).toBe(1);
  await page.route("**/export",r=>r.abort());
  await generate.click();await expect(page.locator(".native-export [role=alert]")).toContainText("没有得到成功确认");
  await expect(generate).toBeDisabled();
  await page.getByRole("button",{name:"重新读取范围与报告",exact:true}).click();
  await expect(link).toHaveAttribute("href",href!);expect(exports).toBe(2);
  await expect(generate).toBeDisabled();
});
test("native review submits an enabled Skill taxonomy through real HTTP",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const runs=await(await request.get("/api/runs?limit=50")).json();const run=runs.runs.find((r:{project_name:string})=>r.project_name==="TEST Agent UI HTTP fixture");expect(run).toBeTruthy();
  const headers={"x-annotagent-csrf":(await(await request.get("/api/session")).json()).csrf_token};
  const summary=await(await request.get(`/api/projects/${run.project_id}/summary`)).json();const originalSkills=summary.project.enabled_skills.map((s:{id:string;version:string})=>({id:s.id,version:s.version}));
  const registry=await(await request.get("/api/skills")).json();const skill=registry.find((s:{id:string})=>s.id==="robocup.ball");expect(skill.correction_taxonomy).toContain("inaccurate_ball_bbox");
  const skillsUrl=`/api/projects/${run.project_id}/skills`;
  try{
    const enabled=await request.post(skillsUrl,{headers,data:{enabled_skills:[...originalSkills.filter((s:{id:string})=>s.id!==skill.id),{id:skill.id,version:skill.version}]}});expect(enabled.ok(),await enabled.text()).toBeTruthy();
    const base=(await(await request.get(`/api/runs/${run.id}/annotations`)).json()).annotations[0];const id=randomUUID();
    const created=await request.post(`/api/runs/${run.id}/annotations`,{headers,data:{annotation:{...base,id,label:`${base.label} TEST`,source:"human",review_status:"needs_review",created_at:new Date().toISOString()}}});expect(created.ok(),await created.text()).toBeTruthy();
    const queue=await(await request.get(`/api/projects/${run.project_id}/reviews`)).json();const item=queue.reviews.find((r:{annotation_id:string})=>r.annotation_id===id);expect(item).toBeTruthy();
    await page.goto(`/projects/${run.project_id}/manage/review/${item.review_id}`);await page.getByText("审核问题与备注",{exact:true}).click();
    await page.getByLabel("拒绝原因",{exact:true}).selectOption(JSON.stringify([skill.id,"inaccurate_ball_bbox"]));
    await page.getByLabel("审核备注",{exact:true}).fill("TEST enabled taxonomy");page.on("dialog",d=>d.accept());await page.reload();await page.getByText("审核问题与备注",{exact:true}).click();
    await expect(page.getByLabel("拒绝原因",{exact:true})).toHaveValue(JSON.stringify([skill.id,"inaccurate_ball_bbox"]));
    const removed=await request.post(skillsUrl,{headers,data:{enabled_skills:originalSkills.filter((s:{id:string})=>s.id!==skill.id)}});expect(removed.ok(),await removed.text()).toBeTruthy();
    await page.reload();await page.getByText("审核问题与备注",{exact:true}).click();await expect(page.getByLabel("拒绝原因",{exact:true})).toHaveValue(JSON.stringify([skill.id,"inaccurate_ball_bbox"]));await expect(page.getByRole("button",{name:"拒绝并下一项",exact:true})).toBeDisabled();
    const reenabled=await request.post(skillsUrl,{headers,data:{enabled_skills:[...originalSkills.filter((s:{id:string})=>s.id!==skill.id),{id:skill.id,version:skill.version}]}});expect(reenabled.ok(),await reenabled.text()).toBeTruthy();
    await page.reload();await expect(page.getByRole("button",{name:"拒绝并下一项",exact:true})).toBeEnabled();
    const sent=page.waitForRequest(r=>r.url().endsWith("/reject-and-next")&&r.method()==="POST");await page.getByRole("button",{name:"拒绝并下一项",exact:true}).click();
    expect((await sent).postDataJSON()).toMatchObject({decision:"reject",reason_code:"inaccurate_ball_bbox",skill_id:skill.id,note:"TEST enabled taxonomy"});
    await expect.poll(async()=>(await(await request.get(`/api/projects/${run.project_id}/reviews/${item.review_id}`)).json()).annotation.review_status).toBe("rejected");
  }finally{
    const restored=await request.post(skillsUrl,{headers,data:{enabled_skills:originalSkills}});expect(restored.ok(),await restored.text()).toBeTruthy();
    const after=await(await request.get(`/api/projects/${run.project_id}/summary`)).json();expect(after.project.available_workflow_versions).toEqual(summary.project.available_workflow_versions);
  }
});
test("native review preserves failed edits and advances only after a saved decision",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const runs=await(await request.get("/api/runs?limit=50")).json();
  const run=runs.runs.find((r:{project_name:string})=>r.project_name==="TEST Agent UI HTTP fixture");expect(run).toBeTruthy();
  const snapshot=await(await request.get(`/api/runs/${run.id}/annotations`)).json();
  const base=snapshot.annotations[0];expect(base).toBeTruthy();
  const session=await(await request.get("/api/session")).json();
  const old=await(await request.get(`/api/projects/${run.project_id}/reviews`)).json();
  for(const prior of old.reviews){
    expect(prior.annotation.source).toBe("human");expect(prior.annotation.label).toMatch(/ TEST$|^TEST_manual_box$/);
    const outcome=await request.post(`/api/projects/${run.project_id}/reviews/${prior.review_id}/accept-and-next`,{headers:{"x-annotagent-csrf":session.csrf_token},data:{decision:"accept",reason_code:"accepted_as_is",note:"Finish an interrupted isolated review test"}});
    expect(outcome.ok(),await outcome.text()).toBeTruthy();
  }
  const id=randomUUID();
  const seeded=await request.post(`/api/runs/${run.id}/annotations`,{headers:{"x-annotagent-csrf":session.csrf_token},data:{annotation:{...base,id,source:"human",review_status:"needs_review",created_at:new Date().toISOString()}}});
  expect(seeded.ok(),await seeded.text()).toBeTruthy();
  const queue=await(await request.get(`/api/projects/${run.project_id}/reviews`)).json();
  const review=queue.reviews.find((r:{annotation_id:string})=>r.annotation_id===id);expect(review).toBeTruthy();
  await page.goto(`/projects/${run.project_id}/manage/review/${review.review_id}?queue_offset=50`);
  await expect(page.getByRole("heading",{name:"审核标注",exact:true})).toBeVisible();
  await page.getByText("来源与审核证据",{exact:true}).click();await page.getByRole("link",{name:"查看来源运行",exact:true}).click();await expect(page).toHaveURL(new RegExp(`source_review=${review.review_id}&queue_offset=50`));await page.reload();await page.getByRole("link",{name:"返回来源审核项",exact:true}).click();await expect(page).toHaveURL(new RegExp(`/manage/review/${review.review_id}\\?queue_offset=50$`));await expect(page.getByRole("link",{name:"返回审核队列",exact:true})).toHaveAttribute("href",`/projects/${run.project_id}/manage/review?queue_offset=50`);
  await expect(page.locator(".workspace-header")).toContainText("TEST Agent UI HTTP fixture");
  await expect(page.locator(".workspace-header")).not.toContainText("不存在");
  const label=page.getByLabel("标签",{exact:true});await expect(label).toHaveValue(base.label);
  await expect(page.locator(".native-review")).toContainText(`待审核 ${queue.progress.remaining_count}`);
  await page.getByText("属性编辑",{exact:true}).click();const attrs=page.getByRole("region",{name:"标注属性编辑",exact:true});await attrs.getByLabel("属性 JSON",{exact:true}).fill("[]");await attrs.getByRole("button",{name:"应用属性到编辑",exact:true}).click();await expect(attrs.getByRole("alert")).toContainText("JSON 对象");await expect(page.getByRole("button",{name:"接受这个对象并下一项",exact:true})).toBeDisabled();
  await attrs.getByRole("button",{name:"取消属性编辑",exact:true}).click();await expect(page.getByRole("button",{name:"接受这个对象并下一项",exact:true})).toBeEnabled();
  await attrs.getByLabel("属性 JSON",{exact:true}).fill(JSON.stringify({...base.attributes,TEST_manual_attribute:true}));await attrs.getByRole("button",{name:"应用属性到编辑",exact:true}).click();
  await label.fill(`${base.label} TEST`);
  await page.getByRole("button",{name:"撤销编辑",exact:true}).click();await expect(label).toHaveValue(base.label);
  await page.getByRole("button",{name:"重做编辑",exact:true}).click();await expect(label).toHaveValue(`${base.label} TEST`);
  await page.getByRole("button",{name:"原图",exact:true}).click();await expect(page.getByRole("button",{name:"原图",exact:true})).toHaveAttribute("aria-pressed","true");
  await page.getByRole("button",{name:"服务器已保存标注",exact:true}).click();await expect(label).toHaveValue(`${base.label} TEST`);
  await page.getByRole("button",{name:"当前编辑",exact:true}).click();
  page.on("dialog",d=>d.accept());await page.reload();
  await expect(label).toHaveValue(`${base.label} TEST`);
  await page.route(`**/api/annotations/${id}`,r=>r.abort());
  await page.getByRole("button",{name:"保存编辑",exact:true}).click();
  await expect(page.locator(".native-review [role=alert]")).toBeVisible();
  await expect(label).toHaveValue(`${base.label} TEST`);
  await expect(page.getByRole("button",{name:"接受这个对象并下一项",exact:true})).toBeDisabled();
  await page.unroute(`**/api/annotations/${id}`);
  await page.evaluate(()=>{const remove=Storage.prototype.removeItem;Storage.prototype.removeItem=function(key){if(key.startsWith("annotagent.review-edit."))throw new Error("TEST cache unavailable");return remove.call(this,key);};});
  await page.getByRole("button",{name:"保存编辑",exact:true}).click();
  await expect(page.getByRole("button",{name:"接受这个对象并下一项",exact:true})).toBeEnabled();
  await expect(page.locator(".native-review")).toContainText("编辑已保存到服务器；尚未接受此标注。");
  await page.getByText("审核问题与备注",{exact:true}).click();await page.getByLabel("拒绝原因",{exact:true}).selectOption("too_loose");await page.getByLabel("审核备注",{exact:true}).fill("TEST verified correction");
  await page.reload();await expect(label).toHaveValue(`${base.label} TEST`);
  await page.getByText("审核问题与备注",{exact:true}).click();await expect(page.getByLabel("拒绝原因",{exact:true})).toHaveValue("too_loose");await expect(page.getByLabel("审核备注",{exact:true})).toHaveValue("TEST verified correction");
  await page.screenshot({path:"/tmp/annotagent-native-review-controls.png",fullPage:true});
  await page.getByText("来源与审核证据",{exact:true}).click();
  await page.getByRole("button",{name:"读取修订记录",exact:true}).click();
  const decisionRequest=page.waitForRequest(r=>r.url().endsWith("/accept-and-next")&&r.method()==="POST");
  await page.getByRole("button",{name:"接受这个对象并下一项",exact:true}).click();
  expect((await decisionRequest).postDataJSON()).toMatchObject({decision:"accept",reason_code:"accepted_as_is",note:"TEST verified correction"});
  await expect(page.locator(".native-review")).toContainText("队列已结束");
  await expect(label).toHaveValue(`${base.label} TEST`);
  await expect(page.getByRole("button",{name:"接受这个对象并下一项",exact:true})).toBeDisabled();
  const saved=await(await request.get(`/api/projects/${run.project_id}/reviews/${review.review_id}`)).json();expect(saved.annotation.review_status).toBe("human_accepted");
  expect(saved.annotation.attributes.TEST_manual_attribute).toBe(true);
  await page.reload();await expect(label).toHaveValue(`${base.label} TEST`);
  await expect(page.getByRole("button",{name:"接受这个对象并下一项",exact:true})).toBeDisabled();
  const rejectId=randomUUID();
  const added=await request.post(`/api/runs/${run.id}/annotations`,{headers:{"x-annotagent-csrf":session.csrf_token},data:{annotation:{...base,id:rejectId,label:`${base.label} TEST`,source:"human",review_status:"needs_review",created_at:new Date().toISOString()}}});expect(added.ok(),await added.text()).toBeTruthy();
  const rejectQueue=await(await request.get(`/api/projects/${run.project_id}/reviews`)).json();const rejectItem=rejectQueue.reviews.find((r:{annotation_id:string})=>r.annotation_id===rejectId);expect(rejectItem).toBeTruthy();
  await page.goto(`/projects/${run.project_id}/manage/review/${rejectItem.review_id}`);
  await page.getByText("审核问题与备注",{exact:true}).click();await page.getByLabel("拒绝原因",{exact:true}).selectOption("duplicate");await page.getByLabel("审核备注",{exact:true}).fill("TEST duplicate annotation");
  const rejection=page.waitForRequest(r=>r.url().endsWith("/reject-and-next")&&r.method()==="POST");await page.getByRole("button",{name:"拒绝并下一项",exact:true}).click();expect((await rejection).postDataJSON()).toMatchObject({decision:"reject",reason_code:"duplicate",note:"TEST duplicate annotation"});
  await expect(page.locator(".native-review")).toContainText("队列已结束");await expect(page.locator(".native-review")).toContainText("待审核 0");
  const rejected=await(await request.get(`/api/projects/${run.project_id}/reviews/${rejectItem.review_id}`)).json();expect(rejected.annotation.review_status).toBe("rejected");
});
test("new UI creates a TEST project, uploads and defines labels through real HTTP", async ({page,request})=>{
  const health=await request.get("/api/health");
  expect(health.headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const requests:string[]=[];
  page.on("request",r=>{if(r.method()!=="GET")requests.push(r.url());});
  await page.goto("/projects/new");
  await page.getByLabel("项目名称",{exact:true}).fill(`TEST clean-cut ${Date.now()}`);
  await page.getByRole("button",{name:"创建项目",exact:true}).click();
  await expect(page).toHaveURL(/\/projects\/project-[^/]+\/manage\/data$/);
  await page.getByLabel("选择图片",{exact:true}).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await page.getByRole("button",{name:"上传 1 张图片",exact:true}).click();
  await expect(page.getByRole("heading",{name:"已保存图片 · 1"})).toBeVisible();
  await page.getByRole("link",{name:"标签定义",exact:true}).click();
  await page.getByLabel("标签组名称",{exact:true}).fill("测试足球目标");
  await page.getByLabel("类别（逗号或换行分隔）",{exact:true}).fill("ball， ball\nball");
  await page.locator("summary").filter({hasText:"可选属性"}).click();await page.getByLabel("属性名称",{exact:true}).fill("TEST_occluded");await page.getByRole("combobox",{name:"属性类型",exact:true}).selectOption("boolean");
  await page.getByRole("button",{name:"保存标签组",exact:true}).click();
  await expect(page.getByText("标签组已保存；没有自动生成或执行方案。",{exact:true})).toBeVisible();
  const owner=new URL(page.url()).pathname.split("/")[2];const catalog=await(await page.request.get(`/api/projects/${owner}/workflow-catalog`)).json();expect(catalog.project_schema.tasks[0].attributes.TEST_occluded.type).toBe("boolean");
  await page.getByLabel("新增类别",{exact:true}).fill("test-ball");
  await page.getByRole("button",{name:"添加类别",exact:true}).click();
  await expect(page.getByText("ball · test-ball",{exact:true})).toBeVisible();
  await page.reload();
  await expect(page.getByText("ball · test-ball",{exact:true})).toBeVisible();
  expect(requests.filter(p=>/schema-proposals|journey-consents|processing-operations|\/publish/.test(p))).toEqual([]);
});

test("new trash recovers a lost restore response with the same confirmed request",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const session=await(await request.get("/api/session")).json();
  const post=async(path:string,data:unknown)=>{
    const headers:Record<string,string>={"x-annotagent-csrf":session.csrf_token};
    if(path.endsWith("/management/actions")){
      const c=await request.post("/api/session/privileged-confirmation",{headers,data:{action:`POST ${path}`,confirmed:true}});
      expect(c.ok(),await c.text()).toBeTruthy();headers["x-annotagent-privileged-confirmation"]=(await c.json()).confirmation_token;
    }
    const r=await request.post(path,{headers,data});expect(r.ok(),await r.text()).toBeTruthy();return r.json();
  };
  const projects=await(await request.get("/api/projects")).json();
  const project=projects.projects.find((p:{name:string})=>p.name.startsWith("TEST clean-cut"));expect(project).toBeTruthy();
  const draft=await post("/api/workflow-drafts",{project_id:project.id,from_template:false});
  const lifecycle=await(await request.get(`/api/projects/${project.id}/pipelines`)).json();
  const object=lifecycle.pipelines.flatMap((p:{drafts:{object:{id:string}}[]})=>p.drafts).find((d:{object:{id:string}})=>d.object.id===draft.id).object;
  const input={project_id:project.id,objects:[object],action:"move_to_trash",idempotency_key:randomUUID()};
  const impact=await post(`/api/projects/${project.id}/management/preview`,input);expect(impact.can_execute).toBeTruthy();
  await post(`/api/projects/${project.id}/management/actions`,{...input,confirmation_token:impact.confirmation_token});
  await page.goto(`/projects/${project.id}/manage/trash`);
  const submitted:unknown[]=[];
  await page.route("**/management/actions",async route=>{
    submitted.push(route.request().postDataJSON());
    if(submitted.length===1){await route.fetch();await route.abort();}
    else await route.continue();
  });
  await page.getByRole("button",{name:"恢复…",exact:true}).first().click();
  const dialog=page.getByRole("dialog",{name:"恢复",exact:true});
  await expect(dialog.getByRole("button",{name:"恢复",exact:true})).toBeEnabled();
  await dialog.getByRole("button",{name:"恢复",exact:true}).click();
  await expect(dialog.getByRole("button",{name:"重试原操作（相同幂等键）",exact:true})).toBeEnabled();
  await page.reload();
  await page.getByRole("button",{name:"查看原操作",exact:true}).click();
  await page.getByRole("button",{name:"重试原操作（相同幂等键）",exact:true}).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole("region",{name:"管理操作回执"}).getByRole("status")).toContainText("completed");
  const final=await(await request.get(`/api/projects/${project.id}/trash`)).json();expect(final.items.some((i:{object:{id:string}})=>i.object.id===draft.id)).toBe(false);
  await page.reload();await expect(page.getByRole("region",{name:"管理操作回执"}).getByRole("status")).toContainText("completed");
  expect(submitted).toHaveLength(2);expect(submitted[1]).toEqual(submitted[0]);
});
