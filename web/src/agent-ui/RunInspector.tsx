import { useEffect, useState } from "react";
import type { api } from "../api";
import type { RunDebugSummary, RunNodeArtifactInspection } from "../types";
import { Disclosure } from "./Disclosure";

export type RunInspectorService = Pick<typeof api, "pipelineArtifacts" | "runDebugSummary">;
export function assertInspection(value: RunNodeArtifactInspection, project: string, run: string) {
  if (value.project_id !== project || value.run_id !== run) throw new Error("节点产物不属于当前项目或运行。");
  return value;
}
export function RunInspector({ service, projectId, runId }: { service: RunInspectorService; projectId: string; runId: string }) {
  const [inspection, setInspection] = useState<RunNodeArtifactInspection>();
  const [summary, setSummary] = useState<RunDebugSummary>();
  const [error, setError] = useState("");
  const [revision, setRevision] = useState(0);
  const [nodeId, setNodeId] = useState(() => new URL(location.href).searchParams.get("node") || "");
  useEffect(() => {
    const controller = new AbortController();
    setInspection(undefined); setSummary(undefined); setError("");
    void Promise.all([service.pipelineArtifacts(runId, controller.signal), service.runDebugSummary(runId, controller.signal)]).then(([value, detail]) => {
      if (controller.signal.aborted) return;
      assertInspection(value, projectId, runId);
      if (detail.run_id !== runId) throw new Error("调试摘要不属于当前运行。");
      setInspection(value); setSummary(detail);
    }).catch(e => { if (!controller.signal.aborted) setError(e.message); });
    return () => controller.abort();
  }, [service, projectId, runId, revision]);
  useEffect(() => {
    const read = () => setNodeId(new URL(location.href).searchParams.get("node") || "");
    window.addEventListener("popstate", read); return () => window.removeEventListener("popstate", read);
  }, []);
  const node = inspection?.nodes.find(n => n.node_id === nodeId);
  return <section aria-label="节点与 Artifact 检查" className="native-run-inspector">
    <h2>执行详情</h2><p>以下是保存的实际执行记录与中间产物，不是新增的最终目标，也不是一次重跑。</p>
    <button onClick={() => setRevision(v => v + 1)}>重新读取执行记录</button>
    {error && <p role="alert">{error}</p>}
    {!inspection && !error && <p role="status">读取节点与产物…</p>}
    {summary && <p>{summary.succeeded_node_count}/{summary.node_count} 节点成功 · {summary.failed_node_count} 失败 · {summary.duration_ms} ms · 输入/输出 {summary.usage.input_tokens}/{summary.usage.output_tokens} tokens · {Number(summary.usage.estimated_cost) > 0 ? `估算 $${summary.usage.estimated_cost}` : "费用未核实"}</p>}
    {inspection && <>
      <p>Workflow {inspection.workflow_id} · v{inspection.workflow_version}</p>
      {!inspection.nodes.length ? <p>此运行未保存节点产物，没有生成替代记录。</p> : <label>选择实际执行节点<select value={nodeId} onChange={e => {
        setNodeId(e.target.value); const url = new URL(location.href);
        if (e.target.value) url.searchParams.set("node", e.target.value); else url.searchParams.delete("node");
        url.searchParams.delete("artifact"); history.replaceState(history.state, "", url);
      }}><option value="">请选择节点</option>{inspection.nodes.map(n => <option value={n.node_id} key={n.node_id}>{n.operation} · {n.node_id} · {n.status}</option>)}</select></label>}
      {nodeId && !node && <p role="alert">所链接的节点不在本次执行中。请选择列表中的节点；没有自动替换为其他节点。</p>}
      {node && <article>
        <h3>{node.operation}</h3><p>{node.status} · {node.latency_ms} ms · 尝试 {node.attempts} 次 · {node.cache_hit ? "命中缓存" : "未命中缓存"}</p>
        <p>模型用量：{node.usage.input_tokens}/{node.usage.output_tokens} tokens · {Number(node.usage.cost) > 0 ? `$${node.usage.cost}` : "费用未核实"}</p>
        {node.route && <p>实际分支：{node.route}</p>}
        {node.error && <p role="alert">{node.error.code}：{node.error.summary} · {node.error.retryable ? "服务器标记为可重试；未自动执行" : "不可直接重试"}</p>}
        <Disclosure title="节点配置"><pre>{JSON.stringify(node.configuration, null, 2)}</pre></Disclosure>
        {(["inputs", "outputs"] as const).map(key => <div key={key}><h4>{key === "inputs" ? "输入产物" : "输出产物"} · {node[key].length}</h4>{node[key].map((artifact, index) => <Disclosure key={index} title={`${index + 1} · ${artifact.kind}`}><pre>{JSON.stringify(artifact.artifact, null, 2)}</pre></Disclosure>)}</div>)}
        {node.metadata && <Disclosure title="执行元数据"><pre>{JSON.stringify(node.metadata, null, 2)}</pre></Disclosure>}
      </article>}
      <Disclosure title="执行错误汇总">{summary?.issues.map((issue, index) => <p key={index}>{issue.node_id} · {issue.code}：{issue.summary}</p>)}</Disclosure>
    </>}
  </section>;
}
