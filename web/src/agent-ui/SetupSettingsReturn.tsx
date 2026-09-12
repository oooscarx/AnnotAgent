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
  return (
    <aside className="setup-settings-return" aria-label="模型准备返回任务">
      <div>
        <strong>正在为原任务准备模型</strong>
        <p>
          Task {context.task_id.slice(0, 8)}
          {context.draft_id ? ` · Draft ${context.draft_id.slice(0, 8)}` : ""}。
          {` ${context.role}`}。返回后重新检查版本、兼容能力和授权；不会自动继续收费操作。
        </p>
      </div>
      <div className="actions">
        <a href={setupReturnPath(context, "cancelled")}>取消设置并返回</a>
        <a className="primary" href={setupReturnPath(context, "configured")}>配置完成，返回检查</a>
      </div>
    </aside>
  );
}
