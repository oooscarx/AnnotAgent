import { expect, test } from "./fixtures";

test("TEST long journal opens a bounded tail and loads older notes without inference or navigation", async ({ page, request }) => {
  const project = `TEST-history-${Date.now()}`;
  expect((await request.post("/api/projects", { data: { id: project, yaml: "version: 1\nproject:\n  name: TEST journal paging\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n" } })).ok()).toBeTruthy();
  const root = `/api/projects/${project}/conversations`;
  const conversation = (await (await request.post(root)).json()).conversation_id;
  // Explicit browser metadata fixture. Backend paging/restart/owner behavior has
  // separate real SQLite and HTTP tests; no production workspace is populated.
  const messages = Array.from({ length: 250 }, (_, index) => ({ conversation_id: conversation, sequence: index + 1, input: { id: `TEST-note-${index + 1}`, text: `TEST historical note ${index + 1}`, image: null } }));
  const reads: string[] = [];
  await page.route(`**${root}/${conversation}/messages?*`, route => {
    const url = new URL(route.request().url()); reads.push(url.search);
    const before = Number(url.searchParams.get("before"));
    const after = Number(url.searchParams.get("after"));
    const selected = url.searchParams.has("first_goal") ? messages.slice(0, 1) : url.searchParams.has("latest") ? messages.slice(-100) : before ? messages.filter(item => item.sequence < before).slice(-100) : messages.filter(item => item.sequence > after).slice(0, 100);
    return route.fulfill({ json: selected });
  });
  const writes: string[] = [];
  page.on("request", req => { if (req.method() !== "GET" && req.url().includes("/api/")) writes.push(req.url()); });
  await page.goto(`/projects/${project}/work?conversation=${conversation}`);
  const list = page.getByRole("list", { name: "Saved messages", exact: true });
  await expect(list.getByRole("listitem")).toHaveCount(100);
  await expect(list).toContainText("TEST historical note 250");
  expect(reads).toEqual(["?latest=true&limit=100", "?first_goal=true"]);
  const url = page.url();
  await page.getByRole("button", { name: "Load earlier messages", exact: true }).click();
  await expect(list.getByRole("listitem")).toHaveCount(200);
  expect(reads.at(-1)).toBe("?before=151&limit=100");
  expect(page.url()).toBe(url);
  await page.getByRole("button", { name: "Load earlier messages", exact: true }).click();
  await expect(list.getByRole("listitem")).toHaveCount(250);
  await expect(page.getByRole("button", { name: "Load earlier messages", exact: true })).toHaveCount(0);
  await page.reload();
  await expect(list.getByRole("listitem")).toHaveCount(100);
  expect(page.url()).toBe(url);
  expect(writes).toEqual([]);
});
