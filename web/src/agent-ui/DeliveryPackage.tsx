import { useEffect, useRef, useState } from "react";
import { Disclosure } from "./Disclosure";
import { formalReviewCountsComplete } from "./deliveryReviewState";
import { demoPackageEvidenceError } from "./demoDeliveryPresentation";
import type {
  DeliveryPackageConsent,
  DeliveryPackageRead,
  DeliveryPackageReadiness,
  DeliveryService,
} from "./deliveryService";

type Scope = { revision: number; content_sha256: string; image_ids: string[] };
type Props = {
  service: DeliveryService;
  project: string;
  task: string;
  scope: Scope;
  locked?: boolean;
  onInspect: (id: string) => void;
  initialPackageId?:string;
  onReady?:(receipt:DeliveryPackageRead)=>void;
  onDownload?:(packageId:string)=>void;
  expectedDemo?:import("./deliveryService").DemoDeliveryPanelRead["demo"];
};

const phases = {
  preparing: "准备中",
  exporting: "正在打包原图和标签",
  validating: "正在独立校验",
  ready: "数据集已打包",
  failed: "打包失败",
  cancelled: "已取消",
};

/**
 * Readiness and automatic admission belong to the server. React can authorize
 * the frozen intent, cancel that authorization, and display persisted receipts.
 * It never infers readiness by scanning every image and never POSTs on ready.
 */
