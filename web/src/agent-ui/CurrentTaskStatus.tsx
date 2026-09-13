import { useState, type ReactNode } from "react";
import type { Approval, SchemaClarificationChoice } from "./adapter";
import type {
  CurrentTaskAction,
  CurrentTaskPresentation,
} from "./currentTaskPresentation";
import { Disclosure } from "./Disclosure";
import { Icon } from "./Icon";
import { TaskModelBindings } from "./TaskModelBindings";

const actionLabel: Record<CurrentTaskAction["kind"], string> = {
  prepare_sample: "生成方案并测试样例",
  confirm_approval: "确认当前范围",
  stop: "停止当前操作",
  resume: "继续任务",
  open_review: "重新定位当前结果",
  prepare_processing: "确认范围并处理剩余图片",
  prepare_export: "生成训练数据包",
  download_package: "下载训练数据包",
};

const diagnosticActionLabel:Record<string,string>={
  open_model_setup:"查看缺失权重的模型",
  inspect_capability_setup:"检查模型能力要求",
  fix_configuration_and_authorize_new_attempt:"检查请求配置",
  inspect_request_failure_before_new_authorization:"查看未发送请求记录",
  inspect_receipt_and_resolve_unknown:"核实远端请求回执",
  inspect_receipt_before_new_authorization:"查看无效响应记录",
  inspect_empty_result:"查看空结果证据",
  inspect_saved_artifact:"查看投影失败证据",
  inspect_task_authorization_budget:"查看任务授权与预算",
};

export function CurrentTaskStatus({
  presentation,
  approval,
  busy,
  onPrimary,
  onEditTask,
  onClarificationChoice,
  details,
}: {
  presentation: CurrentTaskPresentation;
  approval?: Approval;
  busy: boolean;
  onPrimary: (action: CurrentTaskAction) => void;
  onEditTask: () => void;
  onClarificationChoice?: (choice: SchemaClarificationChoice["value"]) => void;
  details?: ReactNode;
}) {
  const [detailsRead, setDetailsRead] = useState(false);
  const primary = presentation.primary;
  const downloadable =
    primary?.kind === "download_package" && primary.url ? primary.url : undefined;
  const primaryLabel = presentation.action?.id === "test_pipeline_samples"
    ? "继续测试已保存方案"
    : primary ? actionLabel[primary.kind] : "";

  return (
    <section
      className={`current-task current-task-${presentation.kind}`}
      aria-label="当前任务状态"
      aria-live={presentation.kind === "running" ? "polite" : undefined}
    >
      <div className="current-task-copy">
        <small>{presentation.kind === "needs_review" ? "需要你的判断" : "当前任务"}</small>
        <strong>{presentation.title}</strong>
        <p>{presentation.detail}</p>
      </div>

      {presentation.modelBindings && <TaskModelBindings summary={presentation.modelBindings} />}

      {approval && presentation.kind !== "running" && (
        <Disclosure title="查看图片、模型、预算和有效期">
          <ul className="current-task-scope">
            {approval.scope.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
          <p>
            费用：{approval.budget ?? "未知；确认前不会把未知写成 0"}
          </p>
          <small>绑定范围：{approval.revision}</small>
        </Disclosure>
      )}

      <div className="current-task-actions">
        {presentation.diagnostic && <a className="diagnostic-action" href={presentation.diagnostic.safe_action.url} target="_blank" rel="noreferrer">
          <Icon name="arrow-right" size={16}/>{diagnosticActionLabel[presentation.diagnostic.safe_action.id]||"打开只读诊断记录"}
        </a>}
        {presentation.clarification && (
          <div className="current-task-choices" aria-label="选择标注输出类型">
            {presentation.clarification.choices.map((choice) => (
              <div className="current-task-choice" key={choice.value}>
                <button
                  className={choice.supported ? "primary" : undefined}
                  type="button"
                  disabled={busy || !choice.supported || !onClarificationChoice}
                  onClick={() => onClarificationChoice?.(choice.value)}
                >
                  {choice.label}
                </button>
                {!choice.supported && choice.unsupportedReason && (
                  <small>{choice.unsupportedReason}</small>
                )}
              </div>
            ))}
          </div>
        )}
        {presentation.kind === "needs_information" && !presentation.clarification && (
          <button type="button" onClick={onEditTask}>
            <Icon name="edit" size={16} />编辑任务信息
          </button>
        )}
        {downloadable ? (
          <a className="primary" href={downloadable} download>
            <Icon name="download" size={16} />{actionLabel.download_package}
          </a>
        ) : (
          primary && (
            <button
              className={
                primary.kind === "stop" || primary.kind === "open_review"
                  ? undefined
                  : "primary"
              }
              type="button"
              disabled={busy && primary.kind !== "stop"}
              onClick={() => onPrimary(primary)}
            >
              <Icon
                name={
                  primary.kind === "stop"
                    ? "stop"
                    : primary.kind === "open_review"
                      ? "image"
                      : primary.kind === "download_package"
                        ? "download"
                        : "play"
                }
                size={16}
              />
              {busy && primary.kind !== "stop" ? "正在读取服务器状态…" : primaryLabel}
            </button>
          )
        )}
        {presentation.kind !== "needs_information" &&
          presentation.kind !== "running" &&
          presentation.kind !== "delivered" && (
            <button type="button" onClick={onEditTask}>
              编辑任务
            </button>
          )}
      </div>

      {details && (
        <Disclosure
          className="current-task-details"
          title="查看执行与任务详情"
          onToggle={(event) => {
            if (event.currentTarget.open) setDetailsRead(true);
          }}
        >
          {detailsRead && details}
        </Disclosure>
      )}
    </section>
  );
}
