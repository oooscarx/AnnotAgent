import { randomUUID } from "node:crypto";
import { expect, test, fetchWithinMutationLimit } from "./fixtures";

test("server send freezes task admission without inference or duplicate tasks", async ({ request }) => {
  const project = `TEST-agent-send-${Date.now()}`;
  expect((await request.post("/api/projects", { data: { id: project, yaml:
    "version: 1\nproject:\n  name: TEST atomic send\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n",
  } })).ok()).toBe(true);
  const conversation = (await (await request.post(`/api/projects/${project}/conversations`, { data: {} })).json()).conversation_id;
  const root = `/api/projects/${project}/conversations/${conversation}`;
  const revision = (await (await request.get(`/api/projects/${project}/goal`)).json()).revision;
  const input = { message: { id: randomUUID(), text: "TEST find cups", image: null }, task_id: null, schema_revision: revision };
  const firstResponse = await request.post(`${root}/send`, { data: input });
  expect(firstResponse.ok(), await firstResponse.text()).toBe(true);
  const first = await firstResponse.json();
  expect(first.disposition).toBe("new_task");
  const recovered = await (await request.get(`${root}/send/${input.message.id}`)).json();
  expect(recovered.receipt).toEqual(first);
  expect(recovered.input).toEqual(input);
  expect(await (await request.get(`${root}/send/${randomUUID()}`)).json()).toBeNull();
  expect((await request.get(`/api/projects/foreign-owner/conversations/${conversation}/send/${input.message.id}`)).ok()).toBe(false);
  expect(await (await request.post(`${root}/send`, { data: input })).json()).toEqual(first);
  expect((await request.post(`${root}/send`, { data: { ...input, task_id: first.task_id } })).ok()).toBe(false);
  expect((await request.post(`${root}/send`, { data: { ...input, execute: true } })).ok()).toBe(false);
  const followup = { ...input, task_id: first.task_id, message: { ...input.message, id: randomUUID(), text: "TEST exclude bottles" } };
  const second = await (await request.post(`${root}/send`, { data: followup })).json();
  expect(second.task_id).toBe(first.task_id);
  expect(second.disposition).toBe("task_message");
  expect(await (await request.get(`${root}/tasks`)).json()).toHaveLength(1);
  expect(await (await request.get(`${root}/messages`)).json()).toHaveLength(2);
  const budget = await (await request.get(`${root}/tasks/${first.task_id}/budget`)).json();
  expect(budget.total_authorized_calls).toBe(0);
  expect(budget.total_reserved_calls).toBe(0);
});

for (const admitted of [true, false]) test(`pending send refresh reads receipt without repeating POST (admitted=${admitted})`, async ({ page, request }) => {
  const project = `TEST-send-refresh-${admitted}-${Date.now()}`;
  expect((await request.post("/api/projects", { data: { id: project, yaml:
    "version: 1\nproject:\n  name: TEST send refresh\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n",
  } })).ok()).toBe(true);
  const commands: unknown[] = [];
  await page.route(`**/projects/${project}/conversations/*/send`, async route => {
    commands.push(route.request().postDataJSON());
    if (commands.length === 1) {
      if (admitted) expect((await fetchWithinMutationLimit(route)).ok()).toBe(true);
      await route.abort("failed");
    } else await route.continue();
  });
  await page.goto(`/projects/${project}/work`);
  const input = page.getByRole("textbox", { name: "Your message", exact: true });
  const mode=page.getByRole("combobox",{name:"Next message mode",exact:true});
  await mode.selectOption("execute");
  await input.fill("TEST original frozen target");
  await page.getByRole("button", { name: "Send", exact: true }).click();
  await expect(page.getByRole("button", { name: "Retry same send", exact: true })).toBeEnabled();
  await expect(mode).toBeDisabled();
  await page.reload();
  if (admitted) {
    await expect(input).toHaveValue("");
    await expect(page.getByRole("list", { name: "Saved messages", exact: true })).toContainText("TEST original frozen target");
    expect(commands).toHaveLength(1);
  } else {
    await expect(mode).toHaveValue("execute");
    await expect(mode).toBeDisabled();
    await expect(input).toHaveValue("TEST original frozen target");
    await expect(page.getByRole("button", { name: "Retry same send", exact: true })).toBeEnabled();
    expect(commands).toHaveLength(1);
    await page.getByRole("button", { name: "Retry same send", exact: true }).click();
    await expect(input).toHaveValue("");
    expect(commands[1]).toEqual(commands[0]);
  }
});

test("Composer retries the identical frozen send after a lost acknowledgement", async ({ page, request }) => {
  const project = `TEST-agent-send-retry-${Date.now()}`;
  expect((await request.post("/api/projects", { data: { id: project, yaml:
    "version: 1\nproject:\n  name: TEST Composer retry\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n",
  } })).ok()).toBe(true);
  const sends: unknown[] = [];
  await page.route(`**/projects/${project}/conversations/*/send`, async route => {
    sends.push(route.request().postDataJSON());
    if (sends.length === 1) {
      const response = await fetchWithinMutationLimit(route);
      expect(response.ok(), await response.text()).toBe(true);
      await route.abort("failed");
    } else await route.continue();
  });
  await page.goto(`/projects/${project}/work`);
  const input = page.getByRole("textbox", { name: "Your message", exact: true });
  await input.fill("TEST find yellow cylinders");
  await page.getByRole("button", { name: "Send", exact: true }).click();
  await expect(page.getByRole("button", { name: "Retry same send", exact: true })).toBeEnabled();
  await expect(input).toHaveValue("TEST find yellow cylinders");
  await page.getByRole("button", { name: "Open data and results", exact: true }).click();
  await page.getByRole("button", { name: "Retry same send", exact: true }).click();
  await expect(input).toHaveValue("");
  expect(sends).toHaveLength(2);
  expect(sends[1]).toEqual(sends[0]);
  const conversation = (await (await request.get(`/api/projects/${project}/conversations`)).json()).conversation_id;
  const root = `/api/projects/${project}/conversations/${conversation}`;
  expect(await (await request.get(`${root}/tasks`)).json()).toHaveLength(1);
  expect(await (await request.get(`${root}/messages`)).json()).toHaveLength(1);
  await page.reload();
  await expect(page.getByRole("list", { name: "Saved messages", exact: true })).toContainText("TEST find yellow cylinders");
  expect(sends).toHaveLength(2);
});
