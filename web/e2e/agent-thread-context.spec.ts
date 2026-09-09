import {randomUUID} from "node:crypto";
import {expect,test} from "./fixtures";

test("selected goal is context, not a redundant action; other goals remain reachable",async({page,request})=>{
  const project=`TEST-thread-context-${Date.now()}`;
  expect((await request.post("/api/projects",{data:{id:project,yaml:"version: 1\nproject:\n  name: TEST thread context\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}})).ok()).toBe(true);
  await page.goto(`/projects/${project}/work`);
  await expect(page.getByRole("heading",{name:"What would you like to annotate?"})).toBeVisible();
  const conversation=(await (await request.post(`/api/projects/${project}/conversations`,{data:{}})).json()).conversation_id;
  const root=`/api/projects/${project}/conversations/${conversation}`;
  const revision=(await (await request.get(`/api/projects/${project}/goal`)).json()).revision;
  const tasks=[];
  for(const text of ["TEST find cups","TEST find bottles"]){
    const response=await request.post(`${root}/send`,{data:{message:{id:randomUUID(),text,image:null},task_id:null,schema_revision:revision,mode:"plan"}});
    expect(response.ok(),await response.text()).toBe(true);
    tasks.push((await response.json()).task_id);
  }
  await page.goto(`/projects/${project}/work?conversation=${conversation}&task=${tasks[1]}`);
  const messages=page.getByRole("list",{name:"Saved messages",exact:true});
  await expect(messages).toContainText("Current annotation goal");
  await expect(page.getByRole("heading",{name:"What would you like to annotate?"})).toHaveCount(0);
  await expect(page.getByRole("button",{name:"Use message 2 as annotation goal",exact:true})).toHaveCount(0);
  await page.getByRole("button",{name:"Use message 1 as annotation goal",exact:true}).click();
  await expect.poll(()=>new URL(page.url()).searchParams.get("task")).toBe(tasks[0]);
  await expect(page.getByRole("button",{name:"Use message 1 as annotation goal",exact:true})).toHaveCount(0);
  await expect(page.getByRole("button",{name:"Use message 2 as annotation goal",exact:true})).toBeEnabled();
  const writes:string[]=[];
  page.on("request",req=>{if(req.method()!=="GET")writes.push(req.url());});
  await page.reload();
  await expect(page.getByRole("button",{name:"Use message 1 as annotation goal",exact:true})).toHaveCount(0);
  await expect(messages).toContainText("Current annotation goal");
  const requests=page.getByRole("region",{name:"Human requests",exact:true});
  await expect(requests).toHaveAttribute("data-empty","true");
  await requests.getByRole("button",{name:"Refresh requests",exact:true}).click();
  await expect(requests).toContainText("No outstanding visual requests for this goal.");
  expect(writes).toEqual([]);
  for(const width of [1440,1024,390]){
    await page.setViewportSize({width,height:900});
    await expect(page.getByRole("button",{name:width<=1024?"Show tasks":"Hide tasks",exact:true})).toBeVisible();
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    if(width!==1024)await page.screenshot({path:`/tmp/annotagent-thread-context-${width}.png`,fullPage:true});
  }
  await page.getByRole("button",{name:"Show tasks",exact:true}).click();
  await expect(page.getByRole("navigation",{name:"Conversations",exact:true}).getByRole("button",{name:"TEST find cups",exact:true})).toHaveAttribute("aria-current","page");
  await page.getByRole("button",{name:"Hide tasks",exact:true}).click();
  expect(await (await request.get(`${root}/tasks`)).json()).toHaveLength(2);
  for(const task of tasks) expect((await (await request.get(`${root}/tasks/${task}/budget`)).json()).total_reserved_calls).toBe(0);
});
