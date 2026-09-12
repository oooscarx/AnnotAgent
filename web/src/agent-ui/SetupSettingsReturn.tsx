import { useMemo } from "react";
import {
  restoreSetupContext,
  setupReturnPath,
  type SetupContext,
} from "./modelPreparation";
import "./setup-request.css";

export function readSetupSettingsContext(
  storage: Storage,
  search = location.search,
): SetupContext | undefined {
  const id = new URLSearchParams(search).get("setup_request");
  return id ? restoreSetupContext(storage, id) : undefined;
}

export function readSetupCandidate(search = location.search) {
  return new URLSearchParams(search).get("candidate") ?? undefined;
}

export function SetupSettingsReturn({ search = location.search }: { search?: string }) {
  const state = useMemo(() => {
    try {
      return { context: readSetupSettingsContext(sessionStorage, search) };
    } catch (reason) {
      return { error: (reason as Error).message };
    }
  }, [search]);
  if (state.error)
    return <p role="alert">无法恢复模型准备任务：{state.error}</p>;
  const context = state.context;
  if (!context) return null;
  const candidate = readSetupCandidate(search);
  return (
    <aside className="setup-settings-return" aria-label="模型准备返回任务">
      <div>
        <strong>完成这里的必要配置后返回原任务</strong>
        <p>图片、需求和当前任务都已保留。返回时会重新检查模型版本和原授权，不会自动发起收费请求。</p>
        {candidate && <p>已从任务带入一个兼容候选；详细身份可在当前编辑区查看。</p>}
      </div>
      <div className="actions">
        <a href={setupReturnPath(context, "cancelled")}>取消并返回</a>
        <a className="primary" href={setupReturnPath(context, "configured")}>保存完成，返回任务</a>
      </div>
    </aside>
  );
}
