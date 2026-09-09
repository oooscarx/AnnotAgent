import { test,expect } from "@playwright/test";

test("native history explicitly cuts over, recovers lost response and preserves old direct references",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const navigation=await(await request.get("/api/navigation")).json();
  const project=navigation.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture").project_id;
  const before=await(await request.get(`/api/projects/${project}/pipelines`)).json();expect(before.pipelines.length).toBeGreaterThan(0);
  const draft=before.pipelines[0].drafts[0].object.id;
  const original=await(await request.get(`/api/workflow-drafts/${draft}?project_id=${project}`)).json();
  expect((await(await request.get("/api/history-scope")).json()).scope).toBeNull();
  let writes=0;page.on("request",r=>{if(new URL(r.url()).pathname==="/api/history-scope"&&r.method()==="POST")writes++;});
  await page.goto(`/projects/${project}/manage/pipelines`);await expect(page.getByRole("heading",{name:"启用新的历史列表"})).toBeVisible();
  await page.reload();expect(writes).toBe(0);expect((await(await request.get("/api/history-scope")).json()).scope).toBeNull();
  await page.getByRole("button",{name:"预览历史范围…"}).click();await page.getByRole("button",{name:"取消",exact:true}).click();expect(writes).toBe(0);
  await page.getByRole("button",{name:"预览历史范围…"}).click();
  await page.route("**/api/history-scope",async route=>{if(route.request().method()==="POST"){await route.fetch();await route.abort("failed");}else await route.continue();});
  await page.getByRole("button",{name:"确认建立工作区范围"}).click();await expect(page.getByRole("alert")).toContainText("未自动重试");expect(writes).toBe(1);
  await page.reload();await expect(page.getByRole("heading",{name:"自动化方案与版本"})).toBeVisible();await expect(page.getByText("当前范围内没有记录。可返回 Agent 创建新的任务。")).toBeVisible();expect(writes).toBe(1);
  await expect(page.locator(".sidebar")).toHaveCount(0);
  expect(await(await request.get(`/api/workflow-drafts/${draft}?project_id=${project}`)).json()).toEqual(original);
  for(const section of ["runs","trash"]){await page.goto(`/projects/${project}/manage/${section}`);await expect(page.getByRole("heading",{name:section==="runs"?"处理记录":"项目回收站",exact:true})).toBeVisible();await expect(page.locator(".sidebar")).toHaveCount(0);}
  await page.goto(`/projects/${project}/manage/pipelines`);await page.screenshot({path:"/tmp/annotagent-native-history-test.png",fullPage:true});
});

test("post-cutover Draft supports native bulk selection, trash and restore with scoped HTTP",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const navigation=await(await request.get("/api/navigation")).json();const project=navigation.items.find((p:{title:string})=>p.title==="TEST Agent UI HTTP fixture").project_id;
  const scope=(await(await request.get("/api/history-scope")).json()).scope;expect(scope).toBeTruthy();
  const session=await(await request.get("/api/session")).json();const response=await request.post("/api/workflow-drafts",{headers:{"x-annotagent-csrf":session.csrf_token},data:{project_id:project,from_template:false}});expect(response.ok(),await response.text()).toBeTruthy();const draft=await response.json();
  const urls:string[]=[];page.on("request",r=>{const path=new URL(r.url()).pathname;if(path.startsWith("/api/")&&(path.includes("/management/")||path.endsWith("/pipelines")||path.endsWith("/trash")))urls.push(r.url());});
  await page.goto(`/projects/${project}/manage/pipelines`);await expect(page.getByRole("checkbox",{name:"全选当前页"})).toBeEnabled();await page.getByRole("checkbox",{name:"全选当前页"}).check();await expect(page.getByRole("button",{name:"选中项移入回收站…"})).toBeEnabled();
  await page.screenshot({path:"/tmp/annotagent-native-history-populated.png",fullPage:true});
  await page.getByRole("button",{name:"选中项移入回收站…"}).click();await page.getByRole("dialog").getByRole("button",{name:"移入回收站",exact:true}).click();await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByRole("link",{name:"回收站",exact:true}).click();await expect(page.getByRole("heading",{name:"项目回收站"})).toBeVisible();await page.getByRole("checkbox",{name:"全选当前列表"}).check();await page.getByRole("button",{name:"恢复选中项…"}).click();await page.getByRole("dialog").getByRole("button",{name:"恢复",exact:true}).click();await expect(page.getByRole("dialog")).toHaveCount(0);
  expect((await request.get(`/api/workflow-drafts/${draft.id}?project_id=${project}`)).ok()).toBeTruthy();
  expect(urls.length).toBeGreaterThan(3);for(const url of urls)expect(new URL(url).searchParams.get("history_scope")).toBe(scope.id);
});
