import { randomUUID } from "node:crypto";
import { expect, test } from "./fixtures";

test("authorized conversation Schema crosses actual HTTP Provider transport once and restores receipts", async ({ page, request }) => {
  const provider = await (await request.post("/api/providers", { data: { display_name:"Conversation TEST fixture", adapter:"open_ai_compatible",base_url:"http://127.0.0.1:8796/openai/v1" } })).json();
  expect((await request.post(`/api/providers/${provider.id}/credential`,{data:{source:"workspace_file",secret:"TEST-conversation-protocol-only"}})).ok()).toBeTruthy();
  const model = await (await request.post("/api/model-profiles",{data:{provider_id:provider.id,display_name:"Conversation TEST text model",remote_model_id:"e2e-conversation-schema",input_modalities:["text"],task_capabilities:["text_generation"],protocol_features:{tool_calls:true,structured_output:true}}})).json();
  expect((await request.post(`/api/providers/${provider.id}/active-probe`,{data:{model_profile_id:model.id,confirmed_billable:true}})).ok()).toBeTruthy();
  const project = `schema-test-${Date.now()}`;
  const yaml = "version: 1\nproject:\n  name: TEST schema transport\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n";
  expect((await request.post("/api/projects",{data:{id:project,yaml}})).ok()).toBeTruthy();
  const root = `/api/projects/${project}/conversations`;
  const conversation = (await (await request.post(root)).json()).conversation_id;
  const revision = (await (await request.get(`/api/projects/${project}/goal`)).json()).revision;
  for (const [goal,kind] of [["Find cups, not bottles","bounding_box"],["按室内和室外给图片分类","classification"]]) {
    const message = randomUUID();
    expect((await request.post(`${root}/${conversation}/messages`,{data:{id:message,text:goal,image:null}})).ok()).toBeTruthy();
    const task = randomUUID();
    expect((await request.post(`${root}/${conversation}/tasks`,{data:{id:task,source_message_id:message,schema_revision:revision}})).ok()).toBeTruthy();
    const taskRoot = `${root}/${conversation}/tasks/${task}`;
    const previewResponse = await request.get(`${taskRoot}/schema-preview?model_id=${model.id}`);
    expect(previewResponse.ok()).toBeTruthy();
    const preview = await previewResponse.json();
    expect(preview.estimated_cost).toBeNull(); expect(preview.maximum_calls).toBe(1); expect(preview.image_count).toBe(0);
    const consent = {call_id:randomUUID(),model_id:model.id,scope_hash:preview.scope_hash,expires_at:preview.expires_at,allow_unknown_cost:true};
    expect((await request.post(`${taskRoot}/schema-proposals`,{data:{...consent,allow_unknown_cost:false}})).status()).toBe(400);
    expect((await request.post(`${taskRoot}/schema-proposals`,{data:{...consent,scope_hash:"stale"}})).status()).toBe(400);
    const response = await request.post(`${taskRoot}/schema-proposals`,{data:consent});
    expect(response.ok()).toBeTruthy(); const receipt = await response.json();
    expect(receipt.status).toBe("completed");
    expect(receipt.evidence.decision.Ok.kind).toBe(kind);
    expect(receipt.evidence.response.usage.total_tokens).toBe(48);
    expect((await (await request.post(`${taskRoot}/schema-proposals`,{data:consent})).json())).toEqual(receipt);
    expect((await (await request.get(`${taskRoot}/calls/${consent.call_id}`)).json())).toEqual(receipt);
    expect((await request.post(`${taskRoot}/schema-proposals`,{data:{...consent,call_id:randomUUID()}})).status()).toBe(400);
  }
  expect((await (await request.get(`/api/projects/${project}/goal`)).json()).revision).toBe(revision);
  const defaults = await (await request.get("/api/agent-model-bindings")).json();
  expect((await request.put("/api/agent-model-bindings",{data:{...defaults,pipeline_builder:model.id}})).ok()).toBeTruthy();
  const uiProject = `${project}-ui`;
  expect((await request.post("/api/projects",{data:{id:uiProject,yaml}})).ok()).toBeTruthy();
  await page.goto(`/projects/${uiProject}/work`);
  await page.getByLabel("Your message",{exact:true}).fill("TEST find cups, not bottles");
  await page.getByRole("button",{name:"Save message",exact:true}).click();
  await page.getByRole("button",{name:"Prepare label proposal",exact:true}).click();
  await expect(page.getByLabel("Schema model authorization",{exact:true})).toBeVisible();
  await expect(page.getByRole("button",{name:"Generate label proposal",exact:true})).toBeDisabled();
  await page.getByRole("checkbox",{name:/Allow this text request/}).check();
  await page.getByRole("button",{name:"Generate label proposal",exact:true}).click();
  await expect(page.getByText("Schema proposal saved",{exact:true})).toBeVisible();
  await page.reload();
  await expect(page.getByText("Schema proposal saved",{exact:true})).toBeVisible();
  await expect(page.getByText("Object boxes",{exact:true})).toBeVisible();
  for (const width of [1440,390]) {
    await page.setViewportSize({width,height:width===390?844:900});
    expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    await page.screenshot({path:`../docs/execution/conversational-workspace/schema-${width}.png`,fullPage:true,animations:"disabled"});
  }
});
