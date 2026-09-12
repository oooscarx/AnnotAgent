import { describe, expect, it } from "vitest";
import type { request } from "../api";
import type {
  ExpertPluginRegistry,
  InstalledModelBundle,
  InstalledModelInstance,
  ModelCatalogEntry,
  ModelInstanceProfile,
  ProviderProfile,
  RegistryModelProfile,
} from "../types";
import {
  createModelPreparationService,
  preserveSetupContext,
  restoreSetupContext,
  setupReturnPath,
  setupSettingsPath,
  setupContextFromCapabilityRequest,
  type SetupContext,
} from "./modelPreparation";

const protocols = {
  tool_calls: true,
  parallel_tool_calls: false,
  structured_output: true,
  json_schema: true,
  usage_reporting: false,
  streaming: false,
  reasoning_controls: false,
};

function model(
  id: string,
  capability: RegistryModelProfile["task_capabilities"][number],
  status: RegistryModelProfile["status"] = "available",
): RegistryModelProfile {
  return {
    id,
    revision: 2,
    provider_id: "provider",
    display_name: id,
    remote_model_id: `${id}-remote`,
    input_modalities: capability === "text_generation" ? ["text"] : ["image"],
    protocol_features: protocols,
    task_capabilities: [capability],
    capability_source: status === "available" ? "provider_discovered" : "user_declared",
    limits: {},
    generation_defaults: {},
    pricing: { currency: "USD", source: "unknown" },
    status,
    enabled: true,
    locked: false,
    created_at: "2026-01-01",
    updated_at: "2026-01-01",
  };
}

const provider = {
  id: "provider",
  display_name: "Remote Provider",
  adapter: "open_ai_compatible",
  base_url: "https://example.invalid/v1",
  endpoint_summary: "example.invalid",
  safe_headers: {},
  connection_policy: {},
  enabled: true,
  health: { status: "available" },
  credential_configured: true,
  model_count: 2,
} as ProviderProfile;

const context: SetupContext = {
  id: "setup",
  project_id: "p",
  conversation_id: "c",
  task_id: "t",
  task_revision: "schema-1",
  registry_revision: "registry-1",
  role: "task_planning_and_vision",
  compatible_model_ids: ["planner", "vision"],
  setup_status: "required",
  draft_id: "d",
  draft_revision: 1,
  draft_content_hash: "draft-hash",
  authorization_fingerprint: "old-auth",
  allowed_models: [{ model_id: "vision", binding_digest: "binding-hash" }],
  return_to: "/projects/p/work?task=t&draft=d",
  requirements: [
    {
      id: "planner",
      target: "agent_model",
      capability: "text_generation",
      input_modalities: ["text"],
      tool_calls: true,
      structured_output: true,
      purpose: "构造任务方案",
    },
    {
      id: "vision",
      target: "provider_model",
      capability: "image_classification",
      input_modalities: ["image"],
      purpose: "分类图片",
    },
  ],
  created_at: "2026-01-01",
};

function fixture(overrides: {
  models?: RegistryModelProfile[];
  plugins?: ExpertPluginRegistry;
  instances?: InstalledModelInstance[];
  instanceProfiles?: ModelInstanceProfile[];
  installedBundles?: InstalledModelBundle[];
  catalog?: ModelCatalogEntry[];
  taskRevision?: () => string;
  draftRevision?: () => number;
  preferenceRevision?: () => number;
} = {}) {
  const calls: { path: string; init?: RequestInit }[] = [];
  const models = overrides.models ?? [model("planner", "text_generation"), model("vision", "image_classification")];
  const transport = (async (path: string, init?: RequestInit) => {
    calls.push({ path, init });
    if (path.includes("/tasks/t/workspace"))
      return { project_id: "p", conversation_id: "c", task: { input: { id: "t", schema_revision: overrides.taskRevision?.() ?? "schema-1" } } };
    if (path.startsWith("/api/model-profiles/compatible?")) {
      const query = new URL(path, "http://local").searchParams;
      const capability = query.get("capabilities");
      return { models: models.filter((item) => item.status !== "unknown" && item.task_capabilities.includes(capability as never)) };
    }
    if (path === "/api/model-profiles") return { models };
    if (path === "/api/providers") return { providers: [provider] };
    if (path === "/api/plugins") return overrides.plugins ?? { installations: [], models: [], agent_permissions: { discover: true, install: false, accept_licenses: false, provision_weights: false } };
    if (path === "/api/model-instances") return { instances: overrides.instances ?? [], model_profiles: overrides.instanceProfiles ?? [] };
    if (path === "/api/model-bundles") return { bundles: overrides.installedBundles ?? [] };
    if (path === "/api/model-bundles/available") return { bundles: overrides.catalog ?? [] };
    if (path === "/api/projects/p/model-bindings") return { project_id: "p", bindings: [] };
    if (path === "/api/projects/p/conversations/c/agent-model") return { revision: overrides.preferenceRevision?.() ?? 1, model_profile_id: "planner" };
    if (path === "/api/workflow-drafts/d?project_id=p") return { id: "d", project_id: "p", revision: overrides.draftRevision?.() ?? 1, content_hash: "draft-hash" };
    throw new Error(`unexpected ${path}`);
  }) as typeof request;
  return { calls, service: createModelPreparationService(transport) };
}

