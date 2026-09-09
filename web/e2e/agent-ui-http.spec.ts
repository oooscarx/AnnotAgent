import { test, expect } from "@playwright/test";
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

test.beforeEach(async ({request})=>{
  const health=await request.get("/api/health");
  expect(health.headers()["x-annotagent-fixture"]).toBe("external-model-only");
});
async function identity(request: import("@playwright/test").APIRequestContext) {
  const nav=await (await request.get("/api/navigation")).json();
  const p=nav.items.find((p:{project_id:string;title:string})=>p.project_id.startsWith("TEST-") && p.title==="TEST Agent UI HTTP fixture");
  expect(p).toBeTruthy();
  const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;
  const tasks=await (await request.get(`${root}/task-navigation`)).json();
  return {p,root,tasks:tasks.items};
}
test("a: real owned thread/image reads, Back/refresh, no execution on GET",async({page,request})=>{
  const {root,tasks}=await identity(request);
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto(`/?task=${tasks[0].task_id}`);
  await expect(page.getByText("本地工作区")).toBeVisible();
  await expect(page.locator(".user-message").first()).toContainText(tasks[0].title);
  await page.getByRole("button",{name:"打开数据",exact:true}).click();
  await expect(page.getByRole("complementary",{name:"图片与标注"})).toBeVisible();
  await expect(page.locator("svg image")).toHaveAttribute("href",/^\/api\//);
  await page.locator(".task-link").filter({hasText:tasks[1].title}).click();
  await expect(page).toHaveURL(new RegExp(tasks[1].task_id));
  await page.goBack();await page.reload();
  await expect(page).toHaveURL(new RegExp(tasks[0].task_id));
  await expect(page.locator(".user-message").first()).toContainText(tasks[0].title);
  expect(writes).toEqual([]);
  const ws=await (await request.get(`${root}/tasks/${tasks[0].task_id}/workspace`)).json();
  expect(ws.project_owner_id).toBeTruthy();
});
test("b: six real Settings reads, local preference save/cancel and return context",async({page,request})=>{
  const {tasks}=await identity(request);
  await page.goto(`/?task=${tasks[0].task_id}&pane=image`);
  await page.getByRole("button",{name:"设置"}).click();
  for(const title of ["通用","Providers 与账户","Agent 模型","视觉模型与插件","数据与隐私","用量与预算"]){
    await page.getByRole("button",{name:title,exact:true}).click();
    await expect(page.getByRole("heading",{name:title,exact:true})).toBeVisible();
    await expect(page.getByText("预览状态控制")).toHaveCount(0);
  }
  await page.getByRole("button",{name:"通用",exact:true}).click();
  await page.getByLabel("外观",{exact:true}).selectOption("dark");
  await page.getByRole("button",{name:"保存设置",exact:true}).click();
  await expect(page.getByRole("status")).toContainText("已保存");
  await page.getByRole("button",{name:"返回任务",exact:false}).click();
  await expect(page).toHaveURL(new RegExp(`task=${tasks[0].task_id}`));
  await expect(page).toHaveURL(/pane=image/);
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-aa-theme","dark");
});
test("c: IME does not send; explicit Send stores one real task; refresh never executes",async({page,request})=>{
  const {p,root}=await identity(request);
  await page.goto(`/?task=${encodeURIComponent(`new:${p.project_id}`)}`);
  const input=page.getByRole("textbox",{name:"给 AnnotAgent 的需求"});
  const writes:string[]=[];page.on("request",r=>{if(r.method()==="POST"&&r.url().endsWith("/send"))writes.push(r.url());});
  await input.fill("TEST HTTP UI classify indoor or outdoor");
  await input.dispatchEvent("compositionstart");await input.press("Enter");await input.dispatchEvent("compositionend");
  expect(writes).toHaveLength(0);
  await input.press("Shift+Enter");await input.type("TEST persisted input");
  await page.getByRole("button",{name:"发送"}).dblclick();
  await expect(page.locator(".user-message")).toContainText("TEST HTTP UI classify");
  expect(writes).toHaveLength(1);
  const task=new URL(page.url()).searchParams.get("task");expect(task).not.toContain("new:");
  const before=await (await request.get(`${root}/tasks/${task}/workspace`)).json();expect(before.calls).toEqual([]);
  await page.reload();await expect(page.locator(".user-message")).toContainText("TEST HTTP UI classify");
  const after=await (await request.get(`${root}/tasks/${task}/workspace`)).json();expect(after.calls).toEqual([]);
});
test("c: explicit Plan approval calls TEST provider once and persisted receipts survive refresh",async({page,request})=>{
  const {p,root}=await identity(request);
  await page.goto(`/?task=${encodeURIComponent(`new:${p.project_id}`)}`);
  await page.getByRole("textbox",{name:"给 AnnotAgent 的需求"}).fill("按室内和室外给图片分类 TEST authorized schema");
  await page.getByRole("button",{name:"发送"}).click();
  await expect(page.locator(".user-message")).toContainText("TEST authorized schema");
  const task=new URL(page.url()).searchParams.get("task");
  await page.getByRole("button",{name:"查看规划授权",exact:true}).click();
  await expect(page.locator(".plan-block")).toContainText("仅文本规划");
  const before=await (await request.get(`${root}/tasks/${task}/workspace`)).json();expect(before.calls).toEqual([]);
  await page.getByRole("button",{name:"查看并确认授权"}).click();
  await page.getByRole("button",{name:"接受未知费用并执行此范围"}).click();
  await expect(page.locator(".plan-history").filter({hasText:"模型结构化决策"}).first()).toBeVisible();
  await expect.poll(async()=>{const ws=await(await request.get(`${root}/tasks/${task}/workspace`)).json();return ws.calls[0]?.status;}).toBe("completed");
  await page.reload();await expect(page.locator(".plan-history").filter({hasText:"模型结构化决策"}).first()).toBeVisible();
  const after=await(await request.get(`${root}/tasks/${task}/workspace`)).json();expect(after.calls).toHaveLength(1);
});
test("e: current HumanRequest reads final classification and saves exact sandbox answer",async({page,request})=>{
  const {tasks,root}=await identity(request);
  const task=tasks[0].task_id;
  const initial=await(await request.get(`${root}/tasks/${task}/workspace`)).json();
  const prior=initial.human_requests[0];
  const feedback=await(await request.get(`/api/workflow-sample-tests/${prior.input.sample_test_id}/images/${prior.input.image_id}/feedback`)).json();
  const session=await(await request.get("/api/session")).json();
  const human={...prior.input,id:crypto.randomUUID(),expected_feedback_sequence:feedback.revisions.at(-1)?.sequence || 0,question:"TEST UI manual classification save"};
  const created=await request.post(`${root}/tasks/${task}/human-requests`,{headers:{"x-annotagent-csrf":session.csrf_token},data:human});expect(created.ok()).toBe(true);
  // Existing pending request may precede the test request. Both refer to the same exact current sample.
  await page.goto(`/?task=${task}&pane=image`);
  await expect(page.getByLabel("确认类别",{exact:true})).toBeVisible();
  await page.getByLabel("确认类别",{exact:true}).selectOption("室外");
  await page.getByRole("button",{name:"保存当前样例修正"}).click();
  await expect.poll(async()=>{const v=await(await request.get(`/api/workflow-sample-tests/${prior.input.sample_test_id}/images/${prior.input.image_id}/feedback`)).json();return v.revisions.at(-1)?.corrected_label;}).toBe("室外");
  await page.reload();await expect(page.getByText("分类结果：室外",{exact:true})).toBeVisible();
});
test("c: a separately approved Sample Journey produces saved terminal results",async({page,request})=>{
  const {p,root}=await identity(request);
  await page.goto(`/?task=${encodeURIComponent(`new:${p.project_id}`)}`);
  await page.getByRole("textbox",{name:"给 AnnotAgent 的需求"}).fill("按室内和室外给图片分类 TEST UI sample");
  await page.getByRole("button",{name:"发送"}).click();
  await expect(page.locator(".user-message")).toContainText("TEST UI sample");
  const task=new URL(page.url()).searchParams.get("task");
  await page.locator(".secondary-task-actions > summary").click();
  await page.getByRole("button",{name:"构建方案并测试样例…",exact:true}).click();
  await expect(page.locator(".plan-block")).toContainText("不写正式标注");
  await page.getByRole("button",{name:"查看并确认授权"}).click();
  await page.getByRole("button",{name:"接受未知费用并执行此范围"}).click();
  await expect.poll(async()=>{const ws=await(await request.get(`${root}/tasks/${task}/workspace`)).json();return ws.sample_operations[0]?.status;},{timeout:30_000}).toBe("succeeded");
  await page.reload();await page.getByRole("button",{name:"打开数据",exact:true}).click();
  await expect(page.getByText(/分类结果：/).first()).toBeVisible();
  // The product's one confirmation still crosses real publication/start boundaries.
  await page.getByRole("button",{name:"确认方案并开始处理…",exact:true}).click();
  await expect(page.locator(".plan-block").filter({hasText:"确认方案并开始处理"})).toContainText("发布不可变版本");
  await page.getByRole("button",{name:"查看并确认授权"}).click();
  const starts:string[]=[];page.on("request",r=>{if(r.method()==="POST"&&r.url().endsWith("/processing-operations"))starts.push(r.url());});
  await page.getByRole("button",{name:"接受未知费用并执行此范围"}).dblclick();
  await expect.poll(async()=>{const ws=await(await request.get(`${root}/tasks/${task}/workspace`)).json();return ws.processing_operations?.[0]?.batch_id;},{timeout:30_000}).toBeTruthy();
  expect(starts).toHaveLength(1);
  await page.reload();expect(starts).toHaveLength(1);
  await expect(page.getByRole("link",{name:"查看本次结果 →"}).first()).toHaveAttribute("href",new RegExp(`/projects/${p.project_id}/`));
});
test("f: explicit native Export produces a real downloadable file, not a server path link",async({page,request})=>{
  const {tasks}=await identity(request);
  await page.goto(`/?task=${tasks[0].task_id}`);
  await page.locator(".secondary-task-actions > summary").click();
  await page.getByRole("button",{name:"导出…",exact:true}).click();
  await expect(page.locator(".plan-block").filter({hasText:"导出为 Native"})).toContainText("条已确认标注");
  await page.getByRole("button",{name:"查看并确认授权"}).click();
  await page.getByRole("button",{name:"接受未知费用并执行此范围"}).click();
  await page.reload();await expect(page.getByRole("link",{name:"下载真实导出文件"}).first()).toBeVisible();
  const href=await page.getByRole("link",{name:"下载真实导出文件"}).first().getAttribute("href");
  expect(href).toMatch(/^\/api\/projects\/TEST-/);
  const file=await request.get(href!);expect(file.ok()).toBe(true);expect((await file.body()).length).toBeGreaterThan(0);
});
test("d: saved interrupted state cannot become a fictional resume; paused Batch resumes its checkpoint",async({page,request})=>{
  const nav=await(await request.get("/api/navigation")).json();
  const interrupted=nav.items.find((p:{project_id:string})=>p.project_id.startsWith("TEST-agent-ui-interrupted-"));
  const paused=nav.items.find((p:{project_id:string})=>p.project_id.startsWith("TEST-agent-ui-resumable-"));
  expect(interrupted).toBeTruthy();expect(paused).toBeTruthy();
  const interruptedTasks=await(await request.get(`/api/projects/${interrupted.project_id}/conversations/${interrupted.conversation_id}/task-navigation`)).json();
  await page.goto(`/?task=${interruptedTasks.items[0].task_id}`);
  await expect(page.getByText("停止回执已确认",{exact:true})).toBeVisible();
  await expect(page.getByRole("button",{name:"继续任务",exact:true})).toBeDisabled();
  const cr=`/api/projects/${paused.project_id}/conversations/${paused.conversation_id}`;
  const tasks=await(await request.get(`${cr}/task-navigation`)).json();
  const tr=`${cr}/tasks/${tasks.items[0].task_id}`;
  const ws=await(await request.get(`${tr}/workspace`)).json();
  const batch=ws.resume_actions.find((a:{kind:string;available:boolean})=>a.kind==="batch"&&a.available);
  expect(batch).toBeTruthy();
  const before=await(await request.get(`/api/batches/${batch.id}`)).json();
  await page.goto(`/?task=${tasks.items[0].task_id}`);
  await page.getByRole("button",{name:"继续 batch",exact:true}).click();
  await expect.poll(async()=>{const b=await(await request.get(`/api/batches/${batch.id}`)).json();return b.batch.status;},{timeout:30_000}).toBe("completed");
  const after=await(await request.get(`/api/batches/${batch.id}`)).json();
  for(const id of before.batch.child_run_ids)expect(after.batch.child_run_ids).toContain(id);
  expect(after.batch.child_run_ids).toHaveLength(3);
});

test("production routes reject foreign tasks and preserve management return without an old global sidebar",async({page,request})=>{
  const {p,tasks}=await identity(request);
  await page.goto(`/projects/not-this-project/work?task=${tasks[0].task_id}`);
  await expect(page.getByText("找不到这个任务",{exact:true})).toBeVisible();
  await expect(page.locator(".user-message")).toHaveCount(0);
  await page.goto(`/projects/${p.project_id}/work?task=${tasks[0].task_id}&pane=image`);
  await expect(page.locator("svg image")).toBeVisible();
  await expect(page.locator(".sidebar,.agent-conversation-navigation")).toHaveCount(0);
  await expect(page.locator(".project-sidebar")).toHaveCount(1);
  await page.getByLabel("项目管理菜单").click();
  await page.getByRole("link",{name:"项目管理 →",exact:true}).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${p.project_id}\\?return_task=`));
  await expect(page.locator(".sidebar,.agent-conversation-navigation")).toHaveCount(0);
  await page.goBack();await expect(page.locator(".ui-app")).toHaveAttribute("data-adapter","http");
  await expect(page).toHaveURL(new RegExp(`task=${tasks[0].task_id}`));
  await expect(page.locator("svg image")).toBeVisible();
  await page.getByRole("button",{name:"设置"}).click();
  await page.getByRole("button",{name:"Agent 模型",exact:true}).click();
  await page.getByRole("link",{name:"管理模型配置 →",exact:true}).click();
  await expect(page.getByRole("button",{name:"Return to annotation task",exact:true})).toBeVisible();
  await page.getByRole("button",{name:"Return to annotation task",exact:true}).click();
  await expect(page.locator(".ui-app")).toHaveAttribute("data-adapter","http");
  await expect(page).toHaveURL(new RegExp(`task=${tasks[0].task_id}`));
  await expect(page.locator("svg image")).toBeVisible();
});

test("b: Provider metadata and future budget persist through real server writes",async({page,request})=>{
  const {tasks}=await identity(request);
  page.on("dialog",dialog=>void dialog.accept());
  await page.goto(`/?task=${tasks[0].task_id}&settings=providers`);
  await page.getByRole("button",{name:"编辑",exact:true}).first().click();
  await page.getByLabel("显示名称",{exact:true}).fill("TEST HTTP edited provider");
  await page.getByRole("button",{name:"保存账户",exact:true}).click();
  await expect(page.getByRole("status")).toContainText("已保存");
  const providers=await(await request.get("/api/providers")).json();
  expect(providers.providers.some((p:{display_name:string})=>p.display_name==="TEST HTTP edited provider")).toBe(true);
  await page.reload();await expect(page.getByText("TEST HTTP edited provider",{exact:true})).toBeVisible();
  await page.getByRole("button",{name:"用量与预算",exact:true}).click();
  const budget=await page.getByLabel("预算上限",{exact:true}).inputValue()==="9.25"?"9.26":"9.25";
  await page.getByLabel("预算上限",{exact:true}).fill(budget);
  await page.getByRole("button",{name:"保存设置",exact:true}).click();
  await expect(page.getByRole("status")).toContainText("已保存");
  const settings=await(await request.get("/api/settings?view=agent-ui")).json();
  expect(settings.sections.usage_budget.future_run_budget.max_cost).toBe(budget);
  await page.reload();await expect(page.getByLabel("预算上限",{exact:true})).toHaveValue(budget);
});

test("d: saved supplement waits for its own explicit text-only authorization",async({page,request})=>{
  const {p,root}=await identity(request);
  await page.goto(`/projects/${p.project_id}/work`);
  await page.getByRole("textbox",{name:"给 AnnotAgent 的需求"}).fill("TEST initial goal for a fresh queue task");
  await page.getByRole("button",{name:"发送",exact:true}).click();
  await expect(page.locator(".user-message")).toContainText("TEST initial goal");
  const task=new URL(page.url()).searchParams.get("task");
  const before=await(await request.get(`${root}/tasks/${task}/workspace`)).json();
  await page.getByRole("textbox",{name:"给 AnnotAgent 的需求"}).fill("TEST queued supplement classify indoor versus outdoor");
  await page.getByRole("button",{name:"发送",exact:true}).click();
  await expect(page.locator(".user-message").last()).toContainText("TEST queued supplement");
  const after=await(await request.get(`${root}/tasks/${task}/workspace`)).json();expect(after.calls).toHaveLength(before.calls.length);
  await page.locator(".queue summary").click();
  await page.getByRole("button",{name:"查看此输入的规划范围",exact:true}).last().click();
  await expect(page.locator(".plan-block").filter({hasText:"批准此补充输入"})).toContainText("不派发剩余队列");
  await page.getByRole("button",{name:"查看并确认授权"}).click();
  await page.getByRole("button",{name:"接受未知费用并执行此范围"}).click();
  await expect.poll(async()=>{const w=await(await request.get(`${root}/tasks/${task}/workspace`)).json();return w.queue.find((q:{input:{message:{text:string}}})=>q.input.message.text==="TEST queued supplement classify indoor versus outdoor")?.status;}).toBe("completed");
});

test("a: explicit image upload uses the server importer and survives refresh",async({page,request})=>{
  const {p}=await identity(request);
  await page.goto(`/projects/${p.project_id}/work`);
  const imported=page.waitForResponse(r=>r.request().method()==="POST"&&r.url().includes("/image-upload?"));
  await page.locator('input[type="file"]').setInputFiles("../examples/robocup/images/synthetic-robocup.png");
  const receipt=await imported;expect(receipt.ok()).toBe(true);
  await expect(page.locator("svg image")).toHaveAttribute("href",/^\/api\//);
  await page.reload();await expect(page.locator("svg image")).toHaveAttribute("href",/^\/api\//);
  await expect(page.getByText("内存预览，未上传；刷新后需重新选择")).toHaveCount(0);
});

test("b: next-request model selection uses a CAS command without a model probe",async({page,request})=>{
  const {p,root}=await identity(request);
  const before=await(await request.get(`${root}/agent-model`)).json();
  await page.goto(`/projects/${p.project_id}/work`);
  const posts:string[]=[];page.on("request",r=>{if(r.method()==="POST")posts.push(r.url());});
  await page.getByRole("button",{name:/选择模型：/}).click();
  await page.locator(".model-picker section button:not(:disabled)").first().click();
  await expect.poll(async()=>{const value=await(await request.get(`${root}/agent-model`)).json();return value.revision;}).toBe(before.revision+1);
  expect(posts.filter(p=>p.endsWith("/agent-model"))).toHaveLength(1);
  expect(posts.some(p=>p.includes("probe")||p.includes("proposals")||p.endsWith("/check"))).toBe(false);
  await page.reload();await expect(page.getByRole("button",{name:/选择模型：TEST deterministic model/})).toBeVisible();
});

test("b: new TEST Provider credential is write-only and not browser-persisted",async({page,request})=>{
  const {tasks}=await identity(request);
  page.on("dialog",dialog=>void dialog.accept());
  const name=`TEST credential account ${crypto.randomUUID()}`;
  const secret="TEST-NOT-A-REAL-KEY-ui-write-only";
  await page.goto(`/?task=${tasks[0].task_id}&settings=providers`);
  await page.getByRole("button",{name:"添加 Provider",exact:true}).click();
  await page.getByLabel("显示名称",{exact:true}).fill(name);
  await page.getByLabel("Endpoint",{exact:true}).fill("http://127.0.0.1:8797/openai/v1");
  await page.getByRole("button",{name:"保存账户",exact:true}).click();
  await expect(page.getByText(name,{exact:true})).toBeVisible();
  await page.locator(".settings-row").filter({hasText:name}).getByRole("button",{name:"编辑",exact:true}).click();
  await page.getByLabel("替换 API Key（只写）",{exact:true}).fill(secret);
  await page.getByRole("button",{name:"保存新凭证",exact:true}).click();
  await expect(page.getByRole("status")).toContainText("凭证已由服务器保存");
  const providers=await(await request.get("/api/providers")).json();
  expect(providers.providers.find((p:{display_name:string})=>p.display_name===name)?.credential_configured).toBe(true);
  expect(JSON.stringify(providers)).not.toContain(secret);
  const local=await page.evaluate(()=>JSON.stringify({...localStorage}));expect(local).not.toContain(secret);
});

test("e: bbox edit saves source-normalized geometry through the real HumanRequest without accepting annotations",async({page,request})=>{
  const nav=await(await request.get("/api/navigation")).json();
  const p=nav.items.find((p:{project_id:string})=>p.project_id.startsWith("TEST-agent-ui-bbox-"));expect(p).toBeTruthy();
  const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;
  const tasks=await(await request.get(`${root}/task-navigation`)).json();const task=tasks.items[0].task_id;
  const ws=await(await request.get(`${root}/tasks/${task}/workspace`)).json();const human=ws.human_requests.find((h:{status:string})=>h.status==="pending");expect(human).toBeTruthy();
  const feedback=`/api/workflow-sample-tests/${human.input.sample_test_id}/images/${human.input.image_id}/feedback`;
  // UIAPI-004: an unanswered human question cannot create a planning grant.
  const session=await(await request.get("/api/session")).json();
  const goal=await(await request.get(`/api/projects/${p.project_id}/goal`)).json();
  const message=crypto.randomUUID();
  const sent=await request.post(`${root}/send`,{headers:{"x-annotagent-csrf":session.csrf_token},data:{message:{id:message,text:"TEST pending-human supplement",image:null},task_id:task,schema_revision:goal.revision,mode:"plan"}});expect(sent.ok()).toBe(true);
  const queue=`${root}/tasks/${task}/message-queue/${message}`;
  const blocked=await request.get(`${queue}/schema-preview`);expect(blocked.status()).toBe(409);
  expect((await blocked.json()).admitted).toBe(false);
  expect(await(await request.get(`${queue}/schema-authorization`)).json()).toBeNull();
  const readiness=await(await request.get(`/api/projects/${p.project_id}/export-readiness`)).json();
  await page.goto(`/projects/${p.project_id}/work?task=${task}&pane=image&image=${human.input.image_id}`);
  await page.getByText(/标注列表与精确编辑/).click();
  const values={x:80,y:70,w:88,h:90};
  for(const [key,value] of Object.entries(values))await page.getByLabel(`${human.input.outcome_id} ${key}`,{exact:true}).fill(String(value));
  await page.getByRole("button",{name:"保存当前样例修正",exact:true}).click();
  await expect.poll(async()=>{const f=await(await request.get(feedback)).json();return f.revisions.at(-1)?.sequence;}).toBe(human.input.expected_feedback_sequence+1);
  const saved=await(await request.get(feedback)).json();
  const rect=saved.revisions.at(-1).corrected_value.rect;
  for(const [i,value] of [80/640,70/400,88/640,90/400].entries())expect(rect[i]).toBeCloseTo(value,5);
  expect((await(await request.get(`/api/projects/${p.project_id}/export-readiness`)).json()).accepted_annotations).toBe(readiness.accepted_annotations);
  await page.reload();await page.getByText(/标注列表与精确编辑/).click();
  await expect(page.getByLabel(`${human.input.outcome_id} x`,{exact:true})).toHaveValue("80");
});

test("d: actual stop POST is observed as stopping and settles to unknown without fake resume",async({page,request,baseURL},testInfo)=>{
  test.skip(!process.env.AGENT_UI_TEST_MANIFEST,"Requires the marked UIAPI-003 fixture manifest; no synthetic HTTP response substitution");
  const m=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8"));expect(m.base_url).toBe(baseURL);expect(m.fixture).toBe("external-model-only");
  // Settings tests may legitimately advance Provider revisions; do not reuse
  // the seed's now-stale authorization or weaken the server's scope check.
  const prepared=process.env.AGENT_UI_STOP_SCENE ? null : execFileSync("python3",["../crates/annotagent-e2e-fixture/support/http_stop_scene.py","--enable-fixture","--manifest",process.env.AGENT_UI_TEST_MANIFEST!],{encoding:"utf8"});
  const scene=process.env.AGENT_UI_STOP_SCENE ? JSON.parse(readFileSync(process.env.AGENT_UI_STOP_SCENE,"utf8")).scene : JSON.parse(prepared!.split("\nScene and real HTTP trace:")[0]);
  await testInfo.attach("real-test-scene",{body:JSON.stringify(scene,null,2),contentType:"application/json"});
  const session=await(await request.get("/api/session")).json();
  const started=await request.post(scene.start.url,{headers:{"x-annotagent-csrf":session.csrf_token},data:scene.start.body});expect(started.ok()).toBe(true);
  await expect.poll(async()=>{const calls=await(await request.get(scene.wait_for_reserved_url)).json();return calls.some((c:{status:string})=>c.status==="reserved");}).toBe(true);
  await page.goto(`/projects/${m.project}/work?task=${scene.task_id}`);
  await expect(page.getByRole("button",{name:"停止",exact:true})).toBeEnabled();
  await page.getByRole("textbox",{name:"给 AnnotAgent 的需求"}).fill("TEST queued while the model request is active");
  await page.getByRole("button",{name:"排队",exact:true}).click();
  await expect(page.getByText(/1 条排队输入/)).toBeVisible();
  await page.screenshot({path:testInfo.outputPath("running-queue.png")});
  await page.getByRole("button",{name:"打开数据",exact:true}).click();
  await page.setViewportSize({width:390,height:844});
  await expect(page.getByRole("button",{name:"停止当前执行",exact:true})).toBeVisible();
  const response=page.waitForResponse(r=>r.request().method()==="POST"&&r.url().endsWith("/stop-requests"));
  await page.getByRole("button",{name:"停止当前执行",exact:true}).click();
  const initial=await(await response).json();expect(initial.normalized_state).toBe("stopping");
  await testInfo.attach("actual-initial-stop-receipt",{body:JSON.stringify(initial,null,2),contentType:"application/json"});
  await page.getByRole("button",{name:"收起数据",exact:true}).click();
  await expect(page.getByText(/远端结果未知/)).toBeVisible();
  await page.reload();await expect(page.getByText(/远端结果未知/)).toBeVisible();
  const final=await(await request.get(scene.workspace_url)).json();expect(final.calls.some((c:{status:string})=>c.status==="in_doubt")).toBe(true);
  expect(final.resume_actions.filter((a:{available:boolean})=>a.available)).toEqual([]);
});
