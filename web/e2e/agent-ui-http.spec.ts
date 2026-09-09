import { test, expect } from "@playwright/test";

test.beforeEach(async ({request})=>{
  const health=await request.get("/api/health");
  expect(health.headers()["x-annotagent-fixture"]).toBe("external-model-only");
});
async function identity(request: import("@playwright/test").APIRequestContext) {
  const nav=await (await request.get("/api/navigation")).json();
  const p=nav.items.find((p:{project_id:string})=>p.project_id.startsWith("TEST-"));
  expect(p).toBeTruthy();
  const root=`/api/projects/${p.project_id}/conversations/${p.conversation_id}`;
  const tasks=await (await request.get(`${root}/task-navigation`)).json();
  return {p,root,tasks:tasks.items};
}
test("a: real owned thread/image reads, Back/refresh, no execution on GET",async({page,request})=>{
  const {root,tasks}=await identity(request);
  const writes:string[]=[];page.on("request",r=>{if(r.method()!=="GET")writes.push(r.url());});
  await page.goto(`/agent-integration.html?task=${tasks[0].task_id}`);
  await expect(page.getByText("本地工作区 · HTTP")).toBeVisible();
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
  await page.goto(`/agent-integration.html?task=${tasks[0].task_id}&pane=image`);
  await page.getByRole("button",{name:"⚙ 设置"}).click();
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
  await page.goto(`/agent-integration.html?task=${encodeURIComponent(`new:${p.project_id}`)}`);
  const input=page.getByRole("textbox",{name:"给 AnnotAgent 的需求"});
  const writes:string[]=[];page.on("request",r=>{if(r.method()==="POST"&&r.url().endsWith("/send"))writes.push(r.url());});
  await input.fill("TEST HTTP UI classify indoor or outdoor");
  await input.dispatchEvent("compositionstart");await input.press("Enter");await input.dispatchEvent("compositionend");
  expect(writes).toHaveLength(0);
  await input.press("Shift+Enter");await input.type("TEST persisted input");
  await page.getByRole("button",{name:"↑ 发送"}).dblclick();
  await expect(page.locator(".user-message")).toContainText("TEST HTTP UI classify");
  expect(writes).toHaveLength(1);
  const task=new URL(page.url()).searchParams.get("task");expect(task).not.toContain("new:");
  const before=await (await request.get(`${root}/tasks/${task}/workspace`)).json();expect(before.calls).toEqual([]);
  await page.reload();await expect(page.locator(".user-message")).toContainText("TEST HTTP UI classify");
  const after=await (await request.get(`${root}/tasks/${task}/workspace`)).json();expect(after.calls).toEqual([]);
});
test("c: explicit Plan approval calls TEST provider once and persisted receipts survive refresh",async({page,request})=>{
  const {p,root}=await identity(request);
  await page.goto(`/agent-integration.html?task=${encodeURIComponent(`new:${p.project_id}`)}`);
  await page.getByRole("textbox",{name:"给 AnnotAgent 的需求"}).fill("按室内和室外给图片分类 TEST authorized schema");
  await page.getByRole("button",{name:"↑ 发送"}).click();
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
  await page.goto(`/agent-integration.html?task=${task}&pane=image`);
  await expect(page.getByLabel("确认类别",{exact:true})).toBeVisible();
  await page.getByLabel("确认类别",{exact:true}).selectOption("室外");
  await page.getByRole("button",{name:"保存当前样例修正"}).click();
  await expect.poll(async()=>{const v=await(await request.get(`/api/workflow-sample-tests/${prior.input.sample_test_id}/images/${prior.input.image_id}/feedback`)).json();return v.revisions.at(-1)?.corrected_label;}).toBe("室外");
  await page.reload();await expect(page.getByText("分类结果：室外",{exact:true})).toBeVisible();
});
