import { expect, it } from "vitest";
import { readSetupSettingsContext } from "./SetupSettingsReturn";
import { preserveSetupContext, type SetupContext } from "./modelPreparation";

const context = {
  id: "setup-id",
  project_id: "project",
  conversation_id: "conversation",
  task_id: "task",
  task_revision: "revision",
  registry_revision: "registry-revision",
  role: "pipeline_builder",
  compatible_model_ids: [],
  setup_status: "required",
  allowed_models: [],
  return_to: "/projects/project/work?task=task",
  requirements: [{ id: "planner", target: "agent_model", capability: "text_generation", input_modalities: ["text"], purpose: "Plan" }],
  created_at: "2026-01-01",
} as SetupContext;

function memory() {
  const values = new Map<string, string>();
  return {
    setItem: (key: string, value: string) => values.set(key, value),
    getItem: (key: string) => values.get(key) ?? null,
    removeItem: (key: string) => values.delete(key),
  } as unknown as Storage;
}

it("restores only the exact task-scoped settings return token", () => {
  const storage = memory();
  preserveSetupContext(storage, context);
  expect(readSetupSettingsContext(storage, "?setup_request=setup-id")).toEqual(context);
  expect(readSetupSettingsContext(storage, "")).toBeUndefined();
  expect(() => readSetupSettingsContext(storage, "?setup_request=other")).toThrow("不存在");
});
