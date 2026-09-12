import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";
import {
  beginDemoStart,
  clearPendingDemo,
  DemoStartRejectedError,
  readPendingDemo,
  startDemoCommand,
  updatePendingDemo,
  validateDemoReceipt,
  visibleDemoEntries,
  type DemoCatalogEntry,
  type DemoLiveModel,
  type DemoMode,
  type DemoModeAvailability,
  type DemoOnboardingService,
  type PendingDemoStart,
  type StartDemoReceipt,
} from "./demoOnboardingService";

function modeFor(item: DemoCatalogEntry, mode: DemoMode): DemoModeAvailability | undefined {
  return item.modes.find((candidate) => candidate.source_mode === mode);
}

function liveCostLabel(model: DemoLiveModel | undefined): string {
  if (!model) return "尚未选择兼容视觉模型";
  const price = model.pricing;
  const parts = [
    price.input_per_million_tokens ? `输入 ${price.input_per_million_tokens}` : null,
    price.output_per_million_tokens ? `输出 ${price.output_per_million_tokens}` : null,
    price.per_image ? `每图 ${price.per_image}` : null,
    price.per_request ? `每次 ${price.per_request}` : null,
  ].filter(Boolean);
  return parts.length ? `${parts.join(" · ")} ${price.currency}` : "此模型费用未知；执行前仍需确认实际授权范围";
}

