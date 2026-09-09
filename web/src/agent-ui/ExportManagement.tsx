import { useEffect, useRef, useState } from "react";
import type { api } from "../api";
import type { ExportReadiness, ProjectExportResult } from "../types";
import { agentPath } from "./navigationContract";
import { Disclosure } from "./Disclosure";

export type ExportService = Pick<typeof api, "exportReadiness" | "export">;

/** The server owns export eligibility, the report and downloadable bytes. */
export function ExportManagement({ projectId, service }: { projectId: string; service: ExportService }) {
  const [readiness, setReadiness] = useState<ExportReadiness>();
  const [format, setFormat] = useState("");
  const [result, setResult] = useState<ProjectExportResult>();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [uncertain, setUncertain] = useState(false);
  const [reload, setReload] = useState(0);
  const pending = useRef(false);
  const lifetime = useRef<AbortController | undefined>(undefined);

  useEffect(() => {
    const controller = new AbortController();
    lifetime.current = controller;
    setReadiness(undefined);
    setResult(undefined);
    setError("");
    void service.exportReadiness(projectId, controller.signal).then(value => {
      if (controller.signal.aborted) return;
      if (value.project_id !== projectId) throw new Error("导出范围与当前项目不匹配。");
      setReadiness(value);
      setResult(value.last_export);
      setFormat(current => value.formats.some(f => f.format === current && f.supported)
        ? current : value.formats.find(f => f.supported && f.recommended)?.format
          ?? value.formats.find(f => f.supported)?.format ?? "");
    }).catch(reason => { if (!controller.signal.aborted) setError((reason as Error).message); });
    return () => controller.abort();
  }, [projectId, service, reload]);

  const selected = readiness?.formats.find(f => f.format === format);
  const execute = async () => {
    if (pending.current || uncertain || !readiness?.ready || !selected?.supported) return;
    pending.current = true;
    setBusy(true);
    setError("");
    setResult(undefined);
    const signal = lifetime.current?.signal;
    try {
      const value = await service.export(projectId, format, undefined, signal);
      if (signal?.aborted) return;
      if (value.format !== format) throw new Error("服务器导出格式与本次选择不一致，请核对报告。");
      setResult(value);
    } catch (reason) {
      if (!signal?.aborted) {
        setUncertain(true);
        setError(`${(reason as Error).message}。导出没有得到成功确认。重新读取只查询已保存报告，不会再次生成文件。`);
      }
    } finally {
      pending.current = false;
      if (!signal?.aborted) setBusy(false);
    }
  };

  return <section className="native-project-manager native-export">
    <h1>导出标注</h1>
    <nav className="native-management-tabs">
      <a href={agentPath({ kind: "work", projectId })}>返回 Agent</a>
      <a href={agentPath({ kind: "management", projectId, page: "review" })}>审核标注</a>
      <a href={agentPath({ kind: "management", projectId, page: "data" })}>图片数据</a>
    </nav>
    {error && <p className="error" role="alert">{error}</p>}
    {!readiness && !error && <p role="status">读取导出范围…</p>}
    {readiness && <>
      <p>当前项目 · {readiness.image_count} 张图片 · {readiness.processed_image_count} 张已处理</p>
      <p>{readiness.accepted_annotations} 个已接受标注 · {readiness.unresolved_reviews} 个待审核</p>
      <p>仅导出服务器允许交付的已接受标注；导出不会接受待审项，也不会调用模型。</p>
      {readiness.blocking_issues.map(blocker => <div className="notice" key={blocker.code}>
        <strong>{blocker.title}</strong><p>{blocker.explanation}</p>
      </div>)}
      <label>导出格式
        <select value={format} disabled={busy} onChange={event => setFormat(event.target.value)}>
          <option value="" disabled>选择可用格式</option>
          {readiness.formats.map(item => <option key={item.format} value={item.format} disabled={!item.supported}>
            {item.display_name}{item.supported ? "" : " · 不兼容"}
          </option>)}
        </select>
      </label>
      {selected && <><p>{selected.summary}</p>{selected.warnings.map(warning => <p className="notice" key={warning}>{warning}</p>)}</>}
      <Disclosure title="格式兼容性">
        {readiness.formats.map(item => <article className="settings-row" key={item.format}>
          <div><strong>{item.display_name} · {item.supported ? "支持" : "不支持"}</strong>
            <p>{item.summary}</p>{item.warnings.map(warning => <p key={warning}>{warning}</p>)}
            {item.unsupported_task_kinds.length > 0 && <p>不支持的标注类型：{item.unsupported_task_kinds.join("、")}</p>}
          </div>
        </article>)}
      </Disclosure>
      <div className="actions">
        <button disabled={busy} onClick={() => setReload(value => value + 1)}>重新读取范围与报告</button>
        <button className="primary" disabled={busy || uncertain || !readiness.ready || !selected?.supported} onClick={() => void execute()}>
          {busy ? "正在生成导出文件…" : `导出为 ${selected?.display_name || "所选格式"}`}
        </button>
      </div>
      {busy && <p role="status">请求正在执行。离开页面不代表服务器取消；返回后先读取报告。</p>}
      {uncertain && <p>本页暂停再次生成。请核实最新报告；没有报告不等于服务器未执行。</p>}
    </>}
    {result && <section aria-label="已保存导出报告" className="export-delivery">
      <h2>服务器已保存的导出</h2>
      <p>{result.format} · {result.completed_at}</p>
      <p>导出 {result.report.exported_count} 个 · 跳过 {result.report.skipped_count} 个</p>
      {result.report.warnings.map(warning => <p className="notice" key={warning}>{warning}</p>)}
      {result.delivery ? <a download href={`/api/projects/${encodeURIComponent(projectId)}/exports/${encodeURIComponent(result.delivery.id)}/download`}>下载标注文件（{result.delivery.bytes} bytes）</a>
        : <p>此历史报告没有浏览器下载交付；服务器位置不代表本机文件夹。</p>}
      <Disclosure title="文件与来源详情">
        <p>服务器路径：{result.output_path}</p>
        <p>来源指纹：{result.source_fingerprint}</p>
        {result.delivery && <p>SHA-256：{result.delivery.sha256}</p>}
        <ul>{result.report.output_files.map(file => <li key={file}>{file}</li>)}</ul>
        {result.report.unsupported_task_kinds.length > 0 && <p>未导出的类型：{result.report.unsupported_task_kinds.join("、")}</p>}
      </Disclosure>
    </section>}
  </section>;
}
