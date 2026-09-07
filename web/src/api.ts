export type SampleOperation = {
  id: string; project_id: string; draft_id: string;
  status: "queued" | "running" | "cancelling" | "cancelled" | "interrupted" | "failed" | "succeeded";
  error?: string | null;
};
export type ConversationSampleConsent = { conversation_id: string; task_id: string; previous_grant_id: string; scope_hash: string; expires_at: string; allow_unknown_cost: boolean };
export type ConversationSamplePreview = { project_id: string; revision: number; image_count: number; models: {name: string; destination: string; id: string; revision: number}[]; supported: boolean; other_bindings: string[]; authorization_fingerprint: string; request_limit: number; estimated_cost: null; request_id: string; conversation_budget: { previous_grant_id: string; scope_hash: string; expires_at: string; maximum_calls: number; used_calls: number } };
export type ProcessingSelection = { draft_id: string; sample_test_id: string; limit?: number };
export type ProcessingAuthorization = { revision: number; authorization_fingerprint: string; image_count: number; available_images: number; maximum_model_calls: number; sample_feedback_count: number; plan_name: string; goal: { goal?: string }; models: { model_profile_id: string; remote_model_id: string; provider_base_url: string }[]; native_models?: { id: string; name: string; destination: string; revision: number }[] };
export type ProcessingReceipt = { id: string; project_id: string; draft_id: string; phase: string; batch_id?: string; error?: string | null; authorization: ProcessingAuthorization; request?: ConfirmProcessingRequest };
export type ConfirmProcessingRequest = { request_id: string; selection: ProcessingSelection; expected_revision: number; authorization_fingerprint: string };

import type {
  Annotation,
  AnnotationRevision,
  AgentSession,
  CorrectionMemoryRecord,
  DashboardData,
  DatasetBatchSummary,
  DetectionWorkerTestResult,
  DetectionWorkerSampleTestResult,
  CredentialSource,
  HistoryRun,
  ImageItem,
  ModelBinding,
  PipelineBuilderConstraints,
  PipelineBuildMode,
  PipelineDraftDiff,
  ExportReadiness,
  ProjectExportResult,
  ProjectWorkflow,
  ProjectSummary,
  ProjectWorkspaceSummary,
  NodeReplayReport,
  RunNodeArtifactInspection,
  RunResultSummary,
  RunDebugSummary,
  RunAnnotationInspection,
  ReviewDecisionOutcome,
  ReviewItem,
  ReviewNavigation,
  ReviewQueueProgress,
  RunEvent,
  SkillDetail,
  WorkflowDraft,
  WorkflowDraftApplyReport,
  WorkflowCatalog,
  WorkflowDryRunReport,
  WorkflowSampleTestRecord,
  WorkflowVersionComparison,
  WorkflowSuggestion,
  ProviderPresetProfile,
  ProviderProfile,
  RegistryModelProfile,
  InputModality,
  ModelCapability,
  ProviderProbeUsage,
  ProjectModelBinding,
  GlobalModelDefaults,
  ModelBindingRole,
  LegacyRegistryImportPreview,
  LegacyRegistryImportReport,
  GeometryCalibrationReport,
  GeometryCalibrationView,
  GeometryQualitySummary,
  ModelCapabilityQualityContract,
  PipelineImprovementSession,
  ProjectGeometryPolicy,
  ExpertPluginRegistry,
  VerifiedExpertPluginPackage,
  InstalledModelBundle,
  InstalledModelInstance,
  ModelCatalogEntry,
  ModelInstallOperation,
  ModelInstanceProfile,
  PageMetadata,
  ManagementObjectKind,
  ManagementPreview,
  ManagementReceipt,
  ManagementRequest,
  ManagementUsageSummary,
  PipelineLifecycleSummary,
  RunProvenanceSummary,
  TrashEntry,
  VerifiedModelBundlePackage,
} from "./types";

interface LocalApiSession {
  csrf_token: string;
}

let localSession: Promise<LocalApiSession> | undefined;

export class ApiRequestError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly code?: string,
    readonly suggestedAction?: string,
  ) {
    super(message);
    this.name = "ApiRequestError";
  }
}

export function resetLocalApiSessionForTests() {
  localSession = undefined;
}

async function initializeLocalSession(force = false): Promise<LocalApiSession> {
  if (force) localSession = undefined;
  localSession ??= fetch("/api/session", {
    credentials: "same-origin",
    headers: { accept: "application/json" },
  }).then(async (response) => {
    if (!response.ok) throw new Error(`Unable to initialize local session (${response.status}).`);
    const value = await response.json() as Partial<LocalApiSession>;
    if (!value.csrf_token) throw new Error("Local session did not provide a CSRF token.");
    return value as LocalApiSession;
  }).catch((error) => {
    localSession = undefined;
    throw error;
  });
  return localSession;
}

function isMutation(init?: RequestInit) {
  return ["POST", "PUT", "PATCH", "DELETE"].includes((init?.method ?? "GET").toUpperCase());
}

function apiPath(path: string) {
  return path.split("?", 1)[0];
}

function privilegedAction(path: string, init?: RequestInit): string | undefined {
  const method = (init?.method ?? "GET").toUpperCase();
  const cleanPath = apiPath(path);
  const privileged = method === "DELETE"
    || cleanPath === "/api/settings"
    || cleanPath.includes("/credential")
    || cleanPath.endsWith("/active-probe")
    || cleanPath === "/api/model-bundles/install"
    || cleanPath === "/api/model-bundles/import"
    || cleanPath === "/api/model-bundles/gc"
    || cleanPath === "/api/model-installations"
    || cleanPath.endsWith("/management/actions")
    || (cleanPath.startsWith("/api/model-bundles/") && ["/verify", "/test", "/enable", "/disable", "/license-acceptance"].some((suffix) => cleanPath.endsWith(suffix)))
    || (cleanPath.startsWith("/api/model-instances/") && cleanPath.endsWith("/test"))
    || cleanPath === "/api/plugins/packages/install"
    || (cleanPath.startsWith("/api/plugins/") && ["/test", "/enable", "/disable", "/weights", "/legacy-model-bundle"].some((suffix) => cleanPath.endsWith(suffix)));
  return privileged ? `${method} ${cleanPath}` : undefined;
}

