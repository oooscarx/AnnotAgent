import { request } from "../api";

export const historyPolicy = "exclude_existing_history_v1" as const;
export interface HistoryScope {
  id: string;
  revision: number;
  established_at: string;
  policy: typeof historyPolicy;
  establishment_command_id: string;
}
export interface HistoryPreview {
  policy: typeof historyPolicy;
  expected_snapshot_hash: string;
  excluded_counts: Record<string, number>;
  preserves_published_versions: boolean;
  preserves_annotations: boolean;
  preserves_direct_references: boolean;
}
export interface HistoryCommand {
  command_id: string;
  expected_scope_revision: null;
  expected_snapshot_hash: string;
  policy: typeof historyPolicy;
  confirmed: true;
}
export const historyScopeApi = {
  read: (signal?: AbortSignal) => request<{scope: HistoryScope | null}>("/api/history-scope", {signal}),
  preview: () => request<HistoryPreview>("/api/history-scope/preview", {method:"POST",body:JSON.stringify({policy:historyPolicy})}),
  establish: (command: HistoryCommand) => request<{scope:HistoryScope}>("/api/history-scope", {method:"POST",body:JSON.stringify(command)}),
};

export function validateHistoryScope(scope: HistoryScope): HistoryScope {
  if (!scope.id || scope.revision !== 1 || scope.policy !== historyPolicy || !scope.establishment_command_id) throw new Error("服务器历史范围不符合契约；不会读取未隔离的历史。");
  return scope;
}
export function historyCommand(preview: HistoryPreview, commandId: string): HistoryCommand {
  if (preview.policy !== historyPolicy || !preview.expected_snapshot_hash || !preview.preserves_published_versions || !preview.preserves_annotations || !preview.preserves_direct_references) throw new Error("历史预览未确认数据保护要求。");
  return {command_id:commandId,expected_scope_revision:null,expected_snapshot_hash:preview.expected_snapshot_hash,policy:historyPolicy,confirmed:true};
}

/** Explicit scope is mandatory for native history lists. Never fall back unscoped. */
export function historyQuery(scope: HistoryScope, offset=0) {
  validateHistoryScope(scope);
  if (!Number.isSafeInteger(offset) || offset < 0) throw new Error("无效的历史分页位置");
  return new URLSearchParams({history_scope:scope.id,limit:"50",offset:String(offset)});
}
