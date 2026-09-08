import { expect, test } from "./fixtures";

test("saved messages select independent goals without inference or task substitution",async({page,request})=>{
  // This scenario specifically starts without a planner. Earlier scenarios use the
  // same isolated TEST registry; do not assume suite order left its defaults empty.
  const defaults=await (await request.get("/api/agent-model-bindings")).json();
  expect((await request.put("/api/agent-model-bindings",{data:{...defaults,pipeline_builder:null}})).ok()).toBe(true);
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
  const savedTaskUrl=page.url();
  await page.getByRole("button",{name:"Review model setup",exact:true}).click();
  await expect(page).toHaveURL(/\/settings\?/);
  await page.getByRole("navigation",{name:"Settings sections",exact:true}).getByRole("button",{name:"Models",exact:true}).click();
  await expect(page).toHaveURL(/\/settings\/models\?/);
  await page.reload();
  await page.getByRole("navigation",{name:"Settings sections",exact:true}).getByRole("button",{name:"Expert Model Plugins",exact:true}).click();
  await page.getByRole("button",{name:"Return to annotation task",exact:true}).click();
  await expect(page).toHaveURL(savedTaskUrl);
  await expect(page.getByRole("button",{name:"Use message 1 as annotation goal",exact:true})).toHaveAttribute("aria-pressed","true");
  expect((await (await request.get(`${root}/${first}/budget`)).json()).total_reserved_calls).toBe(0);
  await page.getByRole("button",{name:"Prepare label proposal",exact:true}).click();
  await expect(page.getByRole("region",{name:"Annotation Schema proposal",exact:true}).getByRole("alert")).toBeVisible();
  await page.getByRole("button",{name:"Review model setup",exact:true}).click();
  await page.getByRole("button",{name:"Add provider",exact:true}).click();
  const editor=page.locator(".panel").filter({has:page.getByRole("heading",{name:"New Provider",exact:true})});
  await editor.getByLabel("Display name",{exact:true}).fill("Conversation setup TEST");
  await editor.getByLabel("Base URL",{exact:true}).fill("http://127.0.0.1:8796/openai/v1");
  await editor.getByRole("button",{name:"Save Provider",exact:true}).click();
  const provider=page.locator(".registry-provider-card").filter({hasText:"Conversation setup TEST"});
  await provider.getByText("Add credential",{exact:true}).click();
  await provider.getByLabel("API key",{exact:true}).fill("TEST-conversation-setup-only");
  await provider.getByRole("button",{name:"Save credential",exact:true}).click();
  await page.getByRole("navigation",{name:"Settings sections",exact:true}).getByRole("button",{name:"Models",exact:true}).click();
  await page.getByRole("button",{name:"Add model",exact:true}).click();
  const modelEditor=page.locator(".registry-model-editor");
  await modelEditor.getByRole("combobox",{name:"Provider",exact:true}).selectOption({label:"Conversation setup TEST"});
  await modelEditor.getByLabel("Display name",{exact:true}).fill("Conversation TEST setup planner");
  await modelEditor.getByLabel("Remote model ID",{exact:true}).fill("e2e-conversation-classification");
  await modelEditor.getByLabel("Tool calls",{exact:true}).check();
  await modelEditor.getByLabel("Structured output",{exact:true}).check();
  await modelEditor.getByRole("button",{name:"Save Model Profile",exact:true}).click();
  const model=page.locator(".registry-model-card").filter({has:page.getByText("Conversation TEST setup planner",{exact:true})});
  page.once("dialog",dialog=>dialog.accept()); // Explicit TEST transport probe consent, no real Provider.
  await model.getByRole("button",{name:"Run billable test",exact:true}).click();
  await expect(model.getByText("Available",{exact:true})).toBeVisible();
  await page.getByLabel("Default Pipeline Builder model",{exact:true}).selectOption({label:"Conversation TEST setup planner via Conversation setup TEST"});
  await expect(page.getByLabel("Default Pipeline Builder model",{exact:true})).toBeEnabled();
  await page.reload();
  await page.screenshot({path:"../docs/execution/conversational-workspace/task-model-setup.png",fullPage:true,animations:"disabled"});
  await page.setViewportSize({width:390,height:844});
  await expect(page.getByRole("button",{name:"Return to annotation task",exact:true})).toBeVisible();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
  await page.screenshot({path:"../docs/execution/conversational-workspace/task-model-setup-390.png",fullPage:true,animations:"disabled"});
  await page.setViewportSize({width:1280,height:800});
  await page.getByRole("button",{name:"Return to annotation task",exact:true}).click();
  await expect(page).toHaveURL(savedTaskUrl);
  await page.getByRole("button",{name:"Prepare label proposal",exact:true}).click();
  await expect(page.locator('[aria-label="Schema model authorization"]')).toContainText("Conversation TEST setup planner");
  await expect(page.getByRole("button",{name:"Generate label proposal",exact:true})).toBeDisabled();
  expect((await (await request.get(`${root}/${first}/budget`)).json()).total_reserved_calls).toBe(0);
  await page.goto(`/projects/${project}/work?conversation=${conversation}&task=${crypto.randomUUID()}`);
  await expect(page.getByText("The selected annotation task is not available in this conversation. Select a saved message; no other task was substituted.",{exact:true})).toBeVisible();
  await expect(page.getByRole("region",{name:"Annotation Schema proposal",exact:true})).toHaveCount(0);
});
