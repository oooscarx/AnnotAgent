import { expect, test } from "./fixtures";

test("new Agent goal starts with text-only planning and restores without image authorization", async ({page,request}) => {
  const project = `TEST-plan-entry-${Date.now()}`;
  expect((await request.post("/api/projects", {data:{id:project,yaml:"version: 1\nproject:\n  name: TEST Plan entry\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}})).ok()).toBe(true);
  // Explicit, isolated protocol fixture, never a real-provider quality test.
  const provider = await (await request.post("/api/providers",{data:{display_name:`TEST Plan ${project}`,adapter:"open_ai_compatible",base_url:"http://127.0.0.1:8796/openai/v1"}})).json();
  expect((await request.post(`/api/providers/${provider.id}/credential`,{data:{source:"workspace_file",secret:"guided-e2e-protocol-fixture"}})).ok()).toBe(true);
  const model = await (await request.post("/api/model-profiles",{data:{provider_id:provider.id,display_name:`TEST Plan ${project}`,remote_model_id:"e2e-planner",input_modalities:["text"],task_capabilities:["text_generation"],protocol_features:{tool_calls:true,structured_output:true}}})).json();
  expect((await request.post(`/api/providers/${provider.id}/active-probe`,{data:{model_profile_id:model.id,confirmed_billable:true}})).ok()).toBe(true);
  await page.goto(`/projects/${project}/work`);
  await page.getByLabel("Choose Agent model",{exact:true}).click();
  await page.getByRole("combobox",{name:"Agent model",exact:true}).selectOption(model.id);
  await expect(page.getByRole("combobox",{name:"Agent model",exact:true})).toBeEnabled();
  await page.getByRole("searchbox",{name:"Search models"}).press("Escape");
  const mutations:string[]=[];
  page.on("request",req=>{if(req.method()!=="GET")mutations.push(new URL(req.url()).pathname);});
  await page.getByRole("textbox",{name:"Your message",exact:true}).fill("TEST find cups, exclude bottles");
  await page.getByRole("button",{name:"Send",exact:true}).click();
  // This is an existing div with an accessible label, not a synthetic panel.
  const authorization=page.locator('[aria-label="Schema model authorization"]');
  await expect(authorization).toBeVisible();
  await expect(authorization).toContainText("Cost unknown");
  await authorization.screenshot({path:"/tmp/annotagent-agent-plan-entry.png"});
  await expect(page.getByText("Planning may use a paid text model. Image processing requires a separate authorization.")).toBeVisible();
  expect(mutations.every(path=>path.endsWith("/send")||path.endsWith("/task-selection")),JSON.stringify(mutations)).toBe(true);
  await authorization.getByRole("checkbox").check();
  await authorization.getByRole("button",{name:"Generate label proposal",exact:true}).click();
  await expect(page.getByRole("button",{name:"Review Builder authorization",exact:true})).toBeEnabled();
  await expect(page.getByText("This step builds a Draft; it does not test images or publish.",{exact:false})).toBeVisible();
  const conversation=(await (await request.get(`/api/projects/${project}/conversations`)).json()).conversation_id;
  const root=`/api/projects/${project}/conversations/${conversation}`;
  const tasks=await (await request.get(`${root}/tasks`)).json();
  expect(tasks).toHaveLength(1);
  const taskRoot=`${root}/tasks/${tasks[0].input.id}`;
  expect((await (await request.get(`${taskRoot}/budget`)).json()).total_reserved_calls).toBe(1);
  expect(mutations.filter(path=>!path.endsWith("/send")&&!path.endsWith("/task-selection")&&!path.endsWith("/schema-proposals")),JSON.stringify(mutations)).toEqual([]);
  const count=mutations.length;
  await page.reload();
  await expect(page.getByRole("button",{name:"Review Builder authorization",exact:true})).toBeEnabled();
  expect(mutations).toHaveLength(count);
  await page.getByRole("button",{name:"Review Builder authorization",exact:true}).click();
  await expect(page.locator('[aria-label="Builder model authorization"]')).toBeVisible();
  expect(mutations).toHaveLength(count);
  await page.getByRole("button",{name:"Review combined planning and sample authorization",exact:true}).click();
  await expect(page.getByRole("region",{name:"Build and test annotation plan",exact:true})).toBeVisible();
  expect(mutations).toHaveLength(count);
});
