import { useEffect, useState } from "react";
import type { Task } from "./adapter";
import type { ConversationCallReceipt } from "../types";

export const callStage = (stage?: string | null) => stage ? ({reserved:"已受理",provider_request:"模型请求处理中",response_received:"已收到响应，校验结构化结果",settled:"本地执行已结算"}[stage] || stage) : undefined;
export function failureDetail(failure: ConversationCallReceipt["failure"]) {
  if (!failure) return undefined;
  const category = ({configuration:"配置错误",cancelled:"请求被取消",timeout:"请求超时",connection:"连接失败",transport:"传输中断",http_status:"服务商返回错误",invalid_response:"返回内容无法解析",invalid_structured_output:"结构化结果不符合协议",provider_error:"模型接口错误",local_error:"本地处理失败",interrupted:"执行中断"} as Record<string,string>)[failure.category] || failure.category;
  const stage = ({prepare_request:"准备请求",provider_request:"发送或等待模型",response_body:"读取响应",response_decode:"解析响应",structured_output:"校验结构化结果",handler:"本地处理",recovery:"恢复检查"} as Record<string,string>)[failure.stage] || failure.stage;
  return category + " · " + stage + (failure.http_status ? " · HTTP " + failure.http_status : "");
}

export const executionStatus = (status: string) => ({
  reserved: "已受理，等待模型执行回执", running: "执行中", pending: "等待执行",
  queued: "已排队", cancelling: "正在停止", completed: "已完成", succeeded: "已完成",
  failed: "执行失败", invalid_result: "模型已返回，结果校验失败", in_doubt: "本地执行已结束 · 远端结果未知",
}[status] || status);

export function executionElapsed(start?: string, end?: string, duration?: number, now?: number) {
  if (duration !== undefined && Number.isFinite(duration) && duration >= 0) return `${(duration / 1000).toFixed(1)} 秒`;
  const from = Date.parse(start || ""), to = end ? Date.parse(end) : now;
  if (!Number.isFinite(from) || to === undefined || !Number.isFinite(to) || to < from) return "未记录";
  return `${Math.floor((to - from) / 1000)} 秒`;
}

export function ExecutionProgress({ receipts }: { receipts: NonNullable<Task["receipts"]> }) {
  const [now, setNow] = useState(Date.now);
  const current = [...receipts].reverse().find(r => ["reserved", "running", "queued", "pending", "cancelling"].includes(r.status)) || receipts.at(-1);
  const active = !!current && ["reserved", "running", "queued", "pending", "cancelling"].includes(current.status);
  useEffect(() => {
    if (!active || !current?.startedAt) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [active, current?.id, current?.startedAt]);
  if (!current) return null;
  return <section className="execution-progress" aria-label="当前执行状态">
    <p role="status">{current.title} · {executionStatus(current.status)}</p>
    {current.stage && <p>阶段：{current.stage}</p>}
    <small>耗时：{executionElapsed(current.startedAt, current.finishedAt, current.durationMs, active ? now : undefined)}{active && " · 等待服务端回执，不代表模型完成百分比"}</small>
    {active && current.title === "模型结构化决策" && <small>当前接口接收完整模型响应，尚不提供逐段文本；状态更新不是模型文字流。</small>}
    {current.detail && <p>{current.detail}</p>}
    {current.status === "in_doubt" && <p>当前没有此请求的活动执行。远端是否完成和实际费用尚无法确认；刷新不会重新调用模型。</p>}
    <details><summary>执行记录 · {receipts.length} 项</summary>{receipts.map(r => <details key={r.id}>
      <summary>{r.title} · {executionStatus(r.status)}</summary>
      <p>{r.detail || "已保存操作回执，未记录自然语言回复。"}</p>
      <small>开始：{r.startedAt || "未记录"} · 结束：{r.finishedAt || "未记录"}</small>
      <p>请求 ID：{r.id}</p>
    </details>)}</details>
  </section>;
}
