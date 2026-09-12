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

/** Structural mirror of Frontend 1's G0 seam. Keep this module independent of App/Adapter. */
export type MainlineCapabilitySetupRequest = {
  id: string;
  project_id: string;
  task_id: string;
  task_revision: string;
  registry_revision: string;
  role: string;
  required_capabilities: string[];
  compatible_model_ids: string[];
  status: "required" | "ready" | "cancelled" | "stale";
  return_path: string;
};

/** Identity and authorization fields not carried by the G0 seam; never infer these from the URL. */
export type SetupContinuationScope = {
  conversation_id: string;
  draft_id?: string;
  draft_revision?: number;
  draft_content_hash?: string;
  authorization_fingerprint?: string;
  allowed_models: FrozenAllowedModel[];
  requirements: SetupRequirement[];
  created_at: string;
};

export type SetupContext = {
  id: string;
  project_id: string;
  conversation_id: string;
  task_id: string;
  task_revision: string;
  registry_revision: string;
  role: string;
  compatible_model_ids: string[];
  setup_status: MainlineCapabilitySetupRequest["status"];
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
  registry_revision: string;
  compatible_model_ids: string[];
  draft_revision?: number;
  draft_content_hash?: string;
  agent_model_revision: number;
  project_bindings: ProjectModelBinding[];
  authorization_fingerprint?: string;
  allowed_models: FrozenAllowedModel[];
};

export type CapabilityReadinessCandidate = {
  id: string;
  candidate_type: "model_profile" | "plugin_model" | "model_instance";
  model_profile_id?: string;
  model_instance_id?: string;
  revision: number;
  digest: string;
  roles: string[];
  capabilities: ModelCapability[];
  quality_contracts: unknown[];
  readiness: "ready" | "unknown" | "unavailable" | "disabled";
  production_eligible: boolean;
  test_fixture: boolean;
  blocker?: { code: string; message: string } | null;
  project_bindings: ProjectModelBinding[];
  allowed_by_current_scope: boolean;
  selected_for_next_agent_request?: boolean;
  setup: { kind: "model_profile" | "plugin" | "model_instance"; api_url: string };
};

/** Passive server-composed B4 read model. This is the authority for readiness and scope. */
export type CapabilityReadiness = {
  contract_version: "mainline-capability-v1";
  project_id: string;
  project_owner_id: string;
  conversation_id: string;
  task_id: string;
  task_schema_revision: string;
  draft?: { id: string; revision: number; content_hash: string; status: string } | null;
  registry_revision: string;
  registry_revision_kind: "snapshot_sha256";
  candidates: CapabilityReadinessCandidate[];
  agent_model_preference: { revision: number; model_profile_id: string | null };
  authorization: {
    source: string | null;
    consent_id: string | null;
    expires_at: string | null;
    permission_digest: string | null;
    allowed_models: FrozenAllowedModel[];
    active: boolean;
    can_resume_without_authorization: false;
  };
  budget: unknown;
  task_cost: {
    scope: "conversation_task_model_calls";
    receipt_count: number;
    known: boolean;
    amount: string | null;
    currency: string | null;
    reason: string | null;
  };
  passive: true;
  setup_recheck_only: true;
  auto_expands_allowed_models: false;
  consistency: "server_composed_versioned_snapshot";
};

