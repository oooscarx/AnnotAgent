import { request } from "../api";
import type {
  ExpertPluginRegistry,
  InstalledModelBundle,
  InstalledModelInstance,
  InputModality,
  ModelCapability,
  ModelCatalogEntry,
  ModelInstanceProfile,
  ProjectModelBinding,
  ProviderProfile,
  RegistryModelProfile,
  WorkflowDraft,
} from "../types";

export type SetupTarget =
  | "agent_model"
  | "provider_model"
  | "plugin"
  | "model_instance";

export type SetupRequirement = {
  id: string;
  target: SetupTarget;
  capability: ModelCapability;
  input_modalities: InputModality[];
  tool_calls?: boolean;
  structured_output?: boolean;
  json_schema?: boolean;
  purpose: string;
};

export type FrozenAllowedModel = {
  model_id: string;
  binding_digest: string;
};

export type SetupContext = {
  id: string;
  project_id: string;
  conversation_id: string;
  task_id: string;
  task_revision: string;
  draft_id?: string;
  draft_revision?: number;
  draft_content_hash?: string;
  authorization_fingerprint?: string;
  allowed_models: FrozenAllowedModel[];
  return_to: string;
  requirements: SetupRequirement[];
  created_at: string;
};

export type PreparationState = "ready" | "uncertain" | "setup_required" | "blocked";

export type ProviderModelCandidate = {
  kind: "agent_model" | "provider_model";
  id: string;
  revision: number;
  display_name: string;
  remote_model_id: string;
  provider_id: string;
  provider_name: string;
  requirement_ids: string[];
  state: PreparationState;
  reasons: string[];
  capability_source: RegistryModelProfile["capability_source"];
  cost: { state: "known" | "unknown"; currency: string; summary: string };
};

export type PluginCandidate = {
  kind: "plugin";
  id: string;
  version: string;
  display_name: string;
  requirement_ids: string[];
  state: PreparationState;
  reasons: string[];
};

export type ModelInstanceCandidate = {
  kind: "model_instance";
  id: string;
  display_name: string;
  plugin_id: string;
  plugin_version: string;
  bundle_id: string;
  bundle_version: string;
  model_profile_id: string;
  model_profile_revision: number;
  requirement_ids: string[];
  state: PreparationState;
  reasons: string[];
};

export type BundleCandidate = {
  kind: "model_bundle";
  id: string;
  version: string;
  display_name: string;
  requirement_ids: string[];
  compatible_plugins: { plugin_id: string; plugin_version: string }[];
  state: "available" | "installed" | "blocked";
  reasons: string[];
};

export type SetupGuard = {
  task_revision: string;
  draft_revision?: number;
  draft_content_hash?: string;
  agent_model_revision: number;
  project_bindings: ProjectModelBinding[];
  authorization_fingerprint?: string;
  allowed_models: FrozenAllowedModel[];
};

export type PreparationSnapshot = {
  context: SetupContext;
  guard: SetupGuard;
  context_changes: string[];
  provider_models: ProviderModelCandidate[];
  plugins: PluginCandidate[];
  model_instances: ModelInstanceCandidate[];
  model_bundles: BundleCandidate[];
  requirements: {
    requirement: SetupRequirement;
    ready_candidate_ids: string[];
    uncertain_candidate_ids: string[];
  }[];
  authorization: "recheck_required";
};

export type PreparationRecheck = {
  snapshot: PreparationSnapshot;
  changed: string[];
  capability_prepared: boolean;
  can_resume_without_authorization: false;
  task_reused: true;
  draft_reused: true;
  allowed_models_expanded: false;
  authorization: "recheck_required";
};

type TaskWorkspace = {
  project_id: string;
  conversation_id: string;
  task: { input: { id: string; schema_revision: string } };
};

export type ModelPreparationTransport = typeof request;

export type ModelPreparationService = {
  inspect(context: SetupContext, signal: AbortSignal): Promise<PreparationSnapshot>;
  recheck(previous: PreparationSnapshot, signal: AbortSignal): Promise<PreparationRecheck>;
};

