import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";
import {
  beginDemoStart,
  clearPendingDemo,
  readPendingDemo,
  updatePendingDemo,
  validateDemoReceipt,
  visibleDemoEntries,
  type DemoCatalogEntry,
  type DemoMode,
  type DemoModeAvailability,
  type DemoOnboardingService,
  type PendingDemoStart,
  type StartDemoReceipt,
} from "./demoOnboardingService";

function modeFor(item: DemoCatalogEntry, mode: DemoMode): DemoModeAvailability | undefined {
  return item.modes.find((candidate) => candidate.mode === mode);
}

function costLabel(mode: DemoModeAvailability): string {
  if (mode.mode === "preset_candidates") return "本次不调用模型，不产生模型 Token";
  if (mode.maximum_cost === null) return "费用未知；启动前仍受服务器授权范围约束";
  return `费用上限 ${mode.maximum_cost} ${mode.currency || "（币种未提供）"}`;
}

export function DemoCard({
  item,
  busy,
  onStart,
}: {
  item: DemoCatalogEntry;
  busy: boolean;
  onStart: (item: DemoCatalogEntry, mode: DemoMode) => void;
}) {
  const preset = modeFor(item, "preset_candidates");
  const live = modeFor(item, "live_model");
  const liveReady = live?.status === "ready";
  const primary = liveReady ? live : preset?.status === "ready" ? preset : live;
  const secondary = primary?.mode === "live_model" ? preset : live;
  const actionLabel = (mode: DemoModeAvailability) => mode.mode === "preset_candidates"
    ? "免配置体验：预置候选"
    : mode.status === "setup_required" ? "配置模型并试跑" : "用我的模型试跑";
  return <article className="demo-card">
    <img src={item.thumbnail_url} alt={item.thumbnail_alt} />
    <div className="demo-card-copy">
      <small>引导式示例 · {item.image_count} 张图片</small>
      <h2>{item.title}</h2>
      <p>{item.description}</p>
      <p><strong>{item.labels.join(" / ")}</strong> · YOLO 检测训练包</p>
      <small>{item.license_summary}</small>
      {primary && <div className="demo-mode-summary">
        <span>{primary.mode === "preset_candidates" ? "预置候选，无本次模型推理" : `${primary.model_name || "已配置模型"}${primary.provider_name ? ` · ${primary.provider_name}` : ""}`}</span>
        <span>{primary.destination}</span>
        <span>{costLabel(primary)}</span>
      </div>}
      <div className="demo-actions">
        {primary && <button className="primary" disabled={busy || primary.status === "unavailable"} onClick={() => onStart(item, primary.mode)}>
          <Icon name={primary.mode === "live_model" ? "play" : "image"} size={16}/>{actionLabel(primary)}
        </button>}
        {secondary && secondary.status !== "unavailable" && <button disabled={busy} onClick={() => onStart(item, secondary.mode)}>{actionLabel(secondary)}</button>}
      </div>
      {primary?.status === "unavailable" && primary.reason && <p role="status">{primary.reason}</p>}
    </div>
  </article>;
}

export function DemoOnboarding({
  service,
  workspaceId,
  storage = localStorage,
  onStarted,
}: {
  service: DemoOnboardingService;
  workspaceId:string;
  storage?: Storage;
  onStarted: (receipt: StartDemoReceipt) => Promise<void> | void;
}) {
  const [catalog, setCatalog] = useState<DemoCatalogEntry[]>();
  const [error, setError] = useState("");
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const initialPending = useRef(readPendingDemo(storage,workspaceId));
  const [pending, setPending] = useState(initialPending.current);

  const acceptReceipt = async (request: PendingDemoStart, value: StartDemoReceipt) => {
    const receipt = validateDemoReceipt(request, value);
    if (receipt.status === "failed") {
      const saved=updatePendingDemo(storage, workspaceId, request, receipt.retry_safe ? "confirmed" : "unknown");
      setPending(saved);
      setError(receipt.detail || "示例初始化失败；服务器已保留可核实的回执");
      return;
    }
    setPending(updatePendingDemo(storage, workspaceId, request, "confirmed"));
    setStatus(receipt.status === "model_setup_required" ? "示例任务已创建，正在打开同一任务的模型设置。" : "示例任务已创建，正在打开。");
    await onStarted(receipt);
  };

  useEffect(() => {
    const controller = new AbortController();
    void service.catalog(controller.signal)
      .then((value) => { if (!controller.signal.aborted) setCatalog(visibleDemoEntries(value)); })
      .catch((cause) => { if (!controller.signal.aborted) setError(`无法读取示例目录：${cause instanceof Error ? cause.message : String(cause)}`); });
    const recovering=initialPending.current;
    if (recovering && recovering.state !== "confirmed") {
      setStatus("正在核实上次示例启动回执；不会重新提交或调用模型。 ");
      void service.receipt(recovering.command_id, controller.signal)
        .then((receipt) => {
          if (controller.signal.aborted) return;
          if (!receipt) { setStatus("没有找到已确认回执。只有再次点击原操作才会重试同一命令。 "); return; }
          return acceptReceipt(recovering, receipt);
        })
        .catch((cause) => { if (!controller.signal.aborted) setError(`启动结果尚未核实：${cause instanceof Error ? cause.message : String(cause)}`); });
    }
    return () => controller.abort();
  }, [service]);

  const start = async (item: DemoCatalogEntry, mode: DemoMode) => {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    setError("");
    setStatus("正在创建独立的 Demo Project 与 Task…");
    const request = beginDemoStart(storage, workspaceId, {
      demo_id: item.id,
      demo_version: item.version,
      catalog_digest: item.catalog_digest,
      mode,
      confirmed_scope: true,
    });
    setPending(request);
    try {
      const receipt = await service.start(request);
      await acceptReceipt(request, receipt);
    } catch (cause) {
      setPending(updatePendingDemo(storage, workspaceId, request, "unknown"));
      setError(`${cause instanceof Error ? cause.message : String(cause)}。刷新只会查询同一命令的服务端回执，不会自动重试收费请求。`);
    } finally {
      setBusy(false);
      lock.current = false;
    }
  };

  return <section className="demo-onboarding" aria-label="用示例试试看">
    <header><small>第一次使用</small><h2>用示例试试看</h2><p>创建独立示例项目，不会向现有项目追加图片或标签。</p></header>
    {!catalog && !error && <p role="status">读取服务器示例目录…</p>}
    {error && <div className="error" role="alert">{error}<button onClick={() => setError("")}>关闭</button></div>}
    {status && <p role="status">{status}</p>}
    {catalog?.length === 0 && <p>服务器当前没有可用且许可完整的示例包。</p>}
    <div className="demo-grid">{catalog?.map((item) => <DemoCard key={`${item.id}@${item.version}`} item={item} busy={busy} onStart={(entry, mode) => void start(entry, mode)}/>)}</div>
    {pending?.state === "confirmed" && <button className="subtle" onClick={() => { clearPendingDemo(storage,workspaceId); setPending(null); setStatus("可以显式创建一个新的示例任务。"); }}>再试一次（创建新任务）</button>}
  </section>;
}
