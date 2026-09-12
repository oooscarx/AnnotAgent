import { useEffect, useRef, useState } from "react";
import { Disclosure } from "./Disclosure";
import "./task-usage.css";

export type TaskUsageState = "no_model_requests" | "complete" | "partial" | "unknown";
export type TaskUsageStatus = "reserved" | "running" | "succeeded" | "failed" | "in_doubt" | "cancelled";

export type TaskUsageAttempt = {
  attempt_id: string;
  call_id: string;
  attempt_number: number;
  kind: "task" | "probe";
  status: TaskUsageStatus;
  model_profile_id: string;
  model_profile_revision: number;
  model_name?: string | null;
  provider_id: string;
  provider_name: string;
  request_id?: string | null;
  input_tokens: number | null;
  cached_input_tokens: number | null;
  output_tokens: number | null;
  usage_source: string;
  cost: string | null;
  currency: string | null;
  pricing_snapshot: {
    input_per_million_tokens: string | null;
    output_per_million_tokens: string | null;
    cached_input_per_million_tokens: string | null;
    per_request: string | null;
    captured_at: string;
  } | null;
  started_at: string;
  completed_at: string | null;
  duration_ms: number | null;
  failure: string | null;
};

export type TaskUsagePage = {
  scope: { project_id: string; conversation_id?: string; task_id: string };
  state: TaskUsageState;
  summary: {
    attempt_count: number;
    known_cost: string | null;
    currency: string | null;
    currency_totals?: { currency: string; cost: string }[];
    input_tokens: number | null;
    cached_input_tokens: number | null;
    output_tokens: number | null;
    unknown_attempt_count: number;
  };
  attempts: { items: TaskUsageAttempt[]; next_cursor: string | null };
};

export type TaskUsageService = {
  getTaskUsage(
    projectId: string,
    taskId: string,
    cursor?: string,
    signal?: AbortSignal,
  ): Promise<TaskUsagePage>;
  subscribeTaskUsage?(
    projectId: string,
    taskId: string,
    onChange: () => void,
  ): () => void;
};

const stateLabels: Record<TaskUsageState, string> = {
  no_model_requests: "无本次模型请求",
  complete: "用量记录完整",
  partial: "部分用量未知",
  unknown: "模型请求结果或用量未知",
};

const statusLabels: Record<TaskUsageStatus, string> = {
  reserved: "已预留",
  running: "请求中",
  succeeded: "成功",
  failed: "失败",
  in_doubt: "结果未知",
  cancelled: "已取消",
};

function tokenValue(value: number | null) {
  return value == null ? "未知" : value.toLocaleString("zh-CN");
}

function attemptCost(attempt: TaskUsageAttempt) {
  return attempt.cost == null || attempt.currency == null
    ? "费用未知（不是 0）"
    : `${attempt.currency} ${attempt.cost}`;
}

function UsageAttempt({ attempt }: { attempt: TaskUsageAttempt }) {
  return (
    <article className="task-usage-attempt">
      <div className="task-usage-attempt-heading">
        <div>
          <strong>{attempt.model_name || attempt.model_profile_id}</strong>
          <p>{attempt.provider_name} · 第 {attempt.attempt_number} 次物理请求</p>
        </div>
        <span data-status={attempt.status}>{statusLabels[attempt.status]}</span>
      </div>
      <dl>
        <div><dt>输入</dt><dd>{tokenValue(attempt.input_tokens)} tokens</dd></div>
        <div><dt>缓存输入</dt><dd>{tokenValue(attempt.cached_input_tokens)} tokens</dd></div>
        <div><dt>输出</dt><dd>{tokenValue(attempt.output_tokens)} tokens</dd></div>
        <div><dt>费用</dt><dd>{attemptCost(attempt)}</dd></div>
      </dl>
      <p>用量来源：{attempt.usage_source} · Model Profile r{attempt.model_profile_revision}</p>
      <p>
        {attempt.pricing_snapshot
          ? `价格快照：${attempt.pricing_snapshot.captured_at}`
          : "价格快照未知；不会按当前价格重算历史"}
        {attempt.duration_ms == null ? " · 耗时未知" : ` · ${attempt.duration_ms} ms`}
      </p>
      {attempt.failure && <p role="alert">{attempt.failure}</p>}
      <Disclosure title="请求与价格证据">
        <pre>{JSON.stringify({
          attempt_id: attempt.attempt_id,
          call_id: attempt.call_id,
          request_id: attempt.request_id,
          provider_id: attempt.provider_id,
          model_profile_id: attempt.model_profile_id,
          model_profile_revision: attempt.model_profile_revision,
          pricing_snapshot: attempt.pricing_snapshot,
          started_at: attempt.started_at,
          completed_at: attempt.completed_at,
        }, null, 2)}</pre>
      </Disclosure>
    </article>
  );
}