async function secureFetch(path: string, init?: RequestInit, retry = true): Promise<Response> {
  if (!isMutation(init)) {
    return fetch(path, { ...init, credentials: "same-origin" });
  }
  const session = await initializeLocalSession();
  const headers = new Headers(init?.headers);
  headers.set("x-annotagent-csrf", session.csrf_token);
  const action = privilegedAction(path, init);
  if (action) {
    const confirmation = await fetch("/api/session/privileged-confirmation", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "content-type": "application/json",
        "x-annotagent-csrf": session.csrf_token,
      },
      body: JSON.stringify({ action, confirmed: true }),
    });
    if (!confirmation.ok) return confirmation;
    const value = await confirmation.json() as { confirmation_token?: string };
    if (!value.confirmation_token) throw new Error("Privileged confirmation did not return a token.");
    headers.set("x-annotagent-privileged-confirmation", value.confirmation_token);
  }
  const response = await fetch(path, {
    ...init,
    credentials: "same-origin",
    headers,
  });
  if (retry && response.status === 401) {
    await initializeLocalSession(true);
    return secureFetch(path, init, false);
  }
  return response;
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await secureFetch(path, {
    ...init,
    headers: { "content-type": "application/json", ...init?.headers },
  });
  if (!response.ok) {
    const body = (await response.json().catch(() => ({}))) as {
      error?: string;
      code?: string;
      active_run_id?: string;
      status?: string;
      suggested_action?: string;
    };
    const active =
      body.code === "active_run_exists"
        ? `Project already has active Run ${body.active_run_id ?? "unknown"} (${body.status ?? "active"}).`
        : undefined;
    const actionable = [body.error, body.suggested_action]
      .filter(Boolean)
      .join(" ");
    throw new ApiRequestError(
      active ?? (actionable || `${response.status} ${response.statusText}`),
      response.status,
      body.code,
      body.suggested_action,
    );
  }
  return response.json() as Promise<T>;
}

async function upload<T>(path: string, file: File): Promise<T> {
  const response = await secureFetch(path, {
    method: "POST",
    headers: { "content-type": "application/octet-stream" },
    body: file,
  });
  if (!response.ok) {
    const body = (await response.json().catch(() => ({}))) as {
      error?: string;
      suggested_action?: string;
    };
    throw new Error(
      [body.error, body.suggested_action].filter(Boolean).join(" ") ||
        `${response.status} ${response.statusText}`,
    );
  }
  return response.json() as Promise<T>;
}

async function requestNoContent(path: string, init?: RequestInit): Promise<void> {
  const response = await secureFetch(path, {
    ...init,
    headers: { "content-type": "application/json", ...init?.headers },
  });
  if (!response.ok) {
    const body = (await response.json().catch(() => ({}))) as {
      error?: string;
      suggested_action?: string;
    };
    throw new Error(
      [body.error, body.suggested_action].filter(Boolean).join(" ") ||
        `${response.status} ${response.statusText}`,
    );
  }
}

