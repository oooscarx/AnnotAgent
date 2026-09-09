import { test, expect } from "@playwright/test";
import { resolve } from "node:path";
import { randomUUID } from "node:crypto";
test("native review preserves failed edits and advances only after a saved decision",async({page,request})=>{
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const runs=await(await request.get("/api/runs?limit=50")).json();
  const run=runs.runs.find((r:{project_name:string})=>r.project_name==="TEST Agent UI HTTP fixture");expect(run).toBeTruthy();
  const snapshot=await(await request.get(`/api/runs/${run.id}/annotations`)).json();
  const base=snapshot.annotations[0];expect(base).toBeTruthy();
  const session=await(await request.get("/api/session")).json();
  const old=await(await request.get(`/api/projects/${run.project_id}/reviews`)).json();
  for(const prior of old.reviews){
    expect(prior.annotation.source).toBe("human");expect(prior.annotation.label).toMatch(/ TEST$/);
    const outcome=await request.post(`/api/projects/${run.project_id}/reviews/${prior.review_id}/accept-and-next`,{headers:{"x-annotagent-csrf":session.csrf_token},data:{decision:"accept",reason_code:"accepted_as_is",note:"Finish an interrupted isolated review test"}});
    expect(outcome.ok(),await outcome.text()).toBeTruthy();
  }
  const id=randomUUID();
  const seeded=await request.post(`/api/runs/${run.id}/annotations`,{headers:{"x-annotagent-csrf":session.csrf_token},data:{annotation:{...base,id,source:"human",review_status:"needs_review",created_at:new Date().toISOString()}}});
  expect(seeded.ok(),await seeded.text()).toBeTruthy();
  const queue=await(await request.get(`/api/projects/${run.project_id}/reviews`)).json();
  const review=queue.reviews.find((r:{annotation_id:string})=>r.annotation_id===id);expect(review).toBeTruthy();
  await page.goto(`/projects/${run.project_id}/manage/review/${review.review_id}`);
  await expect(page.getByRole("heading",{name:"审核标注",exact:true})).toBeVisible();
  const label=page.getByLabel("标签",{exact:true});await expect(label).toHaveValue(base.label);
  await label.fill(`${base.label} TEST`);
  page.on("dialog",d=>d.accept());await page.reload();
  await expect(label).toHaveValue(`${base.label} TEST`);
  await page.route(`**/api/annotations/${id}`,r=>r.abort());
  await page.getByRole("button",{name:"保存编辑",exact:true}).click();
  await expect(page.locator(".native-review [role=alert]")).toBeVisible();
  await expect(label).toHaveValue(`${base.label} TEST`);
  await expect(page.getByRole("button",{name:"接受这个对象并下一项",exact:true})).toBeDisabled();
  await page.unroute(`**/api/annotations/${id}`);
  await page.getByRole("button",{name:"保存编辑",exact:true}).click();
  await expect(page.getByRole("button",{name:"接受这个对象并下一项",exact:true})).toBeEnabled();
  await page.getByText("来源与审核证据",{exact:true}).click();
  await page.getByRole("button",{name:"读取修订记录",exact:true}).click();
  await page.getByRole("button",{name:"接受这个对象并下一项",exact:true}).click();
  await expect(page.locator(".native-review")).toContainText("队列已结束");
  await expect(label).toHaveValue(`${base.label} TEST`);
  await expect(page.getByRole("button",{name:"接受这个对象并下一项",exact:true})).toBeDisabled();
  const saved=await(await request.get(`/api/projects/${run.project_id}/reviews/${review.review_id}`)).json();expect(saved.annotation.review_status).toBe("human_accepted");
  await page.reload();await expect(label).toHaveValue(`${base.label} TEST`);
  await expect(page.getByRole("button",{name:"接受这个对象并下一项",exact:true})).toBeDisabled();
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
