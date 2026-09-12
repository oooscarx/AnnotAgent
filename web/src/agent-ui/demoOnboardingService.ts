export type DemoMode = "preset_candidates" | "live_model";

export type DemoModeAvailability = {
  mode: DemoMode;
  status: "ready" | "setup_required" | "unavailable";
  reason: string | null;
  model_name: string | null;
  provider_name: string | null;
  destination: string;
  maximum_model_calls: number | null;
  maximum_cost: string | null;
  currency: string | null;
};

export type DemoCatalogEntry = {
  id: string;
  version: string;
  catalog_digest: string;
  title: string;
  description: string;
  image_count: number;
  labels: string[];
  delivery_format: "yolo_detection";
  thumbnail_url: string;
  thumbnail_alt: string;
  license_summary: string;
  modes: DemoModeAvailability[];
};

export type DemoCatalog = {
  contract_version: "demo-catalog-v1";
  items: DemoCatalogEntry[];
};

export type StartDemoInput = {
  command_id: string;
  demo_id: string;
  demo_version: string;
  catalog_digest: string;
  mode: DemoMode;
  confirmed_scope: true;
};

export type StartDemoReceipt = {
  contract_version: "demo-start-v1";
  command_id: string;
  demo_id: string;
  demo_version: string;
  mode: DemoMode;
  status: "preparing" | "ready" | "model_setup_required" | "failed";
  project_id: string | null;
  conversation_id: string | null;
  task_id: string | null;
  replayed: boolean;
  retry_safe: boolean;
  detail: string | null;
};

export interface DemoOnboardingService {
  catalog(signal?: AbortSignal): Promise<DemoCatalog>;
  start(input: StartDemoInput): Promise<StartDemoReceipt>;
  receipt(commandId: string, signal?: AbortSignal): Promise<StartDemoReceipt | null>;
}

export type PendingDemoStart = StartDemoInput & {
  state: "pending" | "unknown" | "confirmed";
};

export function pendingDemoStorageKey(workspaceId:string):string {
  if(!workspaceId)throw new Error("工作区身份尚未读取，不能保存示例启动命令");
  return `annotagent.demo.pending.v1.${encodeURIComponent(workspaceId)}`;
}

function inputSignature(input: Omit<StartDemoInput, "command_id">): string {
  return JSON.stringify([
    input.demo_id,
    input.demo_version,
    input.catalog_digest,
    input.mode,
    input.confirmed_scope,
  ]);
}

export function readPendingDemo(storage: Pick<Storage, "getItem">,workspaceId:string): PendingDemoStart | null {
  try {
    const value = JSON.parse(storage.getItem(pendingDemoStorageKey(workspaceId)) || "null") as PendingDemoStart | null;
    if (!value || typeof value.command_id !== "string" || !value.command_id) return null;
    if (!["preset_candidates", "live_model"].includes(value.mode)) return null;
    if (!["pending", "unknown", "confirmed"].includes(value.state)) return null;
    if (value.confirmed_scope !== true) return null;
    return value;
  } catch {
    return null;
  }
}

export function beginDemoStart(
  storage: Pick<Storage, "getItem" | "setItem">,
  workspaceId:string,
  input: Omit<StartDemoInput, "command_id">,
  commandId: string = crypto.randomUUID(),
): PendingDemoStart {
  const current = readPendingDemo(storage,workspaceId);
  if (current && inputSignature(current) === inputSignature(input) && current.state !== "confirmed") {
    return current;
  }
  const next: PendingDemoStart = { ...input, command_id: commandId, state: "pending" };
  storage.setItem(pendingDemoStorageKey(workspaceId), JSON.stringify(next));
  return next;
}

export function updatePendingDemo(
  storage: Pick<Storage, "setItem">,
  workspaceId:string,
  input: PendingDemoStart,
  state: PendingDemoStart["state"],
): PendingDemoStart {
  const next = { ...input, state };
  storage.setItem(pendingDemoStorageKey(workspaceId), JSON.stringify(next));
  return next;
}

export function clearPendingDemo(storage: Pick<Storage, "removeItem">,workspaceId:string): void {
  storage.removeItem(pendingDemoStorageKey(workspaceId));
}

export function validateDemoReceipt(input: StartDemoInput, receipt: StartDemoReceipt): StartDemoReceipt {
  if (receipt.contract_version !== "demo-start-v1") throw new Error("服务器返回了不支持的示例启动回执");
  if (
    receipt.command_id !== input.command_id ||
    receipt.demo_id !== input.demo_id ||
    receipt.demo_version !== input.demo_version ||
    receipt.mode !== input.mode
  ) throw new Error("示例启动回执与已确认范围不匹配");
  const entersTask = ["preparing", "ready", "model_setup_required"].includes(receipt.status);
  if (entersTask && (!receipt.project_id || !receipt.task_id)) throw new Error("示例启动回执缺少 Project 或 Task 身份");
  return receipt;
}

export function visibleDemoEntries(catalog: DemoCatalog): DemoCatalogEntry[] {
  if (catalog.contract_version !== "demo-catalog-v1") throw new Error("服务器返回了不支持的示例目录");
  return catalog.items.filter((item) => item.image_count > 0 && item.modes.length > 0).slice(0, 2);
}
