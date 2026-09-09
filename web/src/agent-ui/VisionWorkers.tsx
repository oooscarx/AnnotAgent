import { useEffect, useRef, useState } from "react";
import type { api } from "../api";
import type { DetectionWorkerTestResult, ModelBinding } from "../types";
import { Disclosure } from "./Disclosure";
import { Dialog } from "./Dialog";

export type VisionWorkerService = Pick<typeof api, "models" | "testModel">;
export function VisionWorkers({ service }: { service: VisionWorkerService }) {
  const [workers, setWorkers] = useState<ModelBinding[]>();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [selection, setSelection] = useState<ModelBinding>();
  const [accepted, setAccepted] = useState(false);
  const [results, setResults] = useState<Record<string, DetectionWorkerTestResult>>({});
  const generation = useRef(0);
  const pending = useRef(false);
  const reload = async () => {
    const current = ++generation.current;
    const result = await service.models();
    if (current === generation.current) setWorkers(result.models.filter(m => m.scope === "workspace_worker"));
  };
  useEffect(() => {
    void reload().catch(e => setError(e.message));
    return () => { generation.current++; };
  }, [service]);
  const discover = async () => {
    if (!selection || !accepted || pending.current) return;
    pending.current = true; setBusy(true); setError("");
    const selected = selection;
    try {
      const latest = (await service.models()).models.find(m => m.id === selected.id && m.scope === "workspace_worker");
      if (JSON.stringify(latest) !== JSON.stringify(selected)) throw new Error("模型绑定已变化，请重新读取并确认数据目的地。");
      setResults(old => { const next = { ...old }; delete next[selected.id]; return next; });
      const result = await service.testModel(selected.id);
      if (result.model_id !== selected.id) throw new Error("返回结果不属于所选模型，不能显示为成功。");
      setResults(old => ({ ...old, [selected.id]: result }));
      setSelection(undefined);
      await reload();
    } catch (e) {
      setSelection(undefined);
      setError(`${(e as Error).message}。未自动重试。检查可能已在远端完成，请先重新读取状态。`);
    } finally { pending.current = false; setBusy(false); }
  };
  return <section aria-label="HTTP Vision 模型绑定">
    <h2>HTTP Vision 模型绑定</h2>
    <p>保留现有协议绑定与契约证据。发现检查不上传项目图片，不执行样例推理，不代表标注准确率。</p>
    {error && <p role="alert" className="error">{error}</p>}
    <button disabled={busy} onClick={() => { setError(""); void reload().catch(e => setError(e.message)); }}>重新读取模型绑定</button>
    {!workers ? <p role="status">{error ? "模型绑定未读取。" : "读取模型绑定…"}</p> : workers.length === 0 ? <p>尚无 HTTP Vision 模型绑定。可使用上方的已安装插件和模型配置。</p> : workers.map(worker => <article className="settings-row" key={worker.id}>
      <div><strong>{worker.model}</strong><p>{worker.role} · {worker.health_status}</p><p>{worker.health_detail}</p></div>
      <div className="row-control">
        <Disclosure title="端点、契约与来源">
          <p>绑定：{worker.id}</p><p>端点：{worker.endpoint || "未配置"}</p>
          <p>能力：{worker.capabilities?.join(" · ") || "未声明"}</p>
          <p>分数含义：{worker.score_semantics || "未知"}</p>
          <p>类别：{worker.label_space?.join(" · ") || "未声明"}</p>
          <p>版本：{worker.model_version || "未知"} · 架构：{worker.architecture || "未知"}</p>
          <p>权重摘要：{worker.checkpoint_sha256 || "未知"}</p>
          <p>许可证：{worker.license_summary || "未声明"}</p>
          <p>声明的推理单价：{worker.cost_per_request === undefined ? "未知" : `${worker.cost_per_request} / 请求（不是本次发现检查报价）`}</p>
        </Disclosure>
        <button disabled={busy || !worker.endpoint} onClick={() => { setAccepted(false); setSelection(worker); }}>发现检查…</button>
        {results[worker.id] && <div role="status"><strong>{results[worker.id].passed ? "发现检查通过（非推理验证）" : `检查未通过：${results[worker.id].failed_stage || "契约检查"}`}</strong><p>{results[worker.id].error || results[worker.id].evidence?.detail}</p><Disclosure title="发现检查响应"><pre>{JSON.stringify(results[worker.id], null, 2)}</pre></Disclosure></div>}
      </div>
    </article>)}
    {selection && <Dialog title="确认 HTTP Vision 发现检查" onClose={() => { if (!busy) setSelection(undefined); }}>
      <p>模型：{selection.model} · 绑定：{selection.id}</p><p>目的地：{selection.endpoint}</p>
      <p>读取健康、能力、模型身份和契约，并保存可用性证据；不发送项目图片。远端收费策略未知，不能视为免费。</p>
      <label><input type="checkbox" checked={accepted} disabled={busy} onChange={e => setAccepted(e.target.checked)} />允许本次访问上述端点</label>
      <div className="actions"><button disabled={busy} onClick={() => setSelection(undefined)}>取消</button><button disabled={busy || !accepted} onClick={() => void discover()}>{busy ? "等待检查结果…" : "确认发现检查"}</button></div>
    </Dialog>}
  </section>;
}
