import { test, expect } from "@playwright/test";
import { resolve } from "node:path";
import { randomUUID } from "node:crypto";
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
  await page.getByLabel("类别（逗号或换行分隔）",{exact:true}).fill("ball");
  await page.getByRole("button",{name:"保存标签组",exact:true}).click();
  await expect(page.getByText("标签组已保存；没有自动生成或执行方案。",{exact:true})).toBeVisible();
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
