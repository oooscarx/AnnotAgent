import type { RegistryModelProfile } from "../types";

export type DemoMode = "preset_candidates" | "live_model";

export type DemoModeAvailability = {
  source_mode: DemoMode;
  status: "ready" | "setup_required" | "unavailable";
  reason: string | null;
  required_capabilities: string[];
};

export type DemoCatalogEntry = {
  demo_id: string;
  version: string;
  manifest_sha256: string;
  title: string;
  summary: string;
  learning_objectives: string[];
  image_count: number;
  labels: string[];
  delivery_format: string;
  thumbnail_asset_id: string;
  thumbnail_url: string;
  license: {
    spdx_id: string;
    source_url: string;
    attribution_asset_id: string;
  };
  modes: DemoModeAvailability[];
};

export type DemoCatalog = {
  contract_version: "demo-catalog-v1";
  catalog_revision: string;
  items: DemoCatalogEntry[];
  next_cursor: string | null;
};

/** Exact body accepted by POST /api/demos/start. */
export type StartDemoInput = {
  command_id: string;
  demo_id: string;
  demo_version: string;
  source_mode: DemoMode;
  model_profile_id: string | null;
};

export type StartDemoReceipt = {
  contract_version: "demo-start-v1";
  command_id: string;
  demo_id: string;
  demo_version: string;
  source_mode: DemoMode;
  catalog_revision: string;
  manifest_sha256: string;
  status: "ready";
  project_id: string;
  project_owner_id: string;
  conversation_id: string;
  task_id: string;
  work_route: string;
  source_provenance: {
    kind: DemoMode;
    live_inference_occurred: boolean;
    review_status: string | null;
    source_asset_id: string | null;
    source_asset_sha256: string | null;
  };
  replayed: boolean;
  retry_safe: boolean;
  detail: string | null;
};

export type DemoLiveModel = Pick<
  RegistryModelProfile,
  "id" | "display_name" | "remote_model_id" | "provider_id" | "revision" | "pricing"
>;

export interface DemoOnboardingService {
  catalog(signal?: AbortSignal): Promise<DemoCatalog>;
  compatibleLiveModels(signal?: AbortSignal): Promise<DemoLiveModel[]>;
  start(input: StartDemoInput): Promise<StartDemoReceipt>;
  receipt(commandId: string, signal?: AbortSignal): Promise<StartDemoReceipt | null>;
}

/** A server-confirmed rejection: no Demo task was created and recovery is unnecessary. */
export class DemoStartRejectedError extends Error {
  constructor(message: string, readonly code: string | null = null) {
    super(message);
    this.name = "DemoStartRejectedError";
  }
}

export type PendingDemoStart = StartDemoInput & {
  catalog_revision: string;
  manifest_sha256: string;
  state: "pending" | "unknown" | "confirmed";
};

export function pendingDemoStorageKey(workspaceId: string): string {
  if (!workspaceId) throw new Error("工作区身份尚未读取，不能保存示例启动命令");
  return `annotagent.demo.pending.v1.${encodeURIComponent(workspaceId)}`;
}

type NewDemoStart = Omit<PendingDemoStart, "command_id" | "state">;

function inputSignature(input: NewDemoStart): string {
  return JSON.stringify([
    input.demo_id,
    input.demo_version,
    input.source_mode,
    input.model_profile_id,
    input.catalog_revision,
    input.manifest_sha256,
  ]);
}

export function readPendingDemo(storage: Pick<Storage, "getItem">, workspaceId: string): PendingDemoStart | null {
  try {
    const value = JSON.parse(storage.getItem(pendingDemoStorageKey(workspaceId)) || "null") as PendingDemoStart | null;
    if (!value || typeof value.command_id !== "string" || !value.command_id) return null;
    if (!["preset_candidates", "live_model"].includes(value.source_mode)) return null;
    if (!["pending", "unknown", "confirmed"].includes(value.state)) return null;
    if (value.source_mode === "preset_candidates" && value.model_profile_id !== null) return null;
    if (value.source_mode === "live_model" && !value.model_profile_id) return null;
    if (!value.catalog_revision || !value.manifest_sha256) return null;
    return value;
  } catch {
    return null;
  }
}

export function beginDemoStart(
  storage: Pick<Storage, "getItem" | "setItem">,
  workspaceId: string,
  input: NewDemoStart,
  commandId: string = crypto.randomUUID(),
): PendingDemoStart {
  const current = readPendingDemo(storage, workspaceId);
  if (current && inputSignature(current) === inputSignature(input) && current.state !== "confirmed") {
    return current;
  }
  const next: PendingDemoStart = { ...input, command_id: commandId, state: "pending" };
  storage.setItem(pendingDemoStorageKey(workspaceId), JSON.stringify(next));
  return next;
}

export function startDemoCommand(input: PendingDemoStart): StartDemoInput {
  return {
    command_id: input.command_id,
    demo_id: input.demo_id,
    demo_version: input.demo_version,
    source_mode: input.source_mode,
    model_profile_id: input.model_profile_id,
  };
}

export function updatePendingDemo(
  storage: Pick<Storage, "setItem">,
  workspaceId: string,
  input: PendingDemoStart,
  state: PendingDemoStart["state"],
): PendingDemoStart {
  const next = { ...input, state };
  storage.setItem(pendingDemoStorageKey(workspaceId), JSON.stringify(next));
  return next;
}

export function clearPendingDemo(storage: Pick<Storage, "removeItem">, workspaceId: string): void {
  storage.removeItem(pendingDemoStorageKey(workspaceId));
}

export function validateDemoReceipt(input: PendingDemoStart, receipt: StartDemoReceipt): StartDemoReceipt {
  if (receipt.contract_version !== "demo-start-v1") throw new Error("服务器返回了不支持的示例启动回执");
  if (
    receipt.command_id !== input.command_id ||
    receipt.demo_id !== input.demo_id ||
    receipt.demo_version !== input.demo_version ||
    receipt.source_mode !== input.source_mode ||
    receipt.catalog_revision !== input.catalog_revision ||
    receipt.manifest_sha256 !== input.manifest_sha256
  ) throw new Error("示例启动回执与已确认范围不匹配");
  if (!receipt.project_id || !receipt.project_owner_id || !receipt.conversation_id || !receipt.task_id)
    throw new Error("示例启动回执缺少 Project、Conversation 或 Task 身份");
  if (receipt.source_provenance.kind !== input.source_mode)
    throw new Error("示例来源回执与已确认模式不匹配");
  if (input.source_mode === "preset_candidates" && receipt.source_provenance.live_inference_occurred)
    throw new Error("预置候选回执错误地声明了实时模型调用");
  return receipt;
}

export function visibleDemoEntries(catalog: DemoCatalog): DemoCatalogEntry[] {
  if (catalog.contract_version !== "demo-catalog-v1") throw new Error("服务器返回了不支持的示例目录");
  if (!catalog.catalog_revision) throw new Error("服务器示例目录缺少不可变 revision");
  return catalog.items.filter((item) => item.image_count > 0 && item.modes.length > 0).slice(0, 2);
}