const esc = encodeURIComponent;

function stable<T>(value: T): T {
  if (Array.isArray(value)) return value.map(stable) as T;
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([key, item]) => [key, stable(item)]),
    ) as T;
  }
  return value;
}

function same(a: unknown, b: unknown) {
  return JSON.stringify(stable(a)) === JSON.stringify(stable(b));
}

function requirementMatchesModel(
  requirement: SetupRequirement,
  model: RegistryModelProfile,
) {
  return (
    model.task_capabilities.includes(requirement.capability) &&
    requirement.input_modalities.every((item) =>
      model.input_modalities.includes(item),
    ) &&
    (!requirement.tool_calls || model.protocol_features.tool_calls) &&
    (!requirement.structured_output ||
      model.protocol_features.structured_output) &&
    (!requirement.json_schema || model.protocol_features.json_schema)
  );
}

function requirementMatchesCapabilities(
  requirement: SetupRequirement,
  capabilities: ModelCapability[],
) {
  return capabilities.includes(requirement.capability);
}

function modelCost(model: RegistryModelProfile) {
  const values = [
    ["每次请求", model.pricing.per_request],
    ["每张图片", model.pricing.per_image],
    ["输入 / 百万 tokens", model.pricing.input_per_million_tokens],
    ["输出 / 百万 tokens", model.pricing.output_per_million_tokens],
  ].filter((entry): entry is [string, string] => !!entry[1]);
  return values.length
    ? {
        state: "known" as const,
        currency: model.pricing.currency,
        summary: values
          .map(([label, value]) => `${label} ${model.pricing.currency} ${value}`)
          .join(" · "),
      }
    : {
        state: "unknown" as const,
        currency: model.pricing.currency,
        summary: "费用未知；不是免费或零费用",
      };
}

function providerModelState(
  model: RegistryModelProfile,
  provider: ProviderProfile | undefined,
): { state: PreparationState; reasons: string[] } {
  const reasons: string[] = [];
  if (!model.enabled || model.status === "disabled")
    reasons.push("Model Profile 已禁用");
  if (!provider) reasons.push("Provider 不存在");
  else {
    if (!provider.enabled) reasons.push("Provider 已禁用");
    if (!provider.credential_configured) reasons.push("Provider 缺少凭证");
    if (provider.adapter === "mock") reasons.push("测试 Provider 不能用于真实任务");
    if (["unreachable", "invalid_credential", "rate_limited", "incompatible_protocol", "disabled"].includes(provider.health.status))
      reasons.push(`Provider 状态：${provider.health.status}`);
  }
  if (model.status === "unavailable") reasons.push("模型已记录为不可用");
  if (reasons.length) return { state: "blocked", reasons };
  if (
    model.status === "unknown" ||
    model.status === "unverified" ||
    provider?.health.status === "unknown"
  )
    return {
      state: "uncertain",
      reasons: ["能力或连接尚未主动验证；不自动发起收费探测"],
    };
  return { state: "ready", reasons: [] };
}

function pluginState(enabled: boolean, status: string, hasReadyInstance: boolean) {
  if (!enabled || ["disabled", "unsupported_platform", "incompatible_api", "invalid_manifest", "invalid_contract", "crashed"].includes(status))
    return { state: "blocked" as const, reasons: [!enabled ? "Plugin 已禁用" : `Plugin 状态：${status}`] };
  if (!hasReadyInstance)
    return {
      state: "setup_required" as const,
      reasons: ["Plugin 不是模型；仍需兼容 Model Bundle 和 Ready Model Instance"],
    };
  if (status !== "ready")
    return {
      state: "setup_required" as const,
      reasons: [`Plugin 尚未 Ready：${status}`],
    };
  return { state: "ready" as const, reasons: [] };
}

