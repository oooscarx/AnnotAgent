import { expect, test } from "@playwright/test";
import { resolve } from "node:path";

const pixel = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=";

test("formal review is bound to the Task Batch child Run and preserves failed command retries", async ({ page }) => {
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async ([path, imageUrl]) => {
    const { React, createRoot, DeliveryReview } = await import(path);
    const host = document.createElement("main");
    document.body.replaceChildren(host);
    const state = { reads: [] as (string | null)[], commands: [] as unknown[] };
    Object.assign(window, { deliveryTest: state });
    const service = {
      image: async (_p: string, _t: string, id: string, run: string | null) => {
        state.reads.push(run);
        return {
          intent_revision: 1, intent_sha256: "intent",
          snapshot: { image_id: id, source_run_id: run, sha256: "snapshot", content_sha256: "image", annotations: [] },
          sources: [{ run_id: "other-task-run", model: "must be ignored", status: "completed", created_at: "TEST" }],
          review: null, confirmation_current: false, accepted_objects: 1, unresolved_objects: 0, notice: "TEST",
        };
      },
      confirmImage: async (_p: string, _t: string, input: unknown) => {
        state.commands.push(input);
        throw new Error("TEST revision conflict; no decision saved");
      },
    };
    createRoot(host).render(React.createElement(DeliveryReview, {
      service, project: "TEST", task: "TEST-task",
      formalResult: {
        project_id: "TEST", task_id: "TEST-task", processing_operation_id: "operation-one",
        batch_id: "batch-one", workflow_version: "workflow@1", status: "completed",
        images: [{ image_id: "image-one", child_run_id: "child-one" }],
      },
      images: [{ id: "image-one", name: "TEST original", src: imageUrl }],
    }));
  }, [`/@fs/${resolve("e2e/ui-preview/delivery-review-harness.tsx")}`, pixel]);

  await expect(page.getByRole("heading", { name: "检查当前任务结果" })).toBeVisible();
  await expect(page.getByText(/Batch batch-on.*child Run child-on/)).toBeVisible();
  await expect(page.getByLabel("正式标注来源", { exact: true })).toHaveCount(0);
  await expect(page.getByText("other-task-run")).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => (window as unknown as { deliveryTest: { reads: (string | null)[] } }).deliveryTest.reads)).toEqual(["child-one"]);

  const confirm = page.getByRole("button", { name: "确认整张图标注完整并继续", exact: true });
  await expect(confirm).toBeEnabled();
  await confirm.click();
  await expect(page.getByRole("alert")).toContainText("no decision saved");
  await confirm.click();
  const commands = await page.evaluate(() => (window as unknown as { deliveryTest: { commands: unknown[] } }).deliveryTest.commands);
  expect(commands).toHaveLength(2);
  expect(commands[0]).toEqual(commands[1]);
  expect(commands[0]).toMatchObject({ source_run_id: "child-one", image_id: "image-one" });
});

