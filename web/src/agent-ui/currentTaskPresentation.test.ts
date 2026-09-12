import { describe, expect, it } from "vitest";
import type { Task } from "./adapter";
import type { MainlineAction, MainlineTaskView } from "./mainline";
import { selectCurrentTaskPresentation } from "./currentTaskPresentation";

const action = (
  id: string,
  state: MainlineAction["state"],
): MainlineAction => ({
  id,
  state,
  method: state === "requires_confirmation" ? "GET" : "POST",
  url: `/api/TEST/${id}`,
  execution_method: state === "requires_confirmation" ? "POST" : undefined,
  execution_url:
    state === "requires_confirmation" ? `/api/TEST/${id}/execution` : undefined,
  requires_confirmation: state === "requires_confirmation",
  reason: null,
});

const view = (overrides: Partial<MainlineTaskView> = {}): MainlineTaskView => ({
  contract_version: "mainline-task-v1",
  project_id: "TEST-project",
  project_owner_id: "TEST-owner",
  conversation_id: "TEST-conversation",
  task_id: "TEST-task",
  read_model_revision: "read-1",
  intake: {
    missing_slots: [],
    dataset_scope: [{ image_id: "image-1" }],
    label_rules: [{ display_name: "cup" }],
    training_target: { annotation_kind: "bounding_box" },
  },
  delivery: null,
  schema: null,
  review_summary: {
    selected_images: 3,
    saved_review_receipts: 0,
    current_reviews: 0,
    pending_reviews: 0,
  },
  package: { consents: [], jobs: [] },
  available_actions: [],
  blockers: [],
  completion: {
    model_request_completed: false,
    processing_completed: false,
    package_ready: false,
    task_completed: false,
  },
  ...overrides,
});

const task = (mainline: MainlineTaskView, overrides: Partial<Task> = {}): Task => ({
  id: "TEST-task",
  project: "TEST-project",
  conversationId: "TEST-conversation",
  title: "Cup detection",
  phase: "idle",
  revision: "schema-1",
  items: [],
  queue: [],
  draft: "",
  model: "",
  boxes: [],
  image: "image-1",
  mainline,
  ...overrides,
});

describe("single current-task presentation", () => {
  it("asks only for the missing task information", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          intake: {
            missing_slots: ["training_target"],
            dataset_scope: [{ image_id: "image-1" }],
            label_rules: [{ display_name: "cup" }],
            training_target: null,
          },
        }),
      ),
    );

    expect(result).toMatchObject({
      kind: "needs_information",
      missing: ["training_target"],
    });
    expect(result.primary).toBeUndefined();
  });

  it("presents one current scope approval for builder plus sample", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          available_actions: [
            action("build_and_test_pipeline", "requires_confirmation"),
          ],
        }),
      ),
    );

    expect(result).toMatchObject({
      kind: "ready_to_start",
      primary: { kind: "prepare_sample" },
    });
  });

  it("never exposes a second same-scope Sample execution approval", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          available_actions: [
            action("test_pipeline_samples", "requires_confirmation"),
          ],
        }),
      ),
    );

    expect(result.kind).toBe("blocked");
    expect(result.primary).toBeUndefined();
    expect(result.detail).toContain("授权");
  });

  it("uses actual server activity as the running state", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          steps: [
            {
              id: "builder",
              kind: "builder",
              title: "生成标注方法",
              status: "running",
            },
          ],
          active_operation_ids: ["operation-1"],
        }),
        {
          phase: "running",
          actions: { stop: { available: true, reason: "server operation active" } },
        },
      ),
    );

    expect(result).toMatchObject({
      kind: "running",
      title: "生成标注方法",
      primary: { kind: "stop" },
    });
  });

  it("moves directly to the real review decision when results need a human", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          review_summary: {
            selected_images: 3,
            saved_review_receipts: 0,
            current_reviews: 1,
            pending_reviews: 2,
          },
          review_work_item_id: "review-1",
        }),
        {
          phase: "waiting_for_human",
          humanQuestion: "这一张是否标完整？",
        },
      ),
    );

    expect(result).toMatchObject({
      kind: "needs_review",
      title: "这一张是否标完整？",
      primary: { kind: "open_review" },
    });
  });

  it("defines delivery by the reconciled Ready package", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          package: {
            consents: [],
            jobs: [
              {
                id: "package-1",
                phase: "ready",
                intent_revision: 1,
                snapshot_sha256: "snapshot",
              },
            ],
          },
          completion: {
            model_request_completed: true,
            processing_completed: true,
            package_ready: true,
            task_completed: true,
            status: "package_ready",
            package_id: "package-1",
            download_url: "/api/packages/package-1/download",
          },
        }),
      ),
    );

    expect(result).toMatchObject({
      kind: "delivered",
      primary: { kind: "download_package", id: "package-1" },
    });
  });
});