function instanceState(
  profile: ModelInstanceProfile,
  instance: InstalledModelInstance | undefined,
  plugin: ExpertPluginRegistry["installations"][number] | undefined,
  bundle: InstalledModelBundle | undefined,
) {
  const reasons: string[] = [];
  if (!instance) reasons.push("Model Instance 不存在");
  else {
    if (instance.status !== "ready") reasons.push(`Model Instance 状态：${instance.status}`);
    if (!instance.contract_inspection.valid) reasons.push("能力契约检查未通过");
    if (instance.smoke_test_result?.status !== "passed") reasons.push("模型实例尚无通过的冒烟测试");
  }
  if (!profile.selectable || profile.availability !== "available")
    reasons.push(`Model Instance Profile 状态：${profile.availability}`);
  if (!plugin?.enabled || plugin.status !== "ready" || plugin?.manifest.id !== instance?.plugin_id || plugin?.manifest.version !== instance?.plugin_version || plugin?.package_sha256 !== instance?.plugin_package_sha256)
    reasons.push("Plugin 未 Ready 或摘要/版本不匹配");
  if (!bundle?.enabled || !bundle.manifest.publishable || bundle.manifest.fixture || bundle.bundle_sha256 !== instance?.model_bundle_sha256)
    reasons.push("Model Bundle 未启用、不可发布、为 Fixture 或摘要不匹配");
  return reasons.length
    ? { state: "setup_required" as const, reasons }
    : { state: "ready" as const, reasons: [] };
}

function assertContext(context: SetupContext) {
  if (!context.id || !context.project_id || !context.conversation_id || !context.task_id)
    throw new Error("模型准备请求缺少 Project、Conversation 或 Task 身份");
  if (!context.requirements.length)
    throw new Error("模型准备请求没有具体 capability");
  if (new Set(context.requirements.map((item) => item.id)).size !== context.requirements.length)
    throw new Error("模型准备请求包含重复 requirement ID");
  if (new Set(context.allowed_models.map((item) => item.model_id)).size !== context.allowed_models.length)
    throw new Error("allowed_models 含重复模型，未继续");
  if (context.allowed_models.some((item) => !item.model_id || !item.binding_digest))
    throw new Error("allowed_models 缺少模型或绑定摘要");
  const targets = new Set<SetupTarget>(["agent_model", "provider_model", "plugin", "model_instance"]);
  const capabilities = new Set<ModelCapability>(["text_generation", "vision_language", "image_classification", "object_detection", "open_vocabulary_detection", "phrase_grounding", "semantic_segmentation", "prompted_segmentation", "instance_segmentation", "keypoint_detection"]);
  const modalities = new Set<InputModality>(["text", "image", "video"]);
  if (context.requirements.some((item) => !item.id || !item.purpose || !targets.has(item.target) || !capabilities.has(item.capability) || !item.input_modalities.length || item.input_modalities.some((input) => !modalities.has(input))))
    throw new Error("模型准备 requirement 含未知 target、capability 或输入类型");
  const target = new URL(context.return_to, "http://annotagent.local");
  const expected = `/projects/${encodeURIComponent(context.project_id)}/work`;
  if (target.origin !== "http://annotagent.local" || target.pathname !== expected || target.searchParams.get("task") !== context.task_id)
    throw new Error("return_to 必须返回同一 Project 和 Task 的站内工作区");
}