export function TaskUsageView({ value, compact = false }: { value: TaskUsagePage; compact?: boolean }) {
  const taskAttempts = value.attempts.items.filter((attempt) => attempt.kind === "task");
  const probes = value.attempts.items.filter((attempt) => attempt.kind === "probe");
  const currencyTotals = value.summary.currency_totals ?? (
    value.summary.known_cost != null && value.summary.currency != null
      ? [{ currency: value.summary.currency, cost: value.summary.known_cost }]
      : []
  );
  return (
    <div className="task-usage-view" data-state={value.state}>
      <div className="task-usage-summary">
        <div><strong>{stateLabels[value.state]}</strong><p>{value.summary.attempt_count} 次请求尝试</p></div>
        {value.state !== "no_model_requests" && (
          <div>
            <p>输入 {tokenValue(value.summary.input_tokens)} · 输出 {tokenValue(value.summary.output_tokens)}</p>
            {currencyTotals.length > 0
              ? currencyTotals.map((item) => <p key={item.currency}>{item.currency} {item.cost}</p>)
              : <p>总费用未知（不是 0）</p>}
          </div>
        )}
      </div>
      {value.state === "no_model_requests" && <p>此 Task 尚未产生模型请求；Preset 候选不会伪造 Token 或费用。</p>}
      {value.summary.unknown_attempt_count > 0 && <p role="status">{value.summary.unknown_attempt_count} 次尝试缺少完整 Token、价格或终态；未按 0 处理。</p>}
      {!compact && taskAttempts.map((attempt) => <UsageAttempt key={attempt.attempt_id} attempt={attempt} />)}
      {compact && taskAttempts.length > 0 && <Disclosure title={`查看 ${taskAttempts.length} 次任务请求`}><div>{taskAttempts.map((attempt) => <UsageAttempt key={attempt.attempt_id} attempt={attempt} />)}</div></Disclosure>}
      {probes.length > 0 && <Disclosure title={`独立的模型探测记录 · ${probes.length}`}><p>探测不是普通 Task 推理，不计入上方任务尝试。</p>{probes.map((attempt) => <UsageAttempt key={attempt.attempt_id} attempt={attempt} />)}</Disclosure>}
    </div>
  );
}

export function TaskUsage({
  projectId,
  taskId,
  service,
  compact = false,
}: {
  projectId: string;
  taskId: string;
  service: TaskUsageService;
  compact?: boolean;
}) {
  const [pages, setPages] = useState<TaskUsagePage[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [generation, setGeneration] = useState(0);
  const live = useRef(0);

  useEffect(() => service.subscribeTaskUsage?.(projectId, taskId, () => setGeneration((value) => value + 1)), [projectId, taskId, service]);
  useEffect(() => {
    const current = ++live.current;
    const controller = new AbortController();
    setLoading(true);
    setError("");
    void service.getTaskUsage(projectId, taskId, undefined, controller.signal)
      .then((value) => {
        if (current !== live.current) return;
        if (value.scope.project_id !== projectId || value.scope.task_id !== taskId)
          throw new Error("用量记录不属于当前 Project 与 Task");
        setPages([value]);
      })
      .catch((reason) => {
        if (current === live.current && !controller.signal.aborted) setError((reason as Error).message);
      })
      .finally(() => {
        if (current === live.current) setLoading(false);
      });
    return () => {
      controller.abort();
      live.current += 1;
    };
  }, [projectId, taskId, service, generation]);

  const latest = pages[pages.length - 1];
  const combined = latest && pages.length > 1
    ? { ...latest, attempts: { ...latest.attempts, items: pages.flatMap((page) => page.attempts.items) } }
    : latest;
  const loadMore = async () => {
    if (!latest?.attempts.next_cursor || loading) return;
    setLoading(true);
    setError("");
    try {
      const value = await service.getTaskUsage(projectId, taskId, latest.attempts.next_cursor);
      if (value.scope.project_id !== projectId || value.scope.task_id !== taskId)
        throw new Error("下一页用量记录不属于当前 Project 与 Task");
      setPages((current) => [...current, value]);
    } catch (reason) {
      setError((reason as Error).message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <section className="task-usage" aria-label="本次任务模型用量">
      <div className="task-usage-title"><h3>本次模型用量</h3><button disabled={loading} onClick={() => setGeneration((value) => value + 1)}>刷新</button></div>
      {loading && !combined && <p role="status">读取本次 Task 的模型请求…</p>}
      {error && <p role="alert">{error}。未把缺失记录显示为零。</p>}
      {combined && <TaskUsageView value={combined} compact={compact} />}
      {latest?.attempts.next_cursor && <button disabled={loading} onClick={() => void loadMore()}>{loading ? "读取下一页…" : "加载更早请求"}</button>}
    </section>
  );
}
