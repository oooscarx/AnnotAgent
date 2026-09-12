import { useState, type ReactNode } from "react";
import type { Approval, SchemaClarificationChoice } from "./adapter";
import type {
  CurrentTaskAction,
  CurrentTaskPresentation,
} from "./currentTaskPresentation";
import { Disclosure } from "./Disclosure";
import { Icon } from "./Icon";

const actionLabel: Record<CurrentTaskAction["kind"], string> = {
  prepare_sample: "开始标注样例",
  confirm_approval: "确认当前范围",
  stop: "停止当前操作",
  resume: "继续任务",
  open_review: "重新定位当前结果",
  prepare_processing: "确认范围并处理剩余图片",
  prepare_export: "生成训练数据包",
  download_package: "下载训练数据包",
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
              disabled={busy}
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
              {busy ? "正在读取服务器状态…" : actionLabel[primary.kind]}
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