export function createModelPreparationService(
  transport: ModelPreparationTransport = request,
): ModelPreparationService {
  const inspect = async (
    context: SetupContext,
    signal: AbortSignal,
  ): Promise<PreparationSnapshot> => {
    assertContext(context);
    const project = esc(context.project_id);
    const conversation = esc(context.conversation_id);
    const task = esc(context.task_id);
    const profileRequirements = context.requirements.filter((item) =>
      ["agent_model", "provider_model"].includes(item.target),
    );
    const compatibilityReads = Promise.all(
      profileRequirements.map(async (requirement) => {
        const query = new URLSearchParams({
          capabilities: requirement.capability,
          input_modalities: requirement.input_modalities.join(","),
          allow_unverified: "true",
          ...(requirement.tool_calls ? { tool_calls: "true" } : {}),
          ...(requirement.structured_output ? { structured_output: "true" } : {}),
          ...(requirement.json_schema ? { json_schema: "true" } : {}),
        });
        const value = await transport<{ models: RegistryModelProfile[] }>(
          `/api/model-profiles/compatible?${query}`,
          { signal },
        );
        return [requirement.id, new Set(value.models.map((model) => model.id))] as const;
      }),
    );
    const [workspace, profiles, compatibleRows, providers, plugins, instanceResult, bundles, catalog, bindings, preference, draft] = await Promise.all([
      transport<TaskWorkspace>(`/api/projects/${project}/conversations/${conversation}/tasks/${task}/workspace`, { signal }),
      transport<{ models: RegistryModelProfile[] }>("/api/model-profiles", { signal }),
      compatibilityReads,
      transport<{ providers: ProviderProfile[] }>("/api/providers", { signal }),
      transport<ExpertPluginRegistry>("/api/plugins", { signal }),
      transport<{ instances: InstalledModelInstance[]; model_profiles: ModelInstanceProfile[] }>("/api/model-instances", { signal }),
      transport<{ bundles: InstalledModelBundle[] }>("/api/model-bundles", { signal }),
      transport<{ bundles: ModelCatalogEntry[] }>("/api/model-bundles/available", { signal }),
      transport<{ project_id: string; bindings: ProjectModelBinding[] }>(`/api/projects/${project}/model-bindings`, { signal }),
      transport<{ revision: number; model_profile_id: string | null }>(`/api/projects/${project}/conversations/${conversation}/agent-model`, { signal }),
      context.draft_id
        ? transport<WorkflowDraft>(`/api/workflow-drafts/${esc(context.draft_id)}?project_id=${project}`, { signal })
        : Promise.resolve(undefined),
    ]);
    if (
      workspace.project_id !== context.project_id ||
      workspace.conversation_id !== context.conversation_id ||
      workspace.task.input.id !== context.task_id ||
      bindings.project_id !== context.project_id ||
      (draft && (draft.id !== context.draft_id || draft.project_id !== context.project_id))
    )
      throw new Error("模型准备读取到其他 Project、Conversation、Task 或 Draft，未展示候选");
    const compatible = new Map(compatibleRows);
    const contextChanges: string[] = [];
    if (workspace.task.input.schema_revision !== context.task_revision)
      contextChanges.push("Task schema revision 已变化");
    if (draft && (draft.revision !== context.draft_revision || draft.content_hash !== context.draft_content_hash))
      contextChanges.push("Draft revision 或内容摘要已变化");

    const providerModels: ProviderModelCandidate[] = profiles.models
      .flatMap((model): ProviderModelCandidate[] => {
        const matches = context.requirements.filter((requirement) =>
          ["agent_model", "provider_model"].includes(requirement.target) &&
          requirementMatchesModel(requirement, model),
        );
        if (!matches.length) return [];
        const provider = providers.providers.find((item) => item.id === model.provider_id);
        return (["agent_model", "provider_model"] as const).flatMap((kind) => {
          const roleMatches = matches.filter((item) => item.target === kind);
          if (!roleMatches.length) return [];
          const availability = providerModelState(model, provider);
          const omittedByServer = roleMatches.some(
            (requirement) => compatible.get(requirement.id)?.has(model.id) !== true,
          );
          const serverReasons = omittedByServer
            ? [availability.state === "uncertain"
                ? "服务器尚未把此 Profile 判定为可用；unknown 不等于必然失败"
                : "服务器兼容判定未通过"]
            : [];
          const state = omittedByServer && availability.state === "ready"
            ? "blocked" as const
            : availability.state;
          return [{
            kind,
            id: model.id,
            revision: model.revision,
            display_name: model.display_name,
            remote_model_id: model.remote_model_id,
            provider_id: model.provider_id,
            provider_name: provider?.display_name ?? "Provider 不存在",
            requirement_ids: roleMatches.map((item) => item.id),
            state,
            reasons: [...availability.reasons, ...serverReasons],
            capability_source: model.capability_source,
            cost: modelCost(model),
          }];
        });
      });

    const modelInstances: ModelInstanceCandidate[] = instanceResult.model_profiles
      .flatMap((profile): ModelInstanceCandidate[] => {
        const matches = context.requirements.filter((requirement) =>
          requirement.target === "model_instance" &&
          requirementMatchesCapabilities(requirement, profile.capabilities),
        );
        if (!matches.length) return [];
        const instance = instanceResult.instances.find((item) => item.id === profile.model_instance_id);
        const plugin = plugins.installations.find((item) => item.manifest.id === instance?.plugin_id && item.manifest.version === instance?.plugin_version);
        const bundle = bundles.bundles.find((item) => item.manifest.id === instance?.model_bundle_id && item.manifest.version === instance?.model_bundle_version);
        const availability = instanceState(profile, instance, plugin, bundle);
        return [{
          kind: "model_instance" as const,
          id: profile.model_instance_id,
          display_name: profile.display_name,
          plugin_id: instance?.plugin_id ?? "",
          plugin_version: instance?.plugin_version ?? "",
          bundle_id: instance?.model_bundle_id ?? "",
          bundle_version: instance?.model_bundle_version ?? "",
          model_profile_id: profile.model_profile_id,
          model_profile_revision: profile.model_profile_revision,
          requirement_ids: matches.map((item) => item.id),
          state: availability.state,
          reasons: availability.reasons,
        }];
      });

    const pluginCandidates: PluginCandidate[] = plugins.installations
      .flatMap((plugin): PluginCandidate[] => {
        const capabilities = plugin.manifest.models.flatMap((model) => model.capabilities);
        const matches = context.requirements.filter((requirement) =>
          requirement.target === "plugin" &&
          requirementMatchesCapabilities(requirement, capabilities),
        );
        if (!matches.length) return [];
        const ready = instanceResult.model_profiles.some((profile) => {
          if (!matches.some((requirement) => requirementMatchesCapabilities(requirement, profile.capabilities))) return false;
          const instance = instanceResult.instances.find((item) => item.id === profile.model_instance_id);
          if (instance?.plugin_id !== plugin.manifest.id || instance.plugin_version !== plugin.manifest.version) return false;
          const bundle = bundles.bundles.find((item) => item.manifest.id === instance.model_bundle_id && item.manifest.version === instance.model_bundle_version);
          return instanceState(profile, instance, plugin, bundle).state === "ready";
        });
        const availability = pluginState(plugin.enabled, plugin.status, ready);
        return [{
          kind: "plugin" as const,
          id: plugin.manifest.id,
          version: plugin.manifest.version,
          display_name: plugin.manifest.display_name,
          requirement_ids: matches.map((item) => item.id),
          state: availability.state,
          reasons: availability.reasons,
        }];
      });

    const bundleCandidates: BundleCandidate[] = catalog.bundles
      .map((bundle) => {
        const matches = context.requirements.filter((requirement) =>
          ["plugin", "model_instance"].includes(requirement.target) &&
          requirementMatchesCapabilities(requirement, bundle.capabilities),
        );
        if (!matches.length) return undefined;
        const installed = bundles.bundles.find((item) => item.manifest.id === bundle.bundle_id && item.manifest.version === bundle.bundle_version);
        const reasons: string[] = [];
        if (bundle.fixture || !bundle.publishable) reasons.push("此目录项不能用于真实发布");
        return {
          kind: "model_bundle" as const,
          id: bundle.bundle_id,
          version: bundle.bundle_version,
          display_name: bundle.display_name,
          requirement_ids: matches.map((item) => item.id),
          compatible_plugins: bundle.compatible_plugins.map((item) => ({ plugin_id: item.plugin_id, plugin_version: item.plugin_version })),
          state: reasons.length ? "blocked" as const : installed?.enabled ? "installed" as const : "available" as const,
          reasons,
        };
      })
      .filter((item): item is BundleCandidate => !!item);

    const allCandidates = [...providerModels, ...pluginCandidates, ...modelInstances];
    return {
      context,
      guard: {
        task_revision: workspace.task.input.schema_revision,
        draft_revision: draft?.revision,
        draft_content_hash: draft?.content_hash,
        agent_model_revision: preference.revision,
        project_bindings: bindings.bindings,
        authorization_fingerprint: context.authorization_fingerprint,
        allowed_models: context.allowed_models,
      },
      context_changes: contextChanges,
      provider_models: providerModels,
      plugins: pluginCandidates,
      model_instances: modelInstances,
      model_bundles: bundleCandidates,
      requirements: context.requirements.map((requirement) => {
        const candidates = allCandidates.filter((candidate) => candidate.requirement_ids.includes(requirement.id));
        return {
          requirement,
          ready_candidate_ids: candidates.filter((candidate) => candidate.state === "ready").map((candidate) => `${candidate.kind}:${candidate.id}`),
          uncertain_candidate_ids: candidates.filter((candidate) => candidate.state === "uncertain").map((candidate) => `${candidate.kind}:${candidate.id}`),
        };
      }),
      authorization: "recheck_required",
    };
  };

  return {
    inspect,
    async recheck(previous, signal) {
      const snapshot = await inspect(previous.context, signal);
      const changed: string[] = [];
      changed.push(...snapshot.context_changes);
      if (snapshot.guard.task_revision !== previous.guard.task_revision)
        changed.push("Task schema revision 已变化");
      if (snapshot.guard.draft_revision !== previous.guard.draft_revision || snapshot.guard.draft_content_hash !== previous.guard.draft_content_hash)
        changed.push("Draft revision 或内容摘要已变化");
      if (snapshot.guard.agent_model_revision !== previous.guard.agent_model_revision)
        changed.push("Agent 模型偏好 revision 已变化");
      if (!same(snapshot.guard.project_bindings, previous.guard.project_bindings))
        changed.push("Project 模型绑定已变化");
      if (!same(snapshot.context.allowed_models, previous.context.allowed_models))
        throw new Error("设置回流不得扩大 allowed_models");
      const uniqueChanges = [...new Set(changed)];
      return {
        snapshot,
        changed: uniqueChanges,
        capability_prepared: uniqueChanges.length === 0 && snapshot.requirements.every((item) => item.ready_candidate_ids.length > 0),
        can_resume_without_authorization: false,
        task_reused: true,
        draft_reused: true,
        allowed_models_expanded: false,
        authorization: "recheck_required",
      };
    },
  };
}

