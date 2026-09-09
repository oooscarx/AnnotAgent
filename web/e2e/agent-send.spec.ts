import { randomUUID } from "node:crypto";
import { expect, test } from "./fixtures";

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