export type PreparationSnapshot = {
  context: SetupContext;
  readiness: CapabilityReadiness;
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

function sortedUnique(values: string[]) {
  return [...new Set(values)].sort((left, right) => left.localeCompare(right));
}

export function setupContextFromCapabilityRequest(
  request: MainlineCapabilitySetupRequest,
  continuation: SetupContinuationScope,
): SetupContext {
  const required = sortedUnique(request.required_capabilities);
  const supplied = sortedUnique(continuation.requirements.map((item) => item.capability));
  if (!same(required, supplied))
    throw new Error("Capability Setup 的 requirements 与服务器请求不一致");
  if (request.compatible_model_ids.length !== new Set(request.compatible_model_ids).size)
    throw new Error("Capability Setup 含重复兼容模型 ID");
  const context: SetupContext = {
    id: request.id,
    project_id: request.project_id,
    conversation_id: continuation.conversation_id,
    task_id: request.task_id,
    task_revision: request.task_revision,
    registry_revision: request.registry_revision,
    role: request.role,
    compatible_model_ids: [...request.compatible_model_ids],
    setup_status: request.status,
    draft_id: continuation.draft_id,
    draft_revision: continuation.draft_revision,
    draft_content_hash: continuation.draft_content_hash,
    authorization_fingerprint: continuation.authorization_fingerprint,
    allowed_models: continuation.allowed_models,
    return_to: request.return_path,
    requirements: continuation.requirements,
    created_at: continuation.created_at,
  };
  assertContext(context);
  return context;
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

function readinessMatchesRequirement(
  candidate: CapabilityReadinessCandidate,
  requirement: SetupRequirement,
) {
  if (!candidate.capabilities.includes(requirement.capability)) return false;
  if (requirement.target === "agent_model")
    return candidate.candidate_type === "model_profile" && candidate.roles.includes("agent");
  if (requirement.target === "provider_model")
    return candidate.candidate_type === "model_profile";
  if (requirement.target === "plugin")
    return candidate.candidate_type === "plugin_model";
  return candidate.candidate_type === "model_instance";
}

function authoritativeCandidateState(candidate: CapabilityReadinessCandidate) {
  const blocker = candidate.blocker?.message ? [candidate.blocker.message] : [];
  if (candidate.test_fixture)
    return { state: "blocked" as const, reasons: ["TEST/Mock 候选不能用于真实任务", ...blocker] };
  if (candidate.readiness === "unknown")
    return { state: "uncertain" as const, reasons: blocker.length ? blocker : ["服务器尚无当前可用性证据；unknown 不等于必然失败"] };
  if (candidate.readiness !== "ready" || !candidate.production_eligible)
    return { state: "blocked" as const, reasons: blocker.length ? blocker : ["候选当前不可用于生产任务"] };
  return { state: "ready" as const, reasons: [] };
}

export function setupContextFromReadiness(
  request: MainlineCapabilitySetupRequest,
  readiness: CapabilityReadiness,
  requirements: SetupRequirement[],
  created_at: string,
) {
  if (
    readiness.contract_version !== "mainline-capability-v1" ||
    readiness.project_id !== request.project_id ||
    readiness.task_id !== request.task_id ||
    readiness.task_schema_revision !== request.task_revision ||
    readiness.registry_revision !== request.registry_revision
  )
    throw new Error("Capability Setup 与服务器 readiness 身份或 revision 不一致");
  const compatible = sortedUnique(readiness.candidates
    .filter((candidate) => requirements.some((requirement) => readinessMatchesRequirement(candidate, requirement)))
    .filter((candidate) => candidate.production_eligible || (candidate.readiness === "unknown" && !candidate.test_fixture))
    .map((candidate) => candidate.id));
  if (!same(compatible, sortedUnique(request.compatible_model_ids)))
    throw new Error("Capability Setup 的兼容模型集合已经变化");
  return setupContextFromCapabilityRequest(request, {
    conversation_id: readiness.conversation_id,
    draft_id: readiness.draft?.id,
    draft_revision: readiness.draft?.revision,
    draft_content_hash: readiness.draft?.content_hash,
    authorization_fingerprint: readiness.authorization.permission_digest ?? undefined,
    allowed_models: readiness.authorization.allowed_models,
    requirements,
    created_at,
  });
}

function assertContext(context: SetupContext) {
  if (!context.id || !context.project_id || !context.conversation_id || !context.task_id)
    throw new Error("模型准备请求缺少 Project、Conversation 或 Task 身份");
  if (!context.requirements.length)
    throw new Error("模型准备请求没有具体 capability");
  if (!context.registry_revision || !context.role)
    throw new Error("模型准备请求缺少 Registry revision 或模型角色");
  if (!(["required", "ready", "cancelled", "stale"] as string[]).includes(context.setup_status))
    throw new Error("模型准备请求状态无效");
  if (context.compatible_model_ids.length !== new Set(context.compatible_model_ids).size)
    throw new Error("模型准备请求包含重复兼容模型 ID");
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
    const [readiness, workspace, profiles, compatibleRows, providers, plugins, instanceResult, bundles, catalog, bindings, draft] = await Promise.all([
      transport<CapabilityReadiness>(`/api/projects/${project}/conversations/${conversation}/tasks/${task}/capability-readiness`, { signal }),
      transport<TaskWorkspace>(`/api/projects/${project}/conversations/${conversation}/tasks/${task}/workspace`, { signal }),
      transport<{ models: RegistryModelProfile[] }>("/api/model-profiles", { signal }),
      compatibilityReads,
      transport<{ providers: ProviderProfile[] }>("/api/providers", { signal }),
      transport<ExpertPluginRegistry>("/api/plugins", { signal }),
      transport<{ instances: InstalledModelInstance[]; model_profiles: ModelInstanceProfile[] }>("/api/model-instances", { signal }),
      transport<{ bundles: InstalledModelBundle[] }>("/api/model-bundles", { signal }),
      transport<{ bundles: ModelCatalogEntry[] }>("/api/model-bundles/available", { signal }),
      transport<{ project_id: string; bindings: ProjectModelBinding[] }>(`/api/projects/${project}/model-bindings`, { signal }),
      context.draft_id
        ? transport<WorkflowDraft>(`/api/workflow-drafts/${esc(context.draft_id)}?project_id=${project}`, { signal })
        : Promise.resolve(undefined),
    ]);
    if (
      readiness.contract_version !== "mainline-capability-v1" ||
      readiness.passive !== true ||
      readiness.setup_recheck_only !== true ||
      readiness.auto_expands_allowed_models !== false ||
      readiness.consistency !== "server_composed_versioned_snapshot" ||
      readiness.project_id !== context.project_id ||
      readiness.conversation_id !== context.conversation_id ||
      readiness.task_id !== context.task_id ||
      workspace.project_id !== context.project_id ||
      workspace.conversation_id !== context.conversation_id ||
      workspace.task.input.id !== context.task_id ||
      bindings.project_id !== context.project_id ||
      (draft && (draft.id !== context.draft_id || draft.project_id !== context.project_id))
    )
      throw new Error("模型准备读取到其他 Project、Conversation、Task 或 Draft，未展示候选");
    const compatible = new Map(compatibleRows);
    const contextChanges: string[] = [];
    if (readiness.task_schema_revision !== context.task_revision)
      contextChanges.push("Task schema revision 已变化");
    if ((readiness.draft?.id ?? undefined) !== context.draft_id || (readiness.draft?.revision ?? undefined) !== context.draft_revision || (readiness.draft?.content_hash ?? undefined) !== context.draft_content_hash)
      contextChanges.push("Draft revision 或内容摘要已变化");
    if (readiness.registry_revision !== context.registry_revision)
      contextChanges.push("Registry revision 已变化");
    if ((readiness.authorization.permission_digest ?? undefined) !== context.authorization_fingerprint)
      contextChanges.push("任务模型授权已变化");
    if (!same(readiness.authorization.allowed_models, context.allowed_models))
      contextChanges.push("任务 allowed_models 已变化");

    const providerModels: ProviderModelCandidate[] = profiles.models
      .flatMap((model): ProviderModelCandidate[] => {
        const matches = context.requirements.filter((requirement) =>
          ["agent_model", "provider_model"].includes(requirement.target) &&
          requirementMatchesModel(requirement, model),
        );
        if (!matches.length) return [];
        const authority = readiness.candidates.find((candidate) =>
          candidate.candidate_type === "model_profile" && candidate.model_profile_id === model.id,
        );
        if (!authority) return [];
        const provider = providers.providers.find((item) => item.id === model.provider_id);
        return (["agent_model", "provider_model"] as const).flatMap((kind) => {
          const roleMatches = matches.filter((item) => item.target === kind && readinessMatchesRequirement(authority, item));
          if (!roleMatches.length) return [];
          const availability = authoritativeCandidateState(authority);
          const omittedByServer = roleMatches.some(
            (requirement) => compatible.get(requirement.id)?.has(model.id) !== true,
          );
          const serverReasons = omittedByServer
            ? [authority.readiness === "unknown"
                ? "主动兼容清单尚未把此 Profile 判定为 Ready；保留被动 read model 的 unknown 状态"
                : "主动兼容清单未包含此 Profile"]
            : [];
          return [{
            kind,
            id: model.id,
            revision: model.revision,
            display_name: model.display_name,
            remote_model_id: model.remote_model_id,
            provider_id: model.provider_id,
            provider_name: provider?.display_name ?? "Provider 不存在",
            requirement_ids: roleMatches.map((item) => item.id),
            state: availability.state,
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
        const authority = readiness.candidates.find((candidate) => candidate.candidate_type === "model_instance" && candidate.id === profile.selection_id && candidate.model_instance_id === profile.model_instance_id);
        if (!authority) return [];
        const availability = authoritativeCandidateState(authority);
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
        const authorities = plugins.models
          .filter((model) => model.reference.plugin_id === plugin.manifest.id && model.reference.plugin_version === plugin.manifest.version)
          .map((model) => readiness.candidates.find((candidate) => candidate.candidate_type === "plugin_model" && candidate.id === model.selection_id))
          .filter((candidate): candidate is CapabilityReadinessCandidate => !!candidate && matches.some((requirement) => readinessMatchesRequirement(candidate, requirement)));
        const states = authorities.map(authoritativeCandidateState);
        const availability = states.some((item) => item.state === "ready")
          ? { state: "ready" as const, reasons: [] }
          : states.some((item) => item.state === "uncertain")
            ? { state: "uncertain" as const, reasons: states.flatMap((item) => item.reasons) }
            : states.length
              ? { state: "blocked" as const, reasons: states.flatMap((item) => item.reasons) }
              : { state: "setup_required" as const, reasons: ["Plugin 尚无匹配的服务器能力候选"] };
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
    const liveCompatibleIds = sortedUnique(readiness.candidates
      .filter((candidate) => context.requirements.some((requirement) => readinessMatchesRequirement(candidate, requirement)))
      .filter((candidate) => candidate.production_eligible || (candidate.readiness === "unknown" && !candidate.test_fixture))
      .map((candidate) => candidate.id));
    if (!same(liveCompatibleIds, sortedUnique(context.compatible_model_ids)))
      contextChanges.push("兼容模型集合已变化");
    return {
      context,
      readiness,
      guard: {
        task_revision: readiness.task_schema_revision,
        registry_revision: readiness.registry_revision,
        compatible_model_ids: liveCompatibleIds,
        draft_revision: readiness.draft?.revision,
        draft_content_hash: readiness.draft?.content_hash,
        agent_model_revision: readiness.agent_model_preference.revision,
        project_bindings: readiness.candidates.flatMap((candidate) => candidate.project_bindings),
        authorization_fingerprint: readiness.authorization.permission_digest ?? undefined,
        allowed_models: readiness.authorization.allowed_models,
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
      if (snapshot.guard.registry_revision !== previous.guard.registry_revision)
        changed.push("Registry revision 已变化");
      if (!same(snapshot.guard.compatible_model_ids, previous.guard.compatible_model_ids))
        changed.push("兼容模型集合已变化");
      if (snapshot.guard.draft_revision !== previous.guard.draft_revision || snapshot.guard.draft_content_hash !== previous.guard.draft_content_hash)
        changed.push("Draft revision 或内容摘要已变化");
      if (snapshot.guard.agent_model_revision !== previous.guard.agent_model_revision)
        changed.push("Agent 模型偏好 revision 已变化");
      if (!same(snapshot.guard.project_bindings, previous.guard.project_bindings))
        changed.push("Project 模型绑定已变化");
      if (snapshot.guard.authorization_fingerprint !== previous.guard.authorization_fingerprint || !same(snapshot.guard.allowed_models, previous.guard.allowed_models))
        changed.push("任务模型授权已变化");
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