const setupPrefix = "annotagent.setup-request.v1:";

export function preserveSetupContext(storage: Storage, context: SetupContext) {
  assertContext(context);
  storage.setItem(`${setupPrefix}${context.id}`, JSON.stringify(context));
  return context.id;
}

export function restoreSetupContext(storage: Storage, id: string) {
  const raw = storage.getItem(`${setupPrefix}${id}`);
  if (!raw) throw new Error("模型准备上下文不存在；未打开其他 Task");
  const context = JSON.parse(raw) as SetupContext;
  if (context.id !== id) throw new Error("模型准备上下文 ID 不匹配");
  assertContext(context);
  return context;
}

export function clearSetupContext(storage: Storage, id: string) {
  storage.removeItem(`${setupPrefix}${id}`);
}

export function setupSettingsPath(context: SetupContext, target: SetupTarget) {
  assertContext(context);
  const page = target === "agent_model" ? "agent-models" : target === "provider_model" ? "vision-models" : target === "plugin" ? "plugins" : "vision-models";
  return `/settings/${page}?setup_request=${encodeURIComponent(context.id)}`;
}

export function setupReturnPath(
  context: SetupContext,
  outcome: "configured" | "cancelled",
) {
  assertContext(context);
  const target = new URL(context.return_to, "http://annotagent.local");
  target.searchParams.set("setup_request", context.id);
  target.searchParams.set("setup_outcome", outcome);
  return `${target.pathname}${target.search}${target.hash}`;
}