test("continuous review advances only after the formal image receipt is saved", async ({ page }) => {
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async ([path, imageUrl]) => {
    const { React, createRoot, DeliveryReview } = await import(path);
    const host = document.createElement("main");
    document.body.replaceChildren(host);
    const state = { saved: false, reads: [] as (string | null)[] };
    Object.assign(window, { continuousReviewTest: state });
    const service = {
      image: async (_p: string, _t: string, id: string, run: string | null) => {
        state.reads.push(run);
        return {
          intent_revision: 1, intent_sha256: "intent",
          snapshot: { image_id: id, source_run_id: run, sha256: `snapshot-${id}`, content_sha256: id, annotations: [] },
          sources: [], review: null, confirmation_current: false,
          accepted_objects: 1, unresolved_objects: 0, notice: "TEST",
        };
      },
      confirmImage: async () => {
        await new Promise((resolve) => setTimeout(resolve, 80));
        state.saved = true;
        return {};
      },
    };
    createRoot(host).render(React.createElement(DeliveryReview, {
      service, project: "TEST", task: "TEST-task",
      formalResult: {
        project_id: "TEST", task_id: "TEST-task", processing_operation_id: "operation-one",
        batch_id: "batch-one", workflow_version: "workflow@1", status: "completed",
        images: [
          { image_id: "image-one", child_run_id: "child-one" },
          { image_id: "image-two", child_run_id: "child-two" },
        ],
      },
      images: [
        { id: "image-one", name: "one", src: imageUrl },
        { id: "image-two", name: "two", src: imageUrl },
      ],
    }));
  }, [`/@fs/${resolve("e2e/ui-preview/delivery-review-harness.tsx")}`, pixel]);

  const confirm = page.getByRole("button", { name: "确认整张图标注完整并继续", exact: true });
  await expect(confirm).toBeEnabled();
  await confirm.click({ noWaitAfter: true });
  await expect(page).not.toHaveURL(/delivery_image=image-two/);
  await expect.poll(() => page.evaluate(() => (window as unknown as { continuousReviewTest: { saved: boolean } }).continuousReviewTest.saved)).toBe(true);
  await expect(page).toHaveURL(/delivery_image=image-two/);
  await expect.poll(() => page.evaluate(() => (window as unknown as { continuousReviewTest: { reads: (string | null)[] } }).continuousReviewTest.reads)).toContain("child-two");
});

test("sample issue emits a complete sample VisualSelection and never writes formal review", async ({ page }) => {
  await page.goto("/ui-preview?task=new&delivery_view=sample");
  await page.evaluate(async ([path, imageUrl]) => {
    const { React, createRoot, DeliveryReview } = await import(path);
    const host = document.createElement("main");
    document.body.replaceChildren(host);
    const annotation = {
      id: "sample-object", image_id: "image-one", task_id: "objects", label: "ball",
      value: { kind: "bounding_box", rect: [0.1, 0.1, 0.2, 0.2] },
      attributes: {}, source: "model", review_status: "needs_review", provenance: {}, created_at: "TEST",
    };
    const state = { selections: [] as unknown[], writes: 0 };
    Object.assign(window, { sampleSelectionTest: state });
    const service = {
      image: async () => { throw new Error("formal read must not run"); },
      editObject: async () => { state.writes += 1; },
      confirmImage: async () => { state.writes += 1; },
    };
    createRoot(host).render(React.createElement(DeliveryReview, {
      service, project: "TEST", task: "TEST-task",
      sampleResult: {
        project_id: "TEST", conversation_id: "conversation-one", task_id: "TEST-task", project_schema_revision: "schema-one",
        draft_id: "draft-one", draft_revision: 7,
        sample_test_id: "sample-one",
        images: [{ image_id: "image-one", image_sha256: "image-pixels", result_revision: "sample:feedback:3", source_artifacts: { "sample-object": "artifact-one" }, annotations: [annotation] }],
      },
      images: [{ id: "image-one", name: "TEST original", src: imageUrl }],
      onSampleIssue: (selection: unknown) => state.selections.push(selection),
    }));
  }, [`/@fs/${resolve("e2e/ui-preview/delivery-review-harness.tsx")}`, pixel]);

  await page.getByRole("button", { name: "Annotation list · 1", exact: true }).click();
  await page.getByRole("button", { name: /ball/ }).click();
  await page.getByRole("button", { name: "这个样例框有问题", exact: true }).click();
  const state = await page.evaluate(() => (window as unknown as { sampleSelectionTest: { selections: unknown[]; writes: number } }).sampleSelectionTest);
  expect(state.writes).toBe(0);
  expect(state.selections).toEqual([expect.objectContaining({
    project_id: "TEST", conversation_id: "conversation-one", task_id: "TEST-task", project_schema_revision: "schema-one",
    image: { image_id: "image-one", sha256: "image-pixels" },
    sample: { draft_id: "draft-one", draft_revision: 7, sample_test_id: "sample-one" },
    candidate: { candidate_id: "sample-object", source_artifact_id: "artifact-one" },
    annotation: { kind: "bounding_box", label: "ball" },
    result_revision: "sample:feedback:3",
  })]);
});
