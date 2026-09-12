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
  setupContextFromReadiness,
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
  compatible_model_ids: ["model-profile:planner", "model-profile:vision"],
  setup_status: "required",
  draft_id: "d",
  draft_revision: 1,
  draft_content_hash: "draft-hash",
  authorization_fingerprint: "old-auth",
  allowed_models: [{ model_id: "model-profile:vision", binding_digest: "binding-hash" }],
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
    if (path.endsWith("/tasks/t/capability-readiness")) {
      const pluginModels = (overrides.plugins?.models ?? []).map((item) => ({
        id: item.selection_id,
        candidate_type: "plugin_model",
        revision: item.reference.model_profile_revision,
        digest: item.reference.capability_contract_hash,
        roles: ["visual"],
        capabilities: item.capabilities,
        quality_contracts: [],
        readiness: item.availability === "available" ? "ready" : "unknown",
        production_eligible: item.selectable && item.availability === "available",
        test_fixture: false,
        blocker: null,
        project_bindings: [],
        allowed_by_current_scope: false,
        setup: { kind: "plugin", api_url: "/api/plugins" },
      }));
      const instanceModels = (overrides.instanceProfiles ?? []).map((item) => {
        const installed = overrides.instances?.find((instance) => instance.id === item.model_instance_id);
        const ready = item.availability === "available" && installed?.status === "ready";
        return ({
        id: item.selection_id ?? item.model_instance_id,
        candidate_type: "model_instance",
        model_profile_id: item.model_profile_id,
        model_instance_id: item.model_instance_id,
        revision: item.model_profile_revision,
        digest: `digest-${item.model_instance_id}`,
        roles: ["visual"],
        capabilities: item.capabilities,
        quality_contracts: [],
        readiness: ready ? "ready" : "unknown",
        production_eligible: item.selectable && ready,
        test_fixture: false,
        blocker: null,
        project_bindings: [],
        allowed_by_current_scope: false,
        setup: { kind: "model_instance", api_url: `/api/model-instances/${item.model_instance_id}` },
        });
      });
      return {
        contract_version: "mainline-capability-v1",
        project_id: "p",
        project_owner_id: "owner",
        conversation_id: "c",
        task_id: "t",
        task_schema_revision: overrides.taskRevision?.() ?? "schema-1",
        draft: { id: "d", revision: overrides.draftRevision?.() ?? 1, content_hash: "draft-hash", status: "editing" },
        registry_revision: "registry-1",
        registry_revision_kind: "snapshot_sha256",
        candidates: [
          ...models.map((item) => ({
            id: `model-profile:${item.id}`,
            candidate_type: "model_profile",
            model_profile_id: item.id,
            revision: item.revision,
            digest: `digest-${item.id}`,
            roles: item.task_capabilities.includes("text_generation") ? ["agent"] : ["classification"],
            capabilities: item.task_capabilities,
            quality_contracts: [],
            readiness: item.status === "available" ? "ready" : item.status === "unknown" || item.status === "unverified" ? "unknown" : item.status === "disabled" ? "disabled" : "unavailable",
            production_eligible: item.status === "available",
            test_fixture: false,
            blocker: item.status === "available" ? null : { code: "not_verified", message: "not verified" },
            project_bindings: [],
            allowed_by_current_scope: false,
            selected_for_next_agent_request: item.id === "planner",
            setup: { kind: "model_profile", api_url: `/api/model-profiles/${item.id}` },
          })),
          ...pluginModels,
          ...instanceModels,
        ],
        setup_requests: [{
          id: "setup",
          project_id: "p",
          task_id: "t",
          task_revision: overrides.taskRevision?.() ?? "schema-1",
          registry_revision: "registry-1",
          role: "task_planning",
          required_capabilities: ["text_generation"],
          compatible_model_ids: models
            .filter((item) => item.task_capabilities.includes("text_generation"))
            .filter((item) => item.status === "available" || item.status === "unknown" || item.status === "unverified")
            .map((item) => `model-profile:${item.id}`),
          status: "required",
          return_path: "/projects/p/work?task=t&draft=d",
        }],
        visual_readiness_boundary: {
          status: "validate_exact_draft",
          reason: "Validate the exact frozen Draft bindings through existing previews.",
          builder_preview_url: "/api/projects/p/conversations/c/tasks/t/builder-preview",
          sample_preview_url: "/api/projects/p/conversations/c/tasks/t/sample-preview",
        },
        agent_model_preference: { revision: overrides.preferenceRevision?.() ?? 1, model_profile_id: "planner" },
        authorization: { source: "journey_consent", consent_id: "consent", expires_at: "2027-01-01", permission_digest: "old-auth", allowed_models: context.allowed_models, active: true, can_resume_without_authorization: false },
        budget: {},
        task_cost: { scope: "conversation_task_model_calls", receipt_count: 0, known: true, amount: "0", currency: null, reason: null },
        passive: true,
        setup_recheck_only: true,
        auto_expands_allowed_models: false,
        consistency: "server_composed_versioned_snapshot",
      };
    }
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
      compatible_model_ids: ["model-profile:planner", "model-profile:vision"],
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
    expect(calls.filter((item) => item.path.endsWith("/capability-readiness"))).toHaveLength(1);
    expect(calls.some((item) => item.path.includes("probe"))).toBe(false);
  });

  it("builds the continuation only from the owned B4 readiness revision", async () => {
    const snapshot = await fixture().service.inspect(context, new AbortController().signal);
    const request = snapshot.readiness.setup_requests[0];
    expect(request.role).toBe("task_planning");
    expect(request.required_capabilities).toEqual(["text_generation"]);
    expect(snapshot.readiness.visual_readiness_boundary).toMatchObject({
      status: "validate_exact_draft",
      builder_preview_url: expect.stringContaining("/builder-preview"),
      sample_preview_url: expect.stringContaining("/sample-preview"),
    });
    const built = setupContextFromReadiness(request, snapshot.readiness, "2026-01-01");
    expect(built).toMatchObject({
      id: "setup",
      project_id: "p",
      conversation_id: "c",
      task_id: "t",
      task_revision: "schema-1",
      draft_id: "d",
      draft_revision: 1,
      allowed_models: context.allowed_models,
      compatible_model_ids: ["model-profile:planner"],
      return_to: "/projects/p/work?task=t&draft=d",
    });
    expect(built.requirements).toEqual([
      expect.objectContaining({ id: "capability:text_generation", capability: "text_generation" }),
    ]);
    expect(built.requirements.every((item) => !("target" in item))).toBe(true);
    expect(() => setupContextFromReadiness({ ...request, registry_revision: "stale" }, snapshot.readiness, "2026-01-01")).toThrow("revision");
    expect(() => setupContextFromReadiness({ ...request, compatible_model_ids: [] }, snapshot.readiness, "2026-01-01")).toThrow("snapshot");
  });

  it("surfaces unknown availability as uncertain instead of a certain failure", async () => {
    const { service } = fixture({ models: [model("planner", "text_generation"), model("vision", "image_classification", "unknown")] });
    const result = await service.inspect(context, new AbortController().signal);
    expect(result.provider_models.find((item) => item.id === "vision")?.state).toBe("uncertain");
    expect(result.requirements.find((item) => item.requirement.id === "vision")?.uncertain_candidate_ids).toEqual(["model-profile:vision"]);
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

  it("treats Provider, Plugin and Model Instance as alternatives for one capability", async () => {
    const localContext: SetupContext = {
      ...context,
      compatible_model_ids: ["instance-selection", "model-profile:segment", "plugin-selection"],
      requirements: [
        { id: "capability:prompted_segmentation", capability: "prompted_segmentation", input_modalities: ["image"], purpose: "执行提示分割" },
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
      selection_id: "instance-selection",
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
      models: [model("segment", "prompted_segmentation")],
      plugins: { installations: [installation], models: [{ selection_id: "plugin-selection", reference: { plugin_id: "plugin", plugin_version: "1", package_digest: "plugin-sha", plugin_api_version: "1", protocol_version: "1", model_id: "segment", model_profile_revision: 1, capability_contract_hash: "contract" }, display_name: "Segment Plugin", capabilities: ["prompted_segmentation"], availability: "available", plugin_status: "ready", enabled: true, selectable: true }], agent_permissions: { discover: true, install: false, accept_licenses: false, provision_weights: false } },
      instances: [instance],
      instanceProfiles: [profile],
      installedBundles: [bundle],
    });
    const result = await service.inspect(localContext, new AbortController().signal);
    expect(result.plugins[0].state).toBe("ready");
    expect(result.model_instances[0].state).toBe("ready");
    expect(result.requirements).toHaveLength(1);
    expect(result.requirements[0].alternatives.map((item) => [item.id, item.target])).toEqual([
      ["model-profile:segment", "provider_model"],
      ["plugin-selection", "plugin"],
      ["instance-selection", "model_instance"],
    ]);
    expect(result.requirements[0].ready_candidate_ids).toEqual([
      "model-profile:segment",
      "plugin-selection",
      "instance-selection",
    ]);

    const broken = fixture({
      models: [model("segment", "prompted_segmentation")],
      plugins: { installations: [installation], models: [{ selection_id: "plugin-selection", reference: { plugin_id: "plugin", plugin_version: "1", package_digest: "plugin-sha", plugin_api_version: "1", protocol_version: "1", model_id: "segment", model_profile_revision: 1, capability_contract_hash: "contract" }, display_name: "Segment Plugin", capabilities: ["prompted_segmentation"], availability: "available", plugin_status: "ready", enabled: true, selectable: true }], agent_permissions: { discover: true, install: false, accept_licenses: false, provision_weights: false } },
      instances: [{ ...instance, status: "loading" }],
      instanceProfiles: [profile],
      installedBundles: [bundle],
    });
    const notReady = await broken.service.inspect(localContext, new AbortController().signal);
    expect(notReady.plugins[0].state).toBe("ready");
    expect(notReady.model_instances[0].state).toBe("uncertain");
    const recheck = await broken.service.recheck(notReady, new AbortController().signal);
    expect(recheck.capability_prepared).toBe(true);
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
    expect(setupSettingsPath(context, "plugin", "plugin-model:segment")).toBe("/settings/plugins?setup_request=setup&candidate=plugin-model%3Asegment");
    expect(setupReturnPath(context, "configured")).toBe("/projects/p/work?task=t&draft=d&setup_request=setup&setup_outcome=configured");
    expect(() => preserveSetupContext(storage, { ...context, return_to: "https://example.com" })).toThrow("同一 Project 和 Task");
    expect(() => preserveSetupContext(storage, { ...context, allowed_models: [...context.allowed_models, context.allowed_models[0]] })).toThrow("重复模型");
  });
});