export const api = {
  health: () => request<{ status: string }>("/api/health"),
  providerPresets: () =>
    request<{ presets: ProviderPresetProfile[] }>("/api/provider-presets"),
  legacyRegistryImport: () =>
    request<{ migration: LegacyRegistryImportPreview }>(
      "/api/registry-migrations/legacy",
    ),
  applyLegacyRegistryImport: () =>
    request<{
      migration: LegacyRegistryImportReport;
      secret_moved: false;
      historical_runs_modified: false;
    }>("/api/registry-migrations/legacy", {
      method: "POST",
      body: JSON.stringify({ confirmed: true }),
    }),
  providers: () => request<{ providers: ProviderProfile[] }>("/api/providers"),
  createProvider: (value: {
    display_name: string;
    preset_id?: string;
    adapter: "open_ai_compatible";
    base_url: string;
    enabled?: boolean;
  }) =>
    request<ProviderProfile>("/api/providers", {
      method: "POST",
      body: JSON.stringify(value),
    }),
  updateProvider: (providerId: string, value: Partial<{
    display_name: string;
    preset_id: string | null;
    adapter: "open_ai_compatible";
    base_url: string;
    enabled: boolean;
  }>) =>
    request<ProviderProfile>(`/api/providers/${encodeURIComponent(providerId)}`, {
      method: "PATCH",
      body: JSON.stringify(value),
    }),
  deleteProvider: (providerId: string) =>
    request<{ deleted: string }>(`/api/providers/${encodeURIComponent(providerId)}`, {
      method: "DELETE",
    }),
  saveProviderCredential: (
    providerId: string,
    value: { source: CredentialSource; secret?: string; environment_variable?: string },
  ) =>
    request<{ provider_id: string; credential_configured: boolean; credential_source: CredentialSource }>(
      `/api/providers/${encodeURIComponent(providerId)}/credential`,
      { method: "POST", body: JSON.stringify(value) },
    ),
  deleteProviderCredential: (providerId: string) =>
    request<{ provider_id: string; credential_configured: boolean }>(
      `/api/providers/${encodeURIComponent(providerId)}/credential`,
      { method: "DELETE" },
    ),
  migrateProviderCredential: (providerId: string, deleteSourceAfterSuccess: boolean) =>
    request<{ provider_id: string; credential_configured: boolean; credential_source: CredentialSource; source_deleted: boolean }>(
      `/api/providers/${encodeURIComponent(providerId)}/migrate-credential`,
      {
        method: "POST",
        body: JSON.stringify({ delete_source_after_success: deleteSourceAfterSuccess }),
      },
    ),
  checkProvider: (providerId: string) =>
    request<{ provider: ProviderProfile; billable: false; check: { latency_ms: number; discovered_model_count: number; safe_message: string } }>(
      `/api/providers/${encodeURIComponent(providerId)}/check`,
      { method: "POST" },
    ),
  discoverProviderModels: (providerId: string) =>
    request<{ provider_id: string; models: { remote_model_id: string }[]; latency_ms: number; warning: string }>(
      `/api/providers/${encodeURIComponent(providerId)}/discover-models`,
      { method: "POST" },
    ),
  activeProbe: (providerId: string, modelProfileId: string) =>
    request<{ billable: true; usage: ProviderProbeUsage }>(
      `/api/providers/${encodeURIComponent(providerId)}/active-probe`,
      {
        method: "POST",
        body: JSON.stringify({ model_profile_id: modelProfileId, confirmed_billable: true }),
      },
    ),
  modelProfiles: (providerId?: string, allRevisions = false) =>
    request<{ models: RegistryModelProfile[] }>(
      `/api/model-profiles?${new URLSearchParams({
        ...(providerId ? { provider_id: providerId } : {}),
        ...(allRevisions ? { all_revisions: "true" } : {}),
      }).toString()}`,
    ),
  compatibleModelProfiles: (requirements: {
    input_modalities?: InputModality[];
    capabilities?: ModelCapability[];
    tool_calls?: boolean;
    structured_output?: boolean;
    json_schema?: boolean;
    allow_unverified?: boolean;
  }) =>
    request<{ models: RegistryModelProfile[] }>(
      `/api/model-profiles/compatible?${new URLSearchParams({
        ...(requirements.input_modalities?.length
          ? { input_modalities: requirements.input_modalities.join(",") }
          : {}),
        ...(requirements.capabilities?.length
          ? { capabilities: requirements.capabilities.join(",") }
          : {}),
        ...(requirements.tool_calls ? { tool_calls: "true" } : {}),
        ...(requirements.structured_output ? { structured_output: "true" } : {}),
        ...(requirements.json_schema ? { json_schema: "true" } : {}),
        ...(requirements.allow_unverified ? { allow_unverified: "true" } : {}),
      }).toString()}`,
    ),
  createModelProfile: (value: {
    provider_id: string;
    display_name: string;
    remote_model_id: string;
    input_modalities: InputModality[];
    task_capabilities: ModelCapability[];
    protocol_features: RegistryModelProfile["protocol_features"];
    pricing?: RegistryModelProfile["pricing"];
  }) =>
    request<RegistryModelProfile>("/api/model-profiles", {
      method: "POST",
      body: JSON.stringify(value),
    }),
  updateModelProfile: (modelId: string, value: Partial<RegistryModelProfile>) =>
    request<RegistryModelProfile>(`/api/model-profiles/${encodeURIComponent(modelId)}`, {
      method: "PATCH",
      body: JSON.stringify(value),
    }),
  deleteModelProfile: (modelId: string) =>
    request<{ deleted: string }>(`/api/model-profiles/${encodeURIComponent(modelId)}`, {
      method: "DELETE",
    }),
  modelProfileUsage: (modelId: string) =>
    request<{ model_profile_id: string; active_probes: ProviderProbeUsage[] }>(
      `/api/model-profiles/${encodeURIComponent(modelId)}/usage`,
    ),
  modelQualityContracts: (modelId: string) =>
    request<{
      model_profile_id: string;
      model_profile_revision: number;
      contracts: ModelCapabilityQualityContract[];
    }>(
      `/api/model-profiles/${encodeURIComponent(modelId)}/quality-contracts`,
    ),
  expertPlugins: () => request<ExpertPluginRegistry>("/api/plugins"),
  compatibleModelBundles: (pluginId: string, version: string) =>
    request<{
      plugin_runtime_status: string;
      available: ModelCatalogEntry[];
      installed: InstalledModelBundle[];
      setup_blockers: {
        bundle_id: string;
        bundle_version: string;
        code: string;
        message: string;
      }[];
    }>(`/api/plugins/${encodeURIComponent(pluginId)}/${encodeURIComponent(version)}/compatible-model-bundles`),
  modelBundles: () => request<{ bundles: InstalledModelBundle[] }>("/api/model-bundles"),
  availableModelBundles: () => request<{ bundles: ModelCatalogEntry[] }>("/api/model-bundles/available"),
  modelInstances: () => request<{ instances: InstalledModelInstance[]; model_profiles: ModelInstanceProfile[] }>("/api/model-instances"),
  inspectModelBundlePackage: (file: File) =>
    upload<VerifiedModelBundlePackage>(
      `/api/model-bundles/packages/inspect?${new URLSearchParams({ filename: file.name })}`,
      file,
    ),
  importModelBundlePackage: (file: File, licenseAccepted: boolean) =>
    upload<{ bundle: InstalledModelBundle; model_instances: InstalledModelInstance[] }>(
      `/api/model-bundles/import?${new URLSearchParams({ filename: file.name, license_accepted: String(licenseAccepted) })}`,
      file,
    ),
  acceptModelBundleLicense: (bundleId: string, version: string, licenseDigest: string) =>
    requestNoContent(
      `/api/model-bundles/${encodeURIComponent(bundleId)}/${encodeURIComponent(version)}/license-acceptance`,
      { method: "POST", body: JSON.stringify({ license_digest: licenseDigest }) },
    ),
  installModelBundle: (catalogId: string, bundleId: string, version: string) =>
    request<{ bundle: InstalledModelBundle; model_instances: InstalledModelInstance[] }>("/api/model-bundles/install", {
      method: "POST",
      body: JSON.stringify({ catalog_id: catalogId, bundle_id: bundleId, bundle_version: version }),
    }),
  modelInstallOperations: () =>
    request<{ operations: ModelInstallOperation[] }>("/api/model-installations"),
  modelInstallOperation: (operationId: string) =>
    request<ModelInstallOperation>(`/api/model-installations/${encodeURIComponent(operationId)}`),
  startModelInstallOperation: (value: {
    catalog_id: string;
    bundle_id: string;
    bundle_version: string;
    plugin_id: string;
    plugin_version: string;
  }) => request<ModelInstallOperation>("/api/model-installations", {
    method: "POST",
    body: JSON.stringify(value),
  }),
  testModelInstance: (instanceId: string) =>
    request<InstalledModelInstance>(`/api/model-instances/${encodeURIComponent(instanceId)}/test`, { method: "POST" }),
  testModelBundle: (bundleId: string, version: string) =>
    request<{ model_instances: InstalledModelInstance[] }>(
      `/api/model-bundles/${encodeURIComponent(bundleId)}/${encodeURIComponent(version)}/test`,
      { method: "POST" },
    ),
  setModelBundleEnabled: (bundleId: string, version: string, enabled: boolean) =>
    requestNoContent(
      `/api/model-bundles/${encodeURIComponent(bundleId)}/${encodeURIComponent(version)}/${enabled ? "enable" : "disable"}`,
      { method: "POST" },
    ),
  modelBundleReferences: (bundleId: string, version: string) =>
    request<{ references: { kind: string; location: string; created_at: string }[] }>(
      `/api/model-bundles/${encodeURIComponent(bundleId)}/${encodeURIComponent(version)}/references`,
    ),
  modelBundleCompatibility: (bundleId: string, version: string) =>
    request<{ compatibility: { plugin_id: string; plugin_version?: string; model_id: string; compatibility: { compatible: boolean; reasons: string[] } }[] }>(
      `/api/model-bundles/${encodeURIComponent(bundleId)}/${encodeURIComponent(version)}/compatibility`,
    ),
  inspectExpertPluginPackage: (file: File) =>
    upload<VerifiedExpertPluginPackage>(
      `/api/plugins/packages/inspect?${new URLSearchParams({ filename: file.name })}`,
      file,
    ),
  installExpertPluginPackage: (
    file: File,
    approval: {
      permissions_reviewed: boolean;
      code_license_accepted: boolean;
      weight_license_accepted: boolean;
    },
  ) =>
    upload<{ plugin_id: string; version: string; status: string; enabled: boolean }>(
      `/api/plugins/packages/install?${new URLSearchParams({
        filename: file.name,
        permissions_reviewed: String(approval.permissions_reviewed),
        code_license_accepted: String(approval.code_license_accepted),
        weight_license_accepted: String(approval.weight_license_accepted),
      })}`,
      file,
    ),
  provisionExpertPluginWeights: (
    pluginId: string,
    version: string,
    modelId: string,
    file: File,
    componentId?: string,
    sha256?: string,
  ) =>
    upload<{ model_id: string; component_id: string; checkpoint_sha256: string; size_bytes: number }>(
      `/api/plugins/${encodeURIComponent(pluginId)}/${encodeURIComponent(version)}/weights?${new URLSearchParams({
        filename: file.name,
        model_id: modelId,
        ...(componentId ? { component_id: componentId } : {}),
        ...(sha256 ? { sha256 } : {}),
      })}`,
      file,
    ),
  createLegacyLocalModelBundle: (
    pluginId: string,
    version: string,
    value: {
      model_id: string;
      bundle_version: string;
      display_name: string;
      upstream_project: string;
      upstream_model_id: string;
      upstream_version?: string;
      source_url?: string;
      exporter_name: string;
      exporter_version: string;
      opset: number;
      license_name: string;
      license_url?: string;
      redistribution: "allowed" | "restricted" | "prohibited" | "unknown";
      commercial_use: "allowed" | "restricted" | "unknown";
      license_text: string;
      contract_document: string;
      license_accepted: boolean;
    },
  ) => request<{
    bundle: InstalledModelBundle;
    model_instances: InstalledModelInstance[];
    local_bundle_path: string;
    legacy_files_preserved: true;
  }>(`/api/plugins/${encodeURIComponent(pluginId)}/${encodeURIComponent(version)}/legacy-model-bundle`, {
    method: "POST",
    body: JSON.stringify(value),
  }),
  testExpertPlugin: (pluginId: string, version: string) =>
    request<{ status: string; report: { passed: boolean; checks: { name: string; passed: boolean; detail: string }[] } }>(
      `/api/plugins/${encodeURIComponent(pluginId)}/${encodeURIComponent(version)}/test`,
      { method: "POST" },
    ),
  setExpertPluginEnabled: (pluginId: string, version: string, enabled: boolean) =>
    request<{ status: string; enabled: boolean }>(
      `/api/plugins/${encodeURIComponent(pluginId)}/${encodeURIComponent(version)}/${enabled ? "enable" : "disable"}`,
      { method: "POST" },
    ),
  uninstallExpertPlugin: (pluginId: string, version: string) =>
    request<{ uninstalled: string }>(
      `/api/plugins/${encodeURIComponent(pluginId)}/${encodeURIComponent(version)}`,
      { method: "DELETE" },
    ),
  dashboard: (signal?: AbortSignal) =>
    request<DashboardData>("/api/projects", { signal }),
  createProject: (id: string, yaml: string) =>
    request<ProjectSummary>("/api/projects", {
      method: "POST",
      body: JSON.stringify({ id, yaml }),
    }),
  conversation: (projectId: string, signal?: AbortSignal) => request<{ conversation_id: string | null }>(`/api/projects/${encodeURIComponent(projectId)}/conversations`, { signal }),
  conversationTasks: (project: string, conversation: string, signal?: AbortSignal) => request<import("./types").ConversationTask[]>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks`, { signal }),
  conversationHumanRequests: (project:string, conversation:string, task:string, signal?:AbortSignal) => request<import("./conversation-human-api").HumanRequest[]>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/human-requests`,{signal}),
  answerHumanRequest: (project:string, value:import("./conversation-human-api").HumanRequest, answer:import("./types").SampleFeedbackRevision) => request<import("./conversation-human-api").HumanRequest>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(value.input.conversation_id)}/tasks/${encodeURIComponent(value.input.task_id)}/human-requests/${encodeURIComponent(value.input.id)}/answer`,{method:"POST",body:JSON.stringify({answer})}),
  cancelHumanRequest: (project:string, value:import("./conversation-human-api").HumanRequest) => request<import("./conversation-human-api").HumanRequest>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(value.input.conversation_id)}/tasks/${encodeURIComponent(value.input.task_id)}/human-requests/${encodeURIComponent(value.input.id)}/cancel`,{method:"POST"}),
  resumeHumanRequest: (project:string, value:import("./conversation-human-api").HumanRequest) => request<import("./conversation-human-api").HumanRequest>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(value.input.conversation_id)}/tasks/${encodeURIComponent(value.input.task_id)}/human-requests/${encodeURIComponent(value.input.id)}/resume`,{method:"POST"}),
  beginConversationTask: (project: string, conversation: string, input: import("./types").ConversationTask["input"]) => request<import("./types").ConversationTask>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks`, { method: "POST", body: JSON.stringify(input) }),
  conversationSchemaPreview: (project: string, conversation: string, task: string) => request<import("./types").ConversationSchemaPreview>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/schema-preview`),
  conversationBuilderPreview: (project: string, conversation: string, task: string, selection: import("./types").ConversationBuilderSelection) => request<import("./types").ConversationBuilderPreview>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/builder-preview?${new URLSearchParams(Object.entries(selection).filter(([,value])=>value!==undefined).map(([key,value])=>[key,String(value)]))}`),
  conversationBuilderHistory: (project: string, conversation: string, task: string, signal?: AbortSignal) => request<{items: import("./types").ConversationBuilderItem[]}>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/builder-operations`,{signal}),
  launchConversationBuilder: (project: string, conversation: string, task: string, consent: import("./types").ConversationBuilderConsent) => request<import("./types").ConversationBuilderOperation>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/builder-operations`,{method:"POST",body:JSON.stringify(consent)}),
  conversationSchemaDraftForCall: (project: string, conversation: string, task: string, call: string, signal?: AbortSignal) => request<import("./types").ConversationSchemaDraft | null>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/calls/${encodeURIComponent(call)}/schema-draft`, { signal }),
  saveConversationSchemaDraft: (project: string, conversation: string, task: string, call: string) => request<import("./types").ConversationSchemaDraft>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/calls/${encodeURIComponent(call)}/schema-draft`, { method: "POST" }),
  editConversationSchemaDraft: (project: string, draft: string, input: { request_id: string; expected_revision: number; decision: unknown }) => request<import("./types").ConversationSchemaDraft>(`/api/projects/${encodeURIComponent(project)}/conversation-schema-drafts/${encodeURIComponent(draft)}`, { method: "POST", body: JSON.stringify(input) }),
  conversationSchemaCalls: (project: string, conversation: string, task: string, signal?: AbortSignal) => request<import("./types").ConversationCallReceipt[]>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/calls`, { signal }),
  proposeConversationSchema: (project: string, conversation: string, task: string, consent: { call_id: string; model_id: string; scope_hash: string; expires_at: string; allow_unknown_cost: boolean }) => request<import("./types").ConversationCallReceipt>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/schema-proposals`, { method: "POST", body: JSON.stringify(consent) }),
  cancelConversationSchema: (project: string, conversation: string, task: string, call: string) => request<import("./types").ConversationCallCancellation>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/calls/${encodeURIComponent(call)}/cancel`, { method: "POST" }),
  conversationSchemaCancellations: (project: string, conversation: string, task: string, signal?: AbortSignal) => request<import("./types").ConversationCallCancellation[]>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/cancellations`, { signal }),
  createConversation: (projectId: string) => request<{ conversation_id: string }>(`/api/projects/${encodeURIComponent(projectId)}/conversations`, { method: "POST" }),
  conversationMessages: (projectId: string, conversationId: string, after = 0, signal?: AbortSignal) => request<import("./types").ConversationMessage[]>(`/api/projects/${encodeURIComponent(projectId)}/conversations/${encodeURIComponent(conversationId)}/messages?after=${after}`, { signal }),
  sendConversationMessage: (projectId: string, conversationId: string, input: import("./types").ConversationMessageInput) => request<import("./types").ConversationMessage>(`/api/projects/${encodeURIComponent(projectId)}/conversations/${encodeURIComponent(conversationId)}/messages`, { method: "POST", body: JSON.stringify(input) }),
  projectGoal: (projectId: string, signal?: AbortSignal) => request<{
    revision: string; goal: string; kind: string | null; labels: string[] | null; editable: boolean;
  }>(`/api/projects/${encodeURIComponent(projectId)}/goal`, { signal }),
  saveProjectGoal: (projectId: string, value: { expected_revision: string; goal: string; kind: string; labels: string[] }) =>
    request<{ revision: string; goal: string; kind: string | null; labels: string[] | null; editable: boolean }>(`/api/projects/${encodeURIComponent(projectId)}/goal`, { method: "PUT", body: JSON.stringify(value) }),
  projectSummary: (projectId: string, signal?: AbortSignal) =>
    request<ProjectWorkspaceSummary>(`/api/projects/${projectId}/summary`, { signal }),
  projectModelBindings: (projectId: string) =>
    request<{ project_id: string; bindings: ProjectModelBinding[] }>(
      `/api/projects/${encodeURIComponent(projectId)}/model-bindings`,
    ),
  saveProjectModelBindings: (
    projectId: string,
    bindings: {
      capability: ModelCapability;
      role: ModelBindingRole;
      match_kind: "capability" | "role";
      model_profile_id: string;
      locked: boolean;
    }[],
  ) =>
    request<{ project_id: string; bindings: ProjectModelBinding[] }>(
      `/api/projects/${encodeURIComponent(projectId)}/model-bindings`,
      { method: "PUT", body: JSON.stringify({ bindings }) },
    ),
  selectProjectModelBinding: (projectId: string, binding: { capability: ModelCapability; role: ModelBindingRole; match_kind: "capability" | "role"; model_profile_id: string; locked: boolean }, expectedBindingId?: string) =>
    request<{ project_id: string; binding: ProjectModelBinding }>(`/api/projects/${encodeURIComponent(projectId)}/model-bindings`, { method: "POST", body: JSON.stringify({ binding, expected_binding_id: expectedBindingId ?? null }) }),
  agentModelBindings: () =>
    request<GlobalModelDefaults>("/api/agent-model-bindings"),
  saveAgentModelBindings: (defaults: GlobalModelDefaults) =>
    request<GlobalModelDefaults>("/api/agent-model-bindings", {
      method: "PUT",
      body: JSON.stringify(defaults),
    }),
  images: (projectId: string, signal?: AbortSignal) =>
    request<{ images: ImageItem[] }>(`/api/projects/${projectId}/images`, { signal }),
  importImages: (projectId: string, source: string) =>
    request<{
      source: string;
      discovered: number;
      imported: number;
      duplicates: number;
      corrupt: { name: string; message: string }[];
      unsupported_files: number;
      supported_formats: string[];
    }>(
      `/api/projects/${projectId}/import`,
      { method: "POST", body: JSON.stringify({ source }) },
    ),
  removeImage: (projectId: string, imageId: string, expectedContentHash: string) =>
    request<{ removed: string }>(`/api/projects/${projectId}/images/${imageId}?expected_content_hash=${encodeURIComponent(expectedContentHash)}`, {
      method: "DELETE",
    }),
  uploadImage: (projectId: string, file: File) => upload<{ imported: number; duplicates: number; corrupt: { name: string; message: string }[] }>(`/api/projects/${encodeURIComponent(projectId)}/image-upload?name=${encodeURIComponent(file.name)}`, file),
  sampleFeedback: (testId: string, imageId: string) => request<{ revisions: import("./types").SampleFeedbackRevision[] }>(`/api/workflow-sample-tests/${encodeURIComponent(testId)}/images/${encodeURIComponent(imageId)}/feedback`),
  copySamplePlan: (projectId: string, copyId: string, testId: string) => request<WorkflowDraft>(`/api/projects/${encodeURIComponent(projectId)}/sample-plan-copies/${encodeURIComponent(copyId)}`, { method: "POST", body: JSON.stringify({ sample_test_id: testId }) }),
  samplePlanEvidence: (projectId: string, copyId: string) => request<{project_id:string; sample_test_id:string; baseline_draft_id:string; feedback:import("./types").SampleFeedbackRevision[]}>(`/api/projects/${encodeURIComponent(projectId)}/sample-plan-copies/${encodeURIComponent(copyId)}`),
  saveSampleFeedback: (revision: import("./types").SampleFeedbackRevision) => request<{ revision: import("./types").SampleFeedbackRevision }>(`/api/workflow-sample-tests/${encodeURIComponent(revision.sample_test_id)}/images/${encodeURIComponent(revision.image_id)}/feedback`, { method: "POST", body: JSON.stringify(revision) }),
  startRun: (
    projectId: string,
    workflow: { workflow_id: string; version: number },
    idempotencyKey: string = crypto.randomUUID(),
    imageId?: string,
  ) =>
    request<{
      run_id: string;
      image_id: string;
      image_path: string;
      status: string;
      idempotent: boolean;
    }>(`/api/projects/${projectId}/runs`, {
      method: "POST",
      headers: { "idempotency-key": idempotencyKey },
      body: JSON.stringify({ ...workflow, ...(imageId ? { image_id: imageId } : {}) }),
    }),
  startBatch: (
    projectId: string,
    limit: number | undefined,
    workflow: { workflow_id: string; version: number },
  ) =>
    request<{ batch: { id: string; status: string } }>(
      `/api/projects/${projectId}/batches`,
      {
        method: "POST",
        body: JSON.stringify({
          ...(limit ? { limit } : {}),
          ...workflow,
        }),
      },
    ),
  control: (runId: string, action: "pause" | "resume" | "cancel") =>
    request(`/api/runs/${runId}/${action}`, { method: "POST" }),
  controlBatch: (batchId: string, action: "pause" | "resume" | "cancel") =>
    request(`/api/batches/${batchId}/${action}`, { method: "POST" }),
  runEvents: (runId: string, signal?: AbortSignal) =>
    request<{ events: RunEvent[] }>(`/api/runs/${runId}/events`, { signal }),
  run: (runId: string, signal?: AbortSignal) =>
    request<{ run: HistoryRun; event_count: number }>(`/api/runs/${runId}`, { signal }),
  runProvenance: (runId: string, signal?: AbortSignal) =>
    request<RunProvenanceSummary>(`/api/runs/${runId}/provenance`, { signal }),
  runs: (signal?: AbortSignal, offset = 0, projectId?: string) => {
    const params = new URLSearchParams({ limit: "100", offset: String(offset) });
    if (projectId) params.set("project_id", projectId);
    return request<{ runs: HistoryRun[]; page: PageMetadata }>(`/api/runs?${params}`, { signal });
  },
  batches: () => request<{ batches: DatasetBatchSummary[]; page: PageMetadata }>("/api/batches?limit=100"),
  batch: (batchId: string, signal?: AbortSignal) =>
    request<{
      batch: DatasetBatchSummary;
      progress: DatasetBatchSummary["progress"];
      events: unknown[];
    }>(`/api/batches/${batchId}`, { signal }),
  previewManagement: (projectId: string, body: ManagementRequest) =>
    request<ManagementPreview>(
      `/api/projects/${encodeURIComponent(projectId)}/management/preview`,
      { method: "POST", body: JSON.stringify(body) },
    ),
  executeManagement: async (projectId: string, body: ManagementRequest) => {
    const receipt = await request<ManagementReceipt>(
      `/api/projects/${encodeURIComponent(projectId)}/management/actions`,
      { method: "POST", body: JSON.stringify(body) },
    );
    if (typeof window !== "undefined") {
      window.dispatchEvent(new CustomEvent("annotagent:management", { detail: receipt }));
      window.localStorage.setItem("annotagent.management.receipt", JSON.stringify(receipt));
    }
    return receipt;
  },
  managementOperation: (projectId: string, operationId: string) =>
    request<ManagementReceipt>(
      `/api/projects/${encodeURIComponent(projectId)}/management/operations/${encodeURIComponent(operationId)}`,
    ),
  managementUsage: (projectId: string) =>
    request<ManagementUsageSummary>(
      `/api/projects/${encodeURIComponent(projectId)}/management/usage`,
    ),
  trash: (projectId: string, kind?: ManagementObjectKind) =>
    request<{ items: TrashEntry[] }>(
      `/api/projects/${encodeURIComponent(projectId)}/trash${kind ? `?kind=${encodeURIComponent(kind)}` : ""}`,
    ),
  pipelineLifecycle: (
    projectId: string,
    includeArchived = false,
    includeDeleted = false,
  ) => {
    const params = new URLSearchParams();
    if (includeArchived) params.set("include_archived", "true");
    if (includeDeleted) params.set("include_deleted", "true");
    return request<{ pipelines: PipelineLifecycleSummary[] }>(
      `/api/projects/${encodeURIComponent(projectId)}/pipelines${params.size ? `?${params}` : ""}`,
    );
  },
  workflows: () => request<{ workflows: ProjectWorkflow[] }>("/api/workflows"),
  workflowDrafts: (projectId?: string, signal?: AbortSignal) =>
    request<{
      drafts: WorkflowDraft[];
      latest_current_sample_test_draft_id?: string | null;
    }>(
      `/api/workflow-drafts${projectId ? `?project_id=${encodeURIComponent(projectId)}` : ""}`,
      { signal },
    ),
  workflowCatalog: (projectId: string, taskId?: string, label?: string) =>
    request<WorkflowCatalog>(
      `/api/projects/${projectId}/workflow-catalog${
        taskId && label
          ? `?target_task_id=${encodeURIComponent(taskId)}&target_label=${encodeURIComponent(label)}`
          : ""
      }`,
    ),
  createWorkflowDraft: (
    projectId: string,
    fromTemplate = false,
    templateId?: string,
  ) =>
    request<WorkflowDraft>("/api/workflow-drafts", {
      method: "POST",
      body: JSON.stringify({
        project_id: projectId,
        from_template: fromTemplate,
        ...(templateId ? { template_id: templateId } : {}),
      }),
    }),
  suggestWorkflow: (
    projectId: string,
    advisor: "llm" = "llm",
    target?: { task_id: string; label: string },
    constraints?: {
      require_review_gate?: boolean;
      max_cost_per_image?: string;
      max_latency_ms?: number;
      minimum_accuracy?: number;
    },
    builderConstraints?: PipelineBuilderConstraints,
    agentModelProfileId?: string,
    retry?: { session_id?: string; base_draft_id?: string },
    buildMode: PipelineBuildMode = { kind: "from_scratch" },
    planningAuthorization?: { model_revision: number; provider_id: string; base_url: string; goal_revision: string },
  ) =>
    request<WorkflowSuggestion>("/api/workflow-drafts/suggest", {
      method: "POST",
      body: JSON.stringify({
        project_id: projectId,
        advisor,
        ...(target
          ? { target_task_id: target.task_id, target_label: target.label }
          : {}),
        constraints: { require_review_gate: true, ...constraints },
        builder_constraints: builderConstraints,
        build_mode: buildMode,
        ...(planningAuthorization ? { planning_authorization: planningAuthorization } : {}),
        ...(agentModelProfileId
          ? { agent_model_profile_id: agentModelProfileId }
          : {}),
        ...(retry?.session_id ? { retry_session_id: retry.session_id } : {}),
        ...(retry?.base_draft_id ? { base_draft_id: retry.base_draft_id } : {}),
      }),
    }),
  workflowDraftDiff: (baseDraftId: string, proposedDraftId: string) =>
    request<PipelineDraftDiff>("/api/workflow-drafts/diff", {
      method: "POST",
      body: JSON.stringify({
        base_draft_id: baseDraftId,
        proposed_draft_id: proposedDraftId,
      }),
    }),
  applyWorkflowDraftDiff: (
    baseDraftId: string,
    proposedDraftId: string,
    selectedChangeIds: string[],
  ) =>
    request<WorkflowDraftApplyReport>(
      `/api/workflow-drafts/${encodeURIComponent(baseDraftId)}/apply-diff`,
      {
        method: "POST",
        body: JSON.stringify({
          proposed_draft_id: proposedDraftId,
          selected_change_ids: selectedChangeIds,
        }),
      },
    ),
  addProjectLabel: (projectId: string, taskId: string, label: string) =>
    request<ProjectSummary>(`/api/projects/${projectId}/schema/labels`, {
      method: "POST",
      body: JSON.stringify({ task_id: taskId, label }),
    }),
  setProjectSkills: (
    projectId: string,
    enabledSkills: { id: string; version: string }[],
  ) =>
    request<ProjectSummary>(`/api/projects/${projectId}/skills`, {
      method: "POST",
      body: JSON.stringify({ enabled_skills: enabledSkills }),
    }),
  addProjectTask: (
    projectId: string,
    task: {
      display_name: string;
      kind: string;
      labels: string[];
      attributes: Record<
        string,
        { type: "enum" | "string" | "number" | "boolean"; required: boolean; values: string[] }
      >;
    },
  ) =>
    request<ProjectSummary>(`/api/projects/${projectId}/schema/tasks`, {
      method: "POST",
      body: JSON.stringify(task),
    }),
  pipelineArtifacts: (runId: string, signal?: AbortSignal) =>
    request<RunNodeArtifactInspection>(
      `/api/runs/${runId}/pipeline-artifacts`,
      { signal },
    ),
  runResultSummary: (runId: string, signal?: AbortSignal) =>
    request<RunResultSummary>(`/api/runs/${runId}/result-summary`, { signal }),
  runDebugSummary: (runId: string, signal?: AbortSignal) =>
    request<RunDebugSummary>(`/api/runs/${runId}/debug-summary`, { signal }),
  runAnnotations: (runId: string, signal?: AbortSignal) =>
    request<RunAnnotationInspection>(`/api/runs/${runId}/annotations`, { signal }),
  replayNode: (runId: string, nodeId: string) =>
    request<NodeReplayReport>(
      `/api/runs/${runId}/replay/${encodeURIComponent(nodeId)}`,
      { method: "POST" },
    ),
  saveWorkflowDraft: (draft: WorkflowDraft, signal?: AbortSignal) =>
    request<WorkflowDraft>(`/api/workflow-drafts/${draft.id}`, {
      method: "PATCH",
      headers: { "if-match": String(draft.revision) },
      body: JSON.stringify(draft),
      signal,
    }),
  samplePreview: (draftId: string) => request<{ project_id: string; revision: number; image_count: number; models: { name: string; destination: string; id: string; revision: number }[]; other_bindings: string[]; estimated_cost: null; request_limit: number; sandbox: true; supported: boolean; authorization_fingerprint: string }>(`/api/workflow-drafts/${encodeURIComponent(draftId)}/sample-preview`),
  processingPreview: (projectId: string, selection: ProcessingSelection, signal?: AbortSignal) => request<ProcessingAuthorization>(`/api/projects/${encodeURIComponent(projectId)}/processing-preview?${new URLSearchParams({ draft_id: selection.draft_id, sample_test_id: selection.sample_test_id, ...(selection.limit ? { limit: String(selection.limit) } : {}) })}`, { signal }),
  confirmProcessing: (projectId: string, input: ConfirmProcessingRequest) => request<ProcessingReceipt>(`/api/projects/${encodeURIComponent(projectId)}/processing-operations`, { method: "POST", body: JSON.stringify(input) }),
  processingOperation: (projectId: string, id: string, signal?: AbortSignal) => request<ProcessingReceipt>(`/api/projects/${encodeURIComponent(projectId)}/processing-operations/${encodeURIComponent(id)}`, { signal }),
  conversationSamplePreview: (project: string, conversation: string, task: string, draft: string, requestId: string) => request<ConversationSamplePreview>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/sample-preview?${new URLSearchParams({draft_id:draft,request_id:requestId})}`),
  conversationSampleHistory: (project: string, conversation: string, task: string, signal?: AbortSignal) => request<{items:SampleOperation[]}>(`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/sample-operations`,{signal}),
  startSampleOperation: (projectId: string, input: { request_id: string; draft_id: string; image_indices: number[]; expected_revision: number; authorization_fingerprint: string; conversation?: ConversationSampleConsent }) => request<SampleOperation>(`/api/projects/${encodeURIComponent(projectId)}/sample-operations`, { method: "POST", body: JSON.stringify(input) }),
  sampleOperation: (projectId: string, id: string, signal?: AbortSignal) => request<SampleOperation>(`/api/projects/${encodeURIComponent(projectId)}/sample-operations/${encodeURIComponent(id)}`, { signal }),
  cancelSampleOperation: (projectId: string, id: string) => request<SampleOperation>(`/api/projects/${encodeURIComponent(projectId)}/sample-operations/${encodeURIComponent(id)}/cancel`, { method: "POST" }),
  dryRunWorkflow: (draftId: string, imageIndices: number[] = [], expectedRevision?: number, authorizationFingerprint?: string) =>
    request<WorkflowDryRunReport>(`/api/workflow-drafts/${draftId}/dry-run`, {
      method: "POST",
      body: JSON.stringify({ image_indices: imageIndices, expected_revision: expectedRevision, authorization_fingerprint: authorizationFingerprint }),
    }),
  workflowSampleTest: (draftId: string, signal?: AbortSignal, sampleTestId?: string) =>
    request<{
      sample_test?: WorkflowSampleTestRecord | null;
      current: boolean;
      annotation_schema?: {schema_draft_id:string;revision:number;task:import("./types").ConversationSchemaDraft["definition"]["task"]} | null;
    }>(`/api/workflow-drafts/${encodeURIComponent(draftId)}/sample-test${
      sampleTestId ? `?test_id=${encodeURIComponent(sampleTestId)}` : ""
    }`, { signal }),
  publishWorkflow: (draftId: string) =>
    request<{ workflow_id: string; version: number }>(
      `/api/workflow-drafts/${draftId}/publish`,
      { method: "POST" },
    ),
  archiveWorkflowDraft: (draftId: string) =>
    request<WorkflowDraft>(`/api/workflow-drafts/${draftId}/archive`, {
      method: "POST",
    }),
  cloneWorkflowVersion: (workflowId: string, version: number) =>
    request<WorkflowDraft>(
      `/api/workflows/${workflowId}/versions/${version}/clone`,
      { method: "POST" },
    ),
  compareWorkflowVersions: (
    left: { workflow_id: string; version: number },
    right: { workflow_id: string; version: number },
  ) =>
    request<WorkflowVersionComparison>("/api/workflows/compare", {
      method: "POST",
      body: JSON.stringify({
        left_workflow_id: left.workflow_id,
        left_version: left.version,
        right_workflow_id: right.workflow_id,
        right_version: right.version,
      }),
    }),
  createGeometrySafeDraft: (workflowId: string, version: number) =>
    request<WorkflowDraft>(
      `/api/workflows/${encodeURIComponent(workflowId)}/versions/${version}/create-geometry-safe-draft`,
      { method: "POST" },
    ),
  geometryPolicy: (projectId: string) =>
    request<{ policies: ProjectGeometryPolicy[] }>(
      `/api/projects/${encodeURIComponent(projectId)}/geometry-policy`,
    ),
  geometryCalibrations: (projectId: string) =>
    request<{ calibrations: GeometryCalibrationView[] }>(
      `/api/projects/${encodeURIComponent(projectId)}/geometry-calibrations`,
    ),
  createGeometryCalibration: (
    projectId: string,
    value: {
      workflow_id: string;
      workflow_version: number;
      node_id: string;
      task_id: string;
      label_id?: string;
      evidence_run_ids: string[];
    },
  ) =>
    request<{ calibration: GeometryCalibrationReport }>(
      `/api/projects/${encodeURIComponent(projectId)}/geometry-calibrations`,
      { method: "POST", body: JSON.stringify(value) },
    ),
  geometryCorrections: (projectId: string) =>
    request<{
      summary: Record<string, unknown>;
      reports: unknown[];
      evidence: unknown[];
    }>(`/api/projects/${encodeURIComponent(projectId)}/geometry-corrections`),
  runGeometryQuality: (runId: string) =>
    request<{ summary: GeometryQualitySummary; reports: unknown[]; evidence: unknown[] }>(
      `/api/runs/${encodeURIComponent(runId)}/geometry-quality`,
    ),
  pipelineImprovements: (projectId: string, signal?: AbortSignal) =>
    request<{ pipeline_improvements: PipelineImprovementSession[] }>(
      `/api/projects/${encodeURIComponent(projectId)}/pipeline-improvements`,
      { signal },
    ),
  createPipelineImprovement: (
    projectId: string,
    value: {
      workflow_id: string;
      workflow_version: number;
      target_task_id: string;
      target_label: string;
      evidence_run_ids: string[];
      evaluation_run_ids: string[];
    },
  ) =>
    request<PipelineImprovementSession>(
      `/api/projects/${encodeURIComponent(projectId)}/pipeline-improvements`,
      { method: "POST", body: JSON.stringify(value) },
    ),
  comparePipelineImprovement: (improvementId: string) =>
    request<PipelineImprovementSession>(
      `/api/pipeline-improvements/${encodeURIComponent(improvementId)}/compare`,
      { method: "POST", body: JSON.stringify({}) },
    ),
  applyPipelineImprovement: (improvementId: string, selectedChangeIds: string[]) =>
    request<PipelineImprovementSession>(
      `/api/pipeline-improvements/${encodeURIComponent(improvementId)}/apply-to-draft`,
      {
        method: "POST",
        body: JSON.stringify({ selected_change_ids: selectedChangeIds }),
      },
    ),
  models: () => request<{ models: ModelBinding[] }>("/api/models"),
  testModel: (modelId: string) =>
    request<DetectionWorkerTestResult>(`/api/models/${encodeURIComponent(modelId)}/test`, {
      method: "POST",
    }),
  sampleTestModel: (
    modelId: string,
    value: { project_id: string; image_index: number; query?: string; box_prompt?: number[] },
  ) =>
    request<DetectionWorkerSampleTestResult>(
      `/api/models/${encodeURIComponent(modelId)}/sample-test`,
      { method: "POST", body: JSON.stringify(value) },
    ),
  reviews: (projectId?: string, signal?: AbortSignal, offset = 0) =>
    request<{ reviews: ReviewItem[]; progress: ReviewQueueProgress; page: PageMetadata }>(
      projectId
        ? `/api/projects/${encodeURIComponent(projectId)}/reviews?limit=50&offset=${offset}`
        : `/api/reviews?limit=50&offset=${offset}`,
      { signal },
    ),
  review: (id: string, signal?: AbortSignal, projectId?: string) =>
    request<ReviewItem>(projectId
      ? `/api/projects/${encodeURIComponent(projectId)}/reviews/${encodeURIComponent(id)}`
      : `/api/reviews/${encodeURIComponent(id)}`, { signal }),
  reviewNext: (id: string, projectId?: string) =>
    request<ReviewNavigation>(
      projectId
        ? `/api/projects/${encodeURIComponent(projectId)}/reviews/${encodeURIComponent(id)}/next`
        : `/api/reviews/${encodeURIComponent(id)}/next`,
    ),
  runReviews: (runId: string, signal?: AbortSignal) =>
    request<{ reviews: ReviewItem[]; progress: ReviewQueueProgress; page: PageMetadata }>(
      `/api/runs/${encodeURIComponent(runId)}/reviews?limit=50`,
      { signal },
    ),
  revise: (annotation: Annotation, reason: string) =>
    request(`/api/annotations/${annotation.id}`, {
      method: "PATCH",
      body: JSON.stringify({ annotation, reason }),
    }),
  createAnnotation: (runId: string, annotation: Annotation) =>
    request<{ annotation: Annotation }>(`/api/runs/${runId}/annotations`, {
      method: "POST",
      body: JSON.stringify({ annotation }),
    }),
  decide: (
    id: string,
    projectId: string,
    decision: "accept" | "reject" | "delete",
    reasonCode: string,
    note: string,
    skillId?: string,
  ) =>
    request(`/api/reviews/${id}/decision`, {
      method: "POST",
      body: JSON.stringify({
        project_id: projectId,
        decision,
        reason_code: reasonCode,
        note,
        skill_id: skillId,
      }),
    }),
  decideAndNext: (
    id: string,
    projectId: string,
    decision: "accept" | "reject",
    reasonCode: string,
    note: string,
    skillId?: string,
    queueProjectId?: string,
  ) =>
    request<ReviewDecisionOutcome>(
      `/api/projects/${encodeURIComponent(projectId)}/reviews/${encodeURIComponent(id)}/${decision}-and-next`,
      {
        method: "POST",
        body: JSON.stringify({
          decision,
          reason_code: reasonCode,
          note,
          skill_id: skillId,
          queue_project_id: queueProjectId,
        }),
      },
    ),
  revisions: (id: string, projectId?: string) =>
    request<{ revisions: AnnotationRevision[] }>(projectId
      ? `/api/projects/${encodeURIComponent(projectId)}/reviews/${encodeURIComponent(id)}/revisions`
      : `/api/annotations/${encodeURIComponent(id)}/revisions`),
  skills: () => request<SkillDetail[]>("/api/skills"),
  agentSessions: (projectId: string, signal?: AbortSignal) =>
    request<{ sessions: AgentSession[] }>(
      `/api/projects/${projectId}/agent-sessions`,
      { signal },
    ),
  cancelAgentSession: (sessionId: string) =>
    request<{ session: AgentSession }>(
      `/api/agent-sessions/${sessionId}/cancel`,
      { method: "POST" },
    ),
  correctionMemory: (projectId: string) =>
    request<{ records: CorrectionMemoryRecord[] }>(
      `/api/projects/${projectId}/correction-memory`,
    ),
  settings: () => request<Record<string, unknown>>("/api/settings"),
  saveSettings: (value: Record<string, unknown>) =>
    request<Record<string, unknown>>("/api/settings", {
      method: "PUT",
      body: JSON.stringify(value),
    }),
  exportReadiness: (projectId: string, signal?: AbortSignal) =>
    request<ExportReadiness>(`/api/projects/${projectId}/export-readiness`, { signal }),
  export: (projectId: string, format: string) =>
    request<ProjectExportResult>(`/api/projects/${projectId}/export`, {
      method: "POST",
      body: JSON.stringify({ format }),
    }),
  importAnnotations: (
    projectId: string,
    format: string,
    source: string,
    dryRun: boolean,
  ) =>
    request<{
      format: string;
      dry_run: boolean;
      imported_count: number;
      skipped_count: number;
      warnings: string[];
      issues: { record: string; message: string }[];
    }>(`/api/projects/${projectId}/annotation-import`, {
      method: "POST",
      body: JSON.stringify({
        format,
        source,
        dry_run: dryRun,
        label_mapping: {},
      }),
    }),
};

export function subscribeEvents(
  onEvent: (event: RunEvent) => void,
  onReconnect: () => void,
  onOpen?: () => void,
): () => void {
  const source = new EventSource("/api/events");
  const kinds = [
    "run_created",
    "run_started",
    "task_started",
    "task_completed",
    "task_failed",
    "model_call_started",
    "model_call_completed",
    "model_call_failed",
    "tool_call_started",
    "tool_call_completed",
    "validation_completed",
    "refinement_started",
    "refinement_completed",
    "artifact_created",
    "artifact_validated",
    "artifact_committed",
    "retry_scheduled",
    "annotation_committed",
    "review_requested",
    "usage_updated",
    "run_paused",
    "run_resumed",
    "run_cancelled",
    "run_budget_exceeded",
    "run_completed",
    "run_failed",
    "run_interrupted",
  ];
  for (const kind of kinds) {
    source.addEventListener(kind, (message) => {
      onEvent(JSON.parse((message as MessageEvent<string>).data) as RunEvent);
    });
  }
  source.onerror = () => onReconnect();
  source.onopen = () => onOpen?.();
  return () => source.close();
}
