import { expect, test } from "@playwright/test";

test("task archive, soft delete and restore stay project-scoped and leave the current task safely", async ({ page, request }) => {
  expect((await request.get("/api/health")).headers()["x-annotagent-fixture"]).toBe("external-model-only");
  const navigation = await (await request.get("/api/navigation")).json();
  const project = navigation.items.find((item: { title: string }) => item.title === "TEST Agent UI HTTP fixture");
  expect(project).toBeTruthy();
  const root = `/api/projects/${project.project_id}/conversations/${project.conversation_id}`;
  const tasks = await (await request.get(`${root}/task-navigation?limit=100`)).json();
  const selected = tasks.items.find((item: { state: string }) => item.state === "idle");
  expect(selected).toBeTruthy();
  const taskId = selected.task_id;
  const taskTitle = selected.title;

  const writes: string[] = [];
  page.on("request", request => {
    const path = new URL(request.url()).pathname;
    if (request.method() === "POST" && path.endsWith("/lifecycle")) writes.push(path);
  });

  await page.goto(`/projects/${project.project_id}/work?task=${taskId}`);
  await page.locator(".task-tree-row", { hasText: taskTitle }).locator(".task-lifecycle-menu summary").click();
  await page.getByRole("menuitem", { name: "归档", exact: true }).click();
  const archive = page.getByRole("dialog", { name: "归档任务" });
  await expect(archive).toContainText("完整对话、执行轨迹和 JSON 上下文都会保留");
  await archive.getByRole("button", { name: "归档任务", exact: true }).click();
  await expect(page).not.toHaveURL(new RegExp(`task=${taskId}`));
  await expect(page.locator(".task-link").filter({ hasText: taskTitle })).toHaveCount(0);

  await page.goto(`/projects/${project.project_id}/manage/tasks?view=archived`);
  const archived = page.getByRole("region", { name: "已归档任务" });
  await expect(archived.getByText(taskTitle, { exact: true })).toBeVisible();
  await archived.getByRole("button", { name: "恢复…", exact: true }).click();
  await page.getByRole("dialog", { name: "恢复任务" }).getByRole("button", { name: "恢复任务", exact: true }).click();
  await expect(archived.getByText("没有已归档任务。", { exact: true })).toBeVisible();

  await page.goto(`/projects/${project.project_id}/work?task=${taskId}`);
  await page.locator(".task-tree-row", { hasText: taskTitle }).locator(".task-lifecycle-menu summary").click();
  await page.getByRole("menuitem", { name: "移入回收站…", exact: true }).click();
  const remove = page.getByRole("dialog", { name: "将任务移入回收站" });
  await expect(remove).toContainText("可恢复的软删除");
  await expect(remove.getByRole("button", { name: "移入回收站", exact: true })).toBeDisabled();
  await remove.getByRole("checkbox").check();
  await remove.getByRole("button", { name: "移入回收站", exact: true }).click();
  await expect(page).not.toHaveURL(new RegExp(`task=${taskId}`));

  await page.goto(`/projects/${project.project_id}/manage/tasks?view=trashed`);
  const trash = page.getByRole("region", { name: "任务回收站" });
  await expect(trash.getByText(taskTitle, { exact: true })).toBeVisible();
  await trash.getByRole("button", { name: "恢复…", exact: true }).click();
  await page.getByRole("dialog", { name: "恢复任务" }).getByRole("button", { name: "恢复任务", exact: true }).click();
  await expect(trash.getByText("任务回收站为空。", { exact: true })).toBeVisible();
  expect(writes).toHaveLength(4);
});

test("task lifecycle menus close with Escape and do not submit any mutation", async ({ page, request }) => {
  const navigation = await (await request.get("/api/navigation")).json();
  const project = navigation.items.find((item: { title: string }) => item.title === "TEST Agent UI HTTP fixture");
  const tasks = await (await request.get(`/api/projects/${project.project_id}/conversations/${project.conversation_id}/task-navigation`)).json();
  await page.goto(`/projects/${project.project_id}/work?task=${tasks.items[0].task_id}`);
  let writes = 0;
  page.on("request", request => { if (request.method() === "POST" && request.url().endsWith("/lifecycle")) writes += 1; });
  const trigger = page.locator(".task-lifecycle-menu summary").first();
  await trigger.click();
  await expect(page.getByRole("menu")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toBeHidden();
  await expect(trigger).toBeFocused();
  expect(writes).toBe(0);
});