export function DeliveryPackage({ service, project, task, scope, locked = false, onInspect, initialPackageId, onReady, onDownload, expectedDemo }: Props) {
  const [history, setHistory] = useState<{ id: string; created_at: string }[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [id, setId] = useState(() => initialPackageId || new URL(location.href).searchParams.get("delivery_package") || "");
  const [job, setJob] = useState<DeliveryPackageRead>();
  const [readiness, setReadiness] = useState<DeliveryPackageReadiness>();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const pending = useRef(false);
  const authorizationRetry = useRef<DeliveryPackageConsent["input"] | undefined>(undefined);
  const readyNotification = useRef<string | undefined>(undefined);

  const select = (next: string) => {
    setId(next);
    const url = new URL(location.href);
    if (next) url.searchParams.set("delivery_package", next);
    else url.searchParams.delete("delivery_package");
    window.history.pushState(window.history.state, "", url);
  };

  useEffect(() => {
    const back = () => setId(new URL(location.href).searchParams.get("delivery_package") || "");
    window.addEventListener("popstate", back);
    return () => window.removeEventListener("popstate", back);
  }, []);

  useEffect(()=>{
    if(initialPackageId)setId(initialPackageId);
  },[initialPackageId]);

  useEffect(() => {
    setReadiness(undefined);
    authorizationRetry.current = undefined;
  }, [scope.content_sha256, scope.revision]);

  useEffect(() => {
    try {
      const legacy = service.pendingPackage?.(project, task);
      if (legacy) setId((current) => current || legacy.command_id);
    } catch (cause) {
      setError((cause as Error).message);
    }
  }, [project, refresh, service, task]);

  useEffect(() => {
    const controller = new AbortController();
    void service.history(project, task, undefined, controller.signal)
      .then((page) => {
        if (!controller.signal.aborted) {
          setHistory(page.items);
          setCursor(page.next_cursor);
        }
      })
      .catch((cause: Error) => {
        if (!controller.signal.aborted) setError(cause.message);
      });
    return () => controller.abort();
  }, [project, refresh, service, task]);

  useEffect(() => {
    setReadiness(undefined);
    if (!service.packageReadiness) return;
    const controller = new AbortController();
    void service.packageReadiness(project, task, controller.signal)
      .then((next) => {
        if (controller.signal.aborted) return;
        setReadiness(next);
        if (next.package) {
          setJob(next.package);
          setId((current) => current || next.package?.job.id || "");
        }
      })
      .catch((cause: Error) => {
        if (!controller.signal.aborted) setError(cause.message);
      });
    return () => controller.abort();
  }, [project, refresh, service, task]);

  useEffect(() => {
    if (readiness?.consent?.state !== "armed" || readiness.package?.job.phase === "ready") return;
    const timer = setTimeout(() => setRefresh((value) => value + 1), 2500);
    return () => clearTimeout(timer);
  }, [readiness]);

  useEffect(() => {
    if (!id) { setJob(undefined); return; }
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    setJob(undefined);
    const read = async () => {
      try {
        const next = await service.packageStatus(project, task, id, controller.signal);
        if (controller.signal.aborted) return;
        setJob(next);
        if (next.active && !next.interrupted) timer = setTimeout(() => void read(), 1500);
      } catch (cause) {
        if (!controller.signal.aborted) setError((cause as Error).message);
      }
    };
    if (!busy) void read();
    return () => { controller.abort(); clearTimeout(timer); };
  }, [busy, id, project, refresh, service, task]);

  useEffect(()=>{
    if(job?.job.phase!=="ready"||!job.job.result||readyNotification.current===job.job.id)return;
    if(expectedDemo&&demoPackageEvidenceError(job.job.result,expectedDemo))return;
    readyNotification.current=job.job.id;
    onReady?.(job);
  },[expectedDemo,job,onReady]);

  const act = async (operation: () => Promise<void>) => {
    if (pending.current) return;
    pending.current = true; setBusy(true); setError("");
    try { await operation(); }
    catch (cause) { setError((cause as Error).message); }
    finally { pending.current = false; setBusy(false); }
  };

  const authorize = () => void act(async () => {
    if (!service.authorizePackage || !readiness || locked) return;
    if (readiness.intent_revision !== scope.revision || readiness.intent_sha256 !== scope.content_sha256) {
      throw new Error("服务器 readiness 不属于当前交付版本，请刷新任务。");
    }
    if (!authorizationRetry.current
      || authorizationRetry.current.intent_revision !== readiness.intent_revision
      || authorizationRetry.current.intent_sha256 !== readiness.intent_sha256) {
      authorizationRetry.current = {
        id: crypto.randomUUID(),
        intent_revision: readiness.intent_revision,
        intent_sha256: readiness.intent_sha256,
        confirmed: true,
      };
    }
    await service.authorizePackage(project, task, authorizationRetry.current);
    authorizationRetry.current = undefined;
    setRefresh((value) => value + 1);
  });

  const cancelAuthorization = () => void act(async () => {
    if (!service.cancelPackageAuthorization || !readiness?.consent) return;
    await service.cancelPackageAuthorization(project, task, readiness.consent.input.id);
    setRefresh((value) => value + 1);
  });

  const receipt = job?.job.result;
  const demoEvidenceError = receipt&&expectedDemo ? demoPackageEvidenceError(receipt,expectedDemo) : null;
  const currentScope = readiness
    && readiness.intent_revision === scope.revision
    && readiness.intent_sha256 === scope.content_sha256;
  const armed = currentScope && readiness.consent?.state === "armed";
  const reviewsComplete = !!readiness && formalReviewCountsComplete(readiness.counts);

  return <section className="delivery-package" aria-label="训练数据包交付">
    <h3>交付训练数据包</h3>
    <p>当前范围 {scope.image_ids.length} 张。服务端根据正式整图审核计算 readiness；浏览器不逐图扫描，也不会在页面打开或变为 Ready 时自动创建数据包。</p>

    {!service.packageReadiness && <p role="status">当前 Adapter 尚未接入服务端打包 readiness。历史和真实回执仍可查看，但不能在这里安全授权自动打包。</p>}
    {service.packageReadiness && !readiness && !error && <p role="status">读取服务端审核摘要与打包 readiness…</p>}

    {readiness && <div className="delivery-package-readiness">
      <strong>{reviewsComplete ? "正式审核齐全" : "正式审核未齐全"}</strong>
      <p>已完成 {readiness.counts.complete}/{readiness.counts.total} · 待处理 {readiness.counts.unresolved} · 失败 {readiness.counts.failed}</p>
      <p>{readiness.ready
        ? "服务端打包条件已满足。"
        : reviewsComplete
          ? armed
            ? "已授权，等待服务端创建或恢复数据包。"
            : "审核已完成，等待你授权本正式范围。"
          : "完成剩余审核后，服务端会重新计算打包条件。"}</p>
      {readiness.blockers.map((blocker) => <div key={blocker.code} className="notice">
        <p>{blocker.message}</p>
        {!!blocker.image_ids.length && <Disclosure title={`检查相关图片 · ${blocker.image_ids.length}`}>
          {blocker.image_ids.map((imageId) => <button type="button" key={imageId} onClick={() => onInspect(imageId)}>检查 {imageId.slice(0, 8)}</button>)}
        </Disclosure>}
      </div>)}
      {!armed && readiness.consent?.state !== "consumed" && <div className="notice">
        <p>授权范围固定为交付 revision {scope.revision}。授权后，服务端只在该版本审核齐全且独立校验通过时触发一次打包。</p>
        <button type="button" disabled={busy || locked || !currentScope || !service.authorizePackage} onClick={authorize}>允许审核齐全后自动打包</button>
      </div>}
      {armed && <div className="notice">
        <p>自动打包授权已保存。未齐全时只等待；齐全后由服务端 admission 触发，不由 React 发送创建请求。</p>
        <button type="button" disabled={busy || !service.cancelPackageAuthorization} onClick={cancelAuthorization}>取消自动打包授权</button>
      </div>}
      {readiness.consent?.state === "consumed" && <p role="status">授权已由服务端消费；请查看对应数据包的真实状态和回执。</p>}
    </div>}

    {error && <p role="alert" className="error">{error} 不会自动重试或显示完成。</p>}
    <button type="button" disabled={busy} onClick={() => setRefresh((value) => value + 1)}>刷新审核与打包状态</button>

    {!!history.length && <label>本任务已保存的数据包
      <select aria-label="本任务已保存的数据包" value={id} onChange={(event) => select(event.target.value)}>
        <option value="">选择数据包</option>
        {history.map((item) => <option key={item.id} value={item.id}>{item.created_at} · {item.id.slice(0, 8)}</option>)}
      </select>
    </label>}
    {cursor && <button type="button" disabled={busy} onClick={() => void act(async () => {
      const page = await service.history(project, task, cursor);
      setHistory((current) => [...current, ...page.items.filter((item) => !current.some((saved) => saved.id === item.id))]);
      setCursor(page.next_cursor);
    })}>更早的数据包</button>}

    {id && <button type="button" disabled={busy} onClick={() => { setError(""); setRefresh((value) => value + 1); }}>读取这个数据包状态</button>}
    {id && !job && !error && <p role="status">读取服务器记录…</p>}
    {job && <article className="delivery-package-receipt">
      <strong role="status">{job.interrupted ? "执行已中断或远端状态未知" : phases[job.job.phase]}</strong>
      {job.interrupted && <p>没有成功回执，不会自动重试收费或重新创建；刷新只查询原命令状态。</p>}
      {job.job.error && <p role="alert">{job.job.error}</p>}
      {!["ready", "failed", "cancelled"].includes(job.job.phase) && <button type="button" disabled={busy} onClick={() => void act(async () => {
        await service.cancelPackage(project, task, job.job.id);
        setRefresh((value) => value + 1);
      })}>取消这个打包任务</button>}
      {job.job.phase === "ready" && receipt && <>
        <p>Ultralytics YOLO · 目标检测 · 冻结交付 revision {job.job.intent_revision}</p>
        <p>类别：{receipt.summary?.labels.join("、") || "旧回执未记录，请查看包内清单"}</p>
        <p>训练图片 {receipt.summary?.splits.train ?? "未记录"} · 验证图片 {receipt.summary?.splits.val ?? "未记录"} · 确认负样本 {receipt.negatives} · 排除 {receipt.excluded}</p>
        <p>已纳入 {receipt.images} 张原图、{receipt.objects} 个正式对象 · {receipt.bytes.toLocaleString()} bytes</p>
        {receipt.summary?.demo&&<p>示例 {receipt.summary.demo.id} · {receipt.summary.demo.version} · 数据 {receipt.summary.demo.data_sha256}</p>}
        {receipt.summary?.source_mode&&<p>来源：{receipt.summary.source_mode==="preset_candidates"?"预置候选":"本次模型预测"} · {receipt.summary.live_inference_occurred?"发生过现场模型推理":"本次无模型请求"}</p>}
        {receipt.summary?.source_counts&&<p>预置候选 {receipt.summary.source_counts.preset_candidate??0} · 模型预测 {receipt.summary.source_counts.live_model_prediction??0} · 人工修订 {receipt.summary.source_counts.human_revision??0}</p>}
        {!!receipt.summary?.review_sources?.length&&<p>审核来源：{receipt.summary.review_sources.join("、")}</p>}
        <p>结构检查通过；不表示模型精度或漏检检查通过。完整性依据为保存的人工整图确认。</p>
        {demoEvidenceError
          ? <p role="alert">{demoEvidenceError}；当前包不可下载，请刷新服务端交付状态。</p>
          : <a href={service.downloadUrl(project, task, job.job.id)} download onClick={()=>onDownload?.(job.job.id)}>下载数据集 ZIP</a>}
        <Disclosure title="查看真实检查报告">
          <p>SHA-256：{receipt.sha256}</p>
          <p>完整报告、原图哈希、来源与划分位于 ZIP 的 annotagent 目录。</p>
          {receipt.summary?.warnings.map((warning, index) => <p key={index}>{warning}</p>)}
          {Object.entries(receipt.summary?.exclusions || {}).map(([imageId, why]) => <p key={imageId}>排除 {imageId}：{why}</p>)}
        </Disclosure>
      </>}
    </article>}
  </section>;
}