describe("task-scoped model preparation", () => {
  it("adapts the G0 CapabilitySetupRequest only with a frozen continuation scope", () => {
    const value = setupContextFromCapabilityRequest({
      id: "setup",
      project_id: "p",
      task_id: "t",
      task_revision: "schema-1",
      registry_revision: "registry-1",
      role: "task_planning_and_vision",
      required_capabilities: ["image_classification", "text_generation"],
      compatible_model_ids: ["planner", "vision"],
      status: "required",
      return_path: "/projects/p/work?task=t&draft=d",
    }, {
      conversation_id: "c",
      draft_id: "d",
      draft_revision: 1,
      draft_content_hash: "draft-hash",
      authorization_fingerprint: "old-auth",
      allowed_models: context.allowed_models,
      requirements: context.requirements,
      created_at: "2026-01-01",
    });
    expect(value).toEqual(context);
    expect(() => setupContextFromCapabilityRequest({
      id: "setup",
      project_id: "p",
      task_id: "t",
      task_revision: "schema-1",
      registry_revision: "registry-1",
      role: "vision",
      required_capabilities: ["image_classification"],
      compatible_model_ids: [],
      status: "required",
      return_path: "/projects/p/work?task=t",
    }, {
      conversation_id: "c",
      allowed_models: [],
      requirements: context.requirements,
      created_at: "2026-01-01",
    })).toThrow("requirements");
  });

  it("keeps planning and vision models separate and never writes or probes", async () => {
    const { calls, service } = fixture();
    const result = await service.inspect(context, new AbortController().signal);
    expect(result.provider_models.map((item) => [item.id, item.kind])).toEqual([
      ["planner", "agent_model"],
      ["vision", "provider_model"],
    ]);
    expect(result.provider_models.every((item) => item.cost.state === "unknown")).toBe(true);
    expect(result.requirements.every((item) => item.ready_candidate_ids.length === 1)).toBe(true);
    expect(calls.every((item) => !item.init?.method || item.init.method === "GET")).toBe(true);
    expect(calls.filter((item) => item.path.includes("/compatible?")).every((item) => item.path.includes("allow_unverified=true"))).toBe(true);
    expect(calls.some((item) => item.path.includes("probe"))).toBe(false);
  });

  it("surfaces unknown availability as uncertain instead of a certain failure", async () => {
    const { service } = fixture({ models: [model("planner", "text_generation"), model("vision", "image_classification", "unknown")] });
    const result = await service.inspect(context, new AbortController().signal);
    expect(result.provider_models.find((item) => item.id === "vision")?.state).toBe("uncertain");
    expect(result.requirements.find((item) => item.requirement.id === "vision")?.uncertain_candidate_ids).toEqual(["provider_model:vision"]);
    const recheck = await service.recheck(result, new AbortController().signal);
    expect(recheck.capability_prepared).toBe(false);
    expect(recheck.can_resume_without_authorization).toBe(false);
  });

  it("represents a multimodal profile separately for planning and vision roles", async () => {
    const shared = model("shared", "text_generation");
    shared.input_modalities = ["text", "image"];
    shared.task_capabilities = ["text_generation", "image_classification"];
    const { service } = fixture({ models: [shared] });
    const result = await service.inspect(context, new AbortController().signal);
    expect(result.provider_models.map((item) => [item.id, item.kind])).toEqual([
      ["shared", "agent_model"],
      ["shared", "provider_model"],
    ]);
  });

  it("requires Plugin, Model Bundle and Model Instance to be independently Ready", async () => {
    const localContext: SetupContext = {
      ...context,
      requirements: [
        { id: "plugin", target: "plugin", capability: "prompted_segmentation", input_modalities: ["image"], purpose: "执行提示分割" },
        { id: "instance", target: "model_instance", capability: "prompted_segmentation", input_modalities: ["image"], purpose: "加载实际模型权重" },
      ],
    };
    const installation = {
      enabled: true,
      status: "ready",
      package_sha256: "plugin-sha",
      manifest: { id: "plugin", version: "1", display_name: "Segment Plugin", models: [{ capabilities: ["prompted_segmentation"] }] },
    } as ExpertPluginRegistry["installations"][number];
    const instance = {
      id: "instance",
      status: "ready",
      plugin_id: "plugin",
      plugin_version: "1",
      plugin_package_sha256: "plugin-sha",
      model_bundle_id: "bundle",
      model_bundle_version: "1",
      model_bundle_sha256: "bundle-sha",
      contract_inspection: { valid: true, errors: [] },
      smoke_test_result: { status: "passed" },
      model_profile_revision: 3,
    } as unknown as InstalledModelInstance;
    const profile = {
      model_instance_id: "instance",
      model_profile_id: "instance-profile",
      model_profile_revision: 3,
      display_name: "Segment Instance",
      capabilities: ["prompted_segmentation"],
      availability: "available",
      selectable: true,
    } as ModelInstanceProfile;
    const bundle = {
      enabled: true,
      bundle_sha256: "bundle-sha",
      manifest: { id: "bundle", version: "1", publishable: true, fixture: false },
    } as InstalledModelBundle;
    const { service } = fixture({
      plugins: { installations: [installation], models: [], agent_permissions: { discover: true, install: false, accept_licenses: false, provision_weights: false } },
      instances: [instance],
      instanceProfiles: [profile],
      installedBundles: [bundle],
    });
    const result = await service.inspect(localContext, new AbortController().signal);
    expect(result.plugins[0].state).toBe("ready");
    expect(result.model_instances[0].state).toBe("ready");

    const broken = fixture({
      plugins: { installations: [installation], models: [], agent_permissions: { discover: true, install: false, accept_licenses: false, provision_weights: false } },
      instances: [{ ...instance, status: "loading" }],
      instanceProfiles: [profile],
      installedBundles: [bundle],
    });
    const notReady = await broken.service.inspect(localContext, new AbortController().signal);
    expect(notReady.plugins[0].state).toBe("setup_required");
    expect(notReady.model_instances[0].state).toBe("setup_required");
  });

  it("rechecks Task, Draft, model preference and authorization without expanding allowed models", async () => {
    let taskRevision = "schema-1";
    let draftRevision = 1;
    let preferenceRevision = 1;
    const { service } = fixture({ taskRevision: () => taskRevision, draftRevision: () => draftRevision, preferenceRevision: () => preferenceRevision });
    const before = await service.inspect(context, new AbortController().signal);
    taskRevision = "schema-2";
    draftRevision = 2;
    preferenceRevision = 2;
    const result = await service.recheck(before, new AbortController().signal);
    expect(result.capability_prepared).toBe(false);
    expect(result.can_resume_without_authorization).toBe(false);
    expect(result.changed).toEqual([
      "Task schema revision 已变化",
      "Draft revision 或内容摘要已变化",
      "Agent 模型偏好 revision 已变化",
    ]);
    expect(result.task_reused).toBe(true);
    expect(result.draft_reused).toBe(true);
    expect(result.allowed_models_expanded).toBe(false);
    expect(result.authorization).toBe("recheck_required");
    expect(result.snapshot.context.allowed_models).toEqual(context.allowed_models);
  });

  it("persists only a validated same-task return context", () => {
    const values = new Map<string, string>();
    const storage = {
      setItem: (key: string, value: string) => values.set(key, value),
      getItem: (key: string) => values.get(key) ?? null,
      removeItem: (key: string) => values.delete(key),
    } as unknown as Storage;
    preserveSetupContext(storage, context);
    expect(restoreSetupContext(storage, context.id)).toEqual(context);
    expect(setupSettingsPath(context, "agent_model")).toBe("/settings/agent-models?setup_request=setup");
    expect(setupSettingsPath(context, "provider_model")).toBe("/settings/vision-models?setup_request=setup");
    expect(setupReturnPath(context, "configured")).toBe("/projects/p/work?task=t&draft=d&setup_request=setup&setup_outcome=configured");
    expect(() => preserveSetupContext(storage, { ...context, return_to: "https://example.com" })).toThrow("同一 Project 和 Task");
    expect(() => preserveSetupContext(storage, { ...context, allowed_models: [...context.allowed_models, context.allowed_models[0]] })).toThrow("重复模型");
  });
});