export function DemoCard({
  item,
  catalogRevision,
  liveModels,
  selectedModel,
  busy,
  onSelectModel,
  onStart,
}: {
  item: DemoCatalogEntry;
  catalogRevision: string;
  liveModels: DemoLiveModel[];
  selectedModel: string;
  busy: boolean;
  onSelectModel: (model: string) => void;
  onStart: (item: DemoCatalogEntry, catalogRevision: string, mode: DemoMode, modelProfileId: string | null) => void;
}) {
  const preset = modeFor(item, "preset_candidates");
  const live = modeFor(item, "live_model");
  const model = liveModels.find((candidate) => candidate.id === selectedModel);
  return <article className="demo-card">
    <img src={item.thumbnail_url} alt="桌面上的杯子和瓶子示例" />
    <div className="demo-card-copy">
      <small>引导式示例 · {item.image_count} 张图片</small>
      <h2>{item.title}</h2>
      <p>{item.summary}</p>
      <p><strong>{item.labels.join(" / ")}</strong> · {item.delivery_format}</p>
      <small>{item.license.spdx_id} · 示例会创建独立 Project，不修改现有项目</small>
      {preset?.status === "ready" && <div className="demo-mode-summary">
        <span>预置候选体验</span>
        <span>不调用模型 · 结果仍需逐图人工审核</span>
        <span>不产生模型 Token，不代表实时模型准确率</span>
      </div>}
      {live && <div className="demo-live-model">
        <label>真实模型试跑
          <select aria-label="示例视觉模型" value={selectedModel} disabled={busy || !liveModels.length} onChange={(event) => onSelectModel(event.target.value)}>
            {!liveModels.length && <option value="">没有已就绪的兼容视觉模型</option>}
            {liveModels.map((candidate) => <option key={candidate.id} value={candidate.id}>{candidate.display_name} · {candidate.remote_model_id}</option>)}
          </select>
        </label>
        <small>{liveCostLabel(model)}</small>
      </div>}
      <div className="demo-actions">
        {preset?.status === "ready" && <button className="primary" disabled={busy} onClick={() => onStart(item, catalogRevision, "preset_candidates", null)}>
          <Icon name="image" size={16}/>体验预置候选
        </button>}
        {live && live.status !== "unavailable" && liveModels.length > 0 && <button disabled={busy || !selectedModel} onClick={() => onStart(item, catalogRevision, "live_model", selectedModel)}>
          <Icon name="play" size={16}/>用所选模型创建任务
        </button>}
        {live && liveModels.length === 0 && <a className="button" href="/settings/models?return_to=%2Fprojects">连接视觉模型</a>}
      </div>
      {live?.reason && liveModels.length === 0 && <p role="status">{live.reason}</p>}
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
  workspaceId: string;
  storage?: Storage;
  onStarted: (receipt: StartDemoReceipt) => Promise<void> | void;
}) {
  const [catalog, setCatalog] = useState<{ revision: string; items: DemoCatalogEntry[] }>();
  const [liveModels, setLiveModels] = useState<DemoLiveModel[]>([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [error, setError] = useState("");
  const [modelError, setModelError] = useState("");
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const initialPending = useRef(readPendingDemo(storage, workspaceId));
  const [pending, setPending] = useState(initialPending.current);

  const acceptReceipt = async (request: PendingDemoStart, value: StartDemoReceipt) => {
    const receipt = validateDemoReceipt(request, value);
    setPending(updatePendingDemo(storage, workspaceId, request, "confirmed"));
    setStatus(receipt.source_mode === "preset_candidates"
      ? "独立示例任务已创建；正在打开尚未人工接受的预置候选。"
      : "独立示例任务已创建；尚未调用模型，正在打开同一任务的有限范围批准。"
    );
    await onStarted(receipt);
  };

  useEffect(() => {
    const controller = new AbortController();
    void service.catalog(controller.signal)
      .then((value) => {
        if (controller.signal.aborted) return;
        setCatalog({ revision: value.catalog_revision, items: visibleDemoEntries(value) });
      })
      .catch((cause) => { if (!controller.signal.aborted) setError(`无法读取示例目录：${cause instanceof Error ? cause.message : String(cause)}`); });
    void service.compatibleLiveModels(controller.signal)
      .then((models) => {
        if (controller.signal.aborted) return;
        setLiveModels(models);
        setSelectedModel((current) => current && models.some((model) => model.id === current) ? current : models[0]?.id || "");
      })
      .catch((cause) => { if (!controller.signal.aborted) setModelError(`无法核实兼容视觉模型：${cause instanceof Error ? cause.message : String(cause)}`); });
    const recovering = initialPending.current;
    if (recovering && recovering.state !== "confirmed") {
      setStatus("正在核实上次示例启动回执；不会重新提交或调用模型。");
      void service.receipt(recovering.command_id, controller.signal)
        .then((receipt) => {
          if (controller.signal.aborted) return;
          if (!receipt) { setStatus("没有找到已确认回执。只有再次点击原操作才会重试同一命令。"); return; }
          return acceptReceipt(recovering, receipt);
        })
        .catch((cause) => { if (!controller.signal.aborted) setError(`启动结果尚未核实：${cause instanceof Error ? cause.message : String(cause)}`); });
    }
    return () => controller.abort();
  }, [service]);

  const start = async (item: DemoCatalogEntry, catalogRevision: string, mode: DemoMode, modelProfileId: string | null) => {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    setError("");
    setStatus("正在创建独立的 Demo Project 与 Task…");
    const pendingRequest = beginDemoStart(storage, workspaceId, {
      demo_id: item.demo_id,
      demo_version: item.version,
      source_mode: mode,
      model_profile_id: modelProfileId,
      catalog_revision: catalogRevision,
      manifest_sha256: item.manifest_sha256,
    });
    setPending(pendingRequest);
    try {
      const receipt = await service.start(startDemoCommand(pendingRequest));
      await acceptReceipt(pendingRequest, receipt);
    } catch (cause) {
      if (cause instanceof DemoStartRejectedError) {
        clearPendingDemo(storage, workspaceId);
        setPending(null);
        setError(`${cause.message}。服务器已拒绝本次创建；没有创建任务、调用模型或改用预置候选。`);
        return;
      }
      setPending(updatePendingDemo(storage, workspaceId, pendingRequest, "unknown"));
      setError(`${cause instanceof Error ? cause.message : String(cause)}。刷新只会查询同一命令的服务端回执，不会自动重试收费请求。`);
    } finally {
      setBusy(false);
      lock.current = false;
    }
  };

  return <section className="demo-onboarding" aria-label="用示例试试看">
    <header><small>第一次使用</small><h2>用示例试试看</h2><p>一次创建独立示例项目；预置候选和真实模型结果始终明确区分。</p></header>
    {!catalog && !error && <p role="status">读取服务器示例目录与兼容模型…</p>}
    {error && <div className="error" role="alert">{error}<button onClick={() => setError("")}>关闭</button></div>}
    {modelError && <p role="status">{modelError}。仍可使用明确标记的预置候选体验。</p>}
    {status && <p role="status">{status}</p>}
    {catalog?.items.length === 0 && <p>服务器当前没有可用且许可完整的示例包。</p>}
    <div className="demo-grid">{catalog?.items.map((item) => <DemoCard
      key={`${item.demo_id}@${item.version}`}
      item={item}
      catalogRevision={catalog.revision}
      liveModels={liveModels}
      selectedModel={selectedModel}
      busy={busy}
      onSelectModel={setSelectedModel}
      onStart={(entry, revision, mode, model) => void start(entry, revision, mode, model)}
    />)}</div>
    {pending?.state === "confirmed" && <button className="subtle" onClick={() => { clearPendingDemo(storage, workspaceId); setPending(null); setStatus("可以显式创建一个新的示例任务。"); }}>再试一次（创建新任务）</button>}
  </section>;
}
