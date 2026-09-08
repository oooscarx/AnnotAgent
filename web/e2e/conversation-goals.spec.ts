import { expect, test } from "./fixtures";

test("saved messages select independent goals without inference or task substitution",async({page,request})=>{
  const project=`conversation-goals-${Date.now()}`;
  const created=await request.post("/api/projects",{data:{id:project,yaml:"version: 1\nproject:\n  name: TEST independent conversation goals\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}});
  expect(created.ok(),await created.text()).toBe(true);
  await page.goto(`/projects/${project}/work`);
  for(const message of ["Find cups, not bottles", "按室内和室外分类"]){
    await page.getByLabel("Your message",{exact:true}).fill(message);
    await page.getByRole("button",{name:"Save message",exact:true}).click();
    await expect(page.getByRole("list",{name:"Saved messages",exact:true})).toContainText(message);
  }
  const conversation=(await (await request.get(`/api/projects/${project}/conversations`)).json()).conversation_id;
  const root=`/api/projects/${project}/conversations/${conversation}/tasks`;
  expect(await (await request.get(root)).json()).toHaveLength(0);
  await page.getByRole("button",{name:"Use message 1 as annotation goal",exact:true}).click();
  await expect(page).toHaveURL(/task=/);
  const first=new URL(page.url()).searchParams.get("task");
  await page.getByRole("button",{name:"Use message 2 as annotation goal",exact:true}).click();
  await expect.poll(()=>new URL(page.url()).searchParams.get("task")).not.toBe(first);
  const second=new URL(page.url()).searchParams.get("task");
  await page.reload();
  await expect(page.getByRole("button",{name:"Use message 2 as annotation goal",exact:true})).toHaveAttribute("aria-pressed","true");
  await page.screenshot({path:"../docs/execution/conversational-workspace/independent-goals.png",fullPage:true,animations:"disabled"});
  await page.getByRole("button",{name:"Use message 2 as annotation goal",exact:true}).click();
  expect(new URL(page.url()).searchParams.get("task")).toBe(second);
  await page.getByRole("button",{name:"Use message 1 as annotation goal",exact:true}).click();
  await expect.poll(()=>new URL(page.url()).searchParams.get("task")).toBe(first);
  const tasks=await (await request.get(root)).json();
  expect(tasks).toHaveLength(2);
  expect(new Set(tasks.map((task:any)=>task.input.source_message_id)).size).toBe(2);
  for(const task of tasks){
    const budget=await (await request.get(`${root}/${task.input.id}/budget`)).json();
    expect(budget.total_authorized_calls).toBe(0);expect(budget.total_reserved_calls).toBe(0);
  }
  await page.goto(`/projects/${project}/work?conversation=${conversation}&task=${crypto.randomUUID()}`);
  await expect(page.getByText("The selected annotation task is not available in this conversation. Select a saved message; no other task was substituted.",{exact:true})).toBeVisible();
  await expect(page.getByRole("region",{name:"Annotation Schema proposal",exact:true})).toHaveCount(0);
});
