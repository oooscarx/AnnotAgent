import { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import type {
  HistoryRun,
  PipelineDraftDiff,
  PipelineGeometryMetrics,
  PipelineImprovementSession,
  ProjectGeometryPolicy,
  ProjectSummary,
  WorkflowVersion,
} from "../types";

function title(value: string): string {
  return value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function percent(value?: number): string {
  return value === undefined ? "Not measured" : `${Math.round(value * 100)}%`;
}

function decimal(value?: number): string {
  return value === undefined ? "Not measured" : value.toFixed(3);
}

function diffRows(diff: PipelineDraftDiff) {
  return [
    ...diff.added_nodes.map((change) => ({ id: change.change_id, tone: "added", label: `Add ${title(change.node_type)}` })),
    ...diff.removed_nodes.map((change) => ({ id: change.change_id, tone: "removed", label: `Remove ${title(change.node_type)}` })),
    ...diff.modified_nodes.map((change) => ({ id: change.change_id, tone: "changed", label: `Update ${change.node_id}` })),
    ...diff.model_binding_changes.map((change) => ({ id: change.change_id, tone: "changed", label: `Change model for ${change.node_id}` })),
    ...diff.policy_changes.map((change) => ({ id: change.change_id, tone: "changed", label: `Update decision policy for ${change.node_id}` })),
    ...diff.added_edges.map((change) => ({ id: change.change_id, tone: "added", label: `Connect ${change.edge.from_node} → ${change.edge.to_node}` })),
    ...diff.removed_edges.map((change) => ({ id: change.change_id, tone: "removed", label: `Disconnect ${change.edge.from_node} → ${change.edge.to_node}` })),
  ];
}

function Metrics({ label, value }: { label: string; value: PipelineGeometryMetrics }) {
  return <section className="geometry-comparison-column">
    <h4>{label}</h4>
    <dl>
      <div><dt>Semantic precision</dt><dd>{percent(value.semantic_precision)}</dd></div>
      <div><dt>Semantic recall</dt><dd>{percent(value.semantic_recall)}</dd></div>
      <div><dt>Median IoU</dt><dd>{decimal(value.median_iou)}</dd></div>
      <div><dt>P10 IoU</dt><dd>{decimal(value.p10_iou)}</dd></div>
      <div><dt>Median center shift</dt><dd>{decimal(value.median_center_shift)}</dd></div>
      <div><dt>P90 center shift</dt><dd>{decimal(value.p90_center_shift)}</dd></div>
      <div><dt>Manual resize rate</dt><dd>{percent(value.manual_resize_rate)}</dd></div>
      <div><dt>Review rate</dt><dd>{percent(value.review_rate)}</dd></div>
      <div><dt>Cost / image</dt><dd>${value.cost_per_image}</dd></div>
      <div><dt>Latency / image</dt><dd>{value.latency_per_image_ms} ms</dd></div>
      <div><dt>Failures</dt><dd>{value.failure_count}</dd></div>
    </dl>
    {Object.keys(value.size_buckets).length > 0 && <details>
      <summary>Object-size evidence</summary>
      <div className="geometry-size-buckets">
        {Object.entries(value.size_buckets).map(([bucket, metrics]) => <span key={bucket}>
          <strong>{title(bucket)}</strong>
          <small>{metrics.reference_count} references · median IoU {decimal(metrics.median_iou)}</small>
        </span>)}
      </div>
    </details>}
  </section>;
}

function RunChoice({
  run,
  checked,
  disabled,
  onChange,
}: {
  run: HistoryRun;
  checked: boolean;
  disabled: boolean;
  onChange: (checked: boolean) => void;
}) {
  return <label className="improvement-run-choice">
    <input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} />
    <span><strong>{run.id.slice(0, 8)}</strong><small>{new Date(run.updated_at).toLocaleString()} · {title(run.status)}</small></span>
  </label>;
}

export function ImproveAutomationPanel({
  project,
  runs,
  workflows,
  onDraftApplied,
  onError,
}: {
  project: ProjectSummary;
  runs: HistoryRun[];
  workflows: WorkflowVersion[];
  onDraftApplied: (draftId: string) => void;
  onError: (message: string) => void;
}) {
  const projectRuns = useMemo(
    () => runs.filter((run) => run.project_name === project.name && !["running", "paused", "pending"].includes(run.status)),
    [project.name, runs],
  );
  const published = workflows.filter((workflow) => workflow.status === "published" && workflow.source.startsWith("published draft"));
  const bboxTasks = project.annotation_schema.filter((task) => task.kind === "bounding_box");
  const [workflowKey, setWorkflowKey] = useState("");
  const [taskId, setTaskId] = useState(bboxTasks[0]?.id ?? "");
  const task = bboxTasks.find((candidate) => candidate.id === taskId);
  const [label, setLabel] = useState(task?.labels[0] ?? "");
  const [evidenceRunIds, setEvidenceRunIds] = useState<string[]>([]);
  const [holdoutRunIds, setHoldoutRunIds] = useState<string[]>([]);
  const [sessions, setSessions] = useState<PipelineImprovementSession[]>([]);
  const [activeId, setActiveId] = useState("");
  const active = sessions.find((session) => session.id === activeId) ?? sessions[0];
  const [selectedChanges, setSelectedChanges] = useState<string[]>([]);
  const [policies, setPolicies] = useState<ProjectGeometryPolicy[]>([]);
  const [calibrations, setCalibrations] = useState<Awaited<ReturnType<typeof api.geometryCalibrations>>["calibrations"]>([]);
  const [calibrationNode, setCalibrationNode] = useState("");
  const [busy, setBusy] = useState("");
  const selectedWorkflow = published.find((workflow) => `${workflow.workflow_id}:${workflow.version}` === workflowKey) ?? published[0];
  const geometryNodes = selectedWorkflow?.nodes.filter((node) => node.model_binding && (node.node_type.includes("detect") || node.node_type.includes("ground") || node.node_type.includes("segment"))) ?? [];
  const refresh = () => Promise.all([
    api.pipelineImprovements(project.id),
    api.geometryPolicy(project.id),
    api.geometryCalibrations(project.id),
  ]).then(([improvements, policyResult, calibrationResult]) => {
    setSessions(improvements.pipeline_improvements);
    setPolicies(policyResult.policies);
    setCalibrations(calibrationResult.calibrations);
    setActiveId((current) => improvements.pipeline_improvements.some((session) => session.id === current) ? current : improvements.pipeline_improvements[0]?.id ?? "");
  });
  useEffect(() => {
    setWorkflowKey((current) => current || (published[0] ? `${published[0].workflow_id}:${published[0].version}` : ""));
    void refresh().catch((error: Error) => onError(error.message));
  }, [project.id]);
  useEffect(() => {
    setLabel(task?.labels[0] ?? "");
  }, [taskId]);
  useEffect(() => {
    setSelectedChanges(active ? diffRows(active.diff).map((change) => change.id) : []);
  }, [active?.id]);
  useEffect(() => {
    setCalibrationNode((current) => geometryNodes.some((node) => node.id === current) ? current : geometryNodes[0]?.id ?? "");
  }, [workflowKey]);
  const perform = (name: string, operation: () => Promise<PipelineImprovementSession>) => {
    setBusy(name);
    void operation().then((session) => {
      setSessions((current) => [session, ...current.filter((item) => item.id !== session.id)]);
      setActiveId(session.id);
    }).catch((error: Error) => onError(error.message)).finally(() => setBusy(""));
  };
  const create = () => {
    if (!selectedWorkflow || !taskId || !label || !evidenceRunIds.length) return;
    perform("create", () => api.createPipelineImprovement(project.id, {
      workflow_id: selectedWorkflow.workflow_id,
      workflow_version: Number(selectedWorkflow.version),
      target_task_id: taskId,
      target_label: label,
      evidence_run_ids: evidenceRunIds,
      evaluation_run_ids: holdoutRunIds,
    }));
  };
  const compare = () => {
    if (!active || !window.confirm("Run the baseline and candidate on the selected independent holdout? This can call configured models and incur Provider cost.")) return;
    perform("compare", () => api.comparePipelineImprovement(active.id));
  };
  const apply = () => {
    if (!active || !selectedChanges.length) return;
    perform("apply", () => api.applyPipelineImprovement(active.id, selectedChanges).then((session) => {
      if (session.applied_draft_id) onDraftApplied(session.applied_draft_id);
      return session;
    }));
  };
  const calibrate = () => {
    if (!selectedWorkflow || !calibrationNode || !taskId || !evidenceRunIds.length) return;
    setBusy("calibrate");
    void api.createGeometryCalibration(project.id, {
      workflow_id: selectedWorkflow.workflow_id,
      workflow_version: Number(selectedWorkflow.version),
      node_id: calibrationNode,
      task_id: taskId,
      label_id: label || undefined,
      evidence_run_ids: evidenceRunIds,
    }).then(refresh).catch((error: Error) => onError(error.message)).finally(() => setBusy(""));
  };
  const changeEvidence = (runId: string, checked: boolean) => {
    setEvidenceRunIds((current) => checked ? [...new Set([...current, runId])] : current.filter((id) => id !== runId));
    if (checked) setHoldoutRunIds((current) => current.filter((id) => id !== runId));
  };
  const changeHoldout = (runId: string, checked: boolean) => {
    setHoldoutRunIds((current) => checked ? [...new Set([...current, runId])] : current.filter((id) => id !== runId));
    if (checked) setEvidenceRunIds((current) => current.filter((id) => id !== runId));
  };
  return <section className="panel improve-automation" id="improve-automation">
    <header className="improve-automation-header">
      <div><span className="eyebrow">Evidence-driven revision</span><h2>Improve Automation</h2><p>Diagnose the current immutable Version, create a focused Patch Draft, and compare it on separate holdout images. AnnotAgent never publishes this change.</p></div>
      {active && <span className={`improvement-status ${active.status}`}>{title(active.status)}</span>}
    </header>
    <div className="geometry-policy-strip">
      {policies.map((policy) => <article key={policy.task_kind}><span>{title(policy.task_kind)}</span><strong>{title(policy.required_quality)}</strong><small>{title(policy.auto_accept_policy)} · {policy.calibration_thresholds.minimum_sample_count} references required</small></article>)}
      {!policies.length && <article><span>Geometry policy</span><strong>Loading conservative defaults…</strong></article>}
    </div>
    <div className="improvement-setup-grid">
      <label>Baseline Version<select value={workflowKey} onChange={(event) => setWorkflowKey(event.target.value)}>{published.map((workflow) => <option key={`${workflow.workflow_id}:${workflow.version}`} value={`${workflow.workflow_id}:${workflow.version}`}>{workflow.name} · v{workflow.version}</option>)}</select></label>
      <label>Bounding-box task<select value={taskId} onChange={(event) => setTaskId(event.target.value)}>{bboxTasks.map((candidate) => <option key={candidate.id} value={candidate.id}>{candidate.display_name || candidate.id}</option>)}</select></label>
      <label>Target Label<select value={label} onChange={(event) => setLabel(event.target.value)}>{(task?.labels ?? []).map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
    </div>
    {!published.length || !bboxTasks.length ? <div className="guided-callout"><strong>Improvement setup is incomplete</strong><p>A published Workflow and bounding-box task are required.</p></div> : <div className="improvement-evidence-grid">
      <fieldset><legend>Diagnosis evidence</legend><p>Reviewed Runs used to decide what is wrong.</p><div className="improvement-run-list">{projectRuns.map((run) => <RunChoice key={run.id} run={run} checked={evidenceRunIds.includes(run.id)} disabled={holdoutRunIds.includes(run.id)} onChange={(checked) => changeEvidence(run.id, checked)} />)}{!projectRuns.length && <small>No terminal Runs are available.</small>}</div></fieldset>
      <fieldset><legend>Independent evaluation holdout</legend><p>Different Runs used to test whether the Patch actually improves.</p><div className="improvement-run-list">{projectRuns.map((run) => <RunChoice key={run.id} run={run} checked={holdoutRunIds.includes(run.id)} disabled={evidenceRunIds.includes(run.id)} onChange={(checked) => changeHoldout(run.id, checked)} />)}{!projectRuns.length && <small>No terminal Runs are available.</small>}</div></fieldset>
    </div>}
    <div className="button-row">
      <button disabled={Boolean(busy) || !selectedWorkflow || !taskId || !label || !evidenceRunIds.length} onClick={create}>{busy === "create" ? "Creating Patch…" : "Diagnose and create Patch Draft"}</button>
      <select aria-label="Saved improvement session" value={active?.id ?? ""} onChange={(event) => setActiveId(event.target.value)}><option value="">No saved session</option>{sessions.map((session) => <option key={session.id} value={session.id}>{title(session.diagnosis.primary_failure_class)} · {session.id.slice(0, 8)}</option>)}</select>
    </div>
    {active && <div className="improvement-result">
      <section className="improvement-diagnosis"><span className="eyebrow">Diagnosis</span><h3>{title(active.diagnosis.primary_failure_class)}</h3><ul>{active.diagnosis.evidence_statements.map((statement) => <li key={statement}>{statement}</li>)}</ul><div className="tag-group"><span>{active.diagnosis.geometry_correction_count} geometry corrections</span><span>{active.diagnosis.semantic_target_correct_count} semantic matches</span><span>{active.diagnosis.provider_failure_count} Provider failures</span><span>{active.diagnosis.no_candidate_count} no-candidate results</span></div></section>
      <section className="improvement-diff"><span className="eyebrow">Pipeline Patch</span><h3>Review selected changes</h3>{diffRows(active.diff).map((change) => <label className={change.tone} key={change.id}><input type="checkbox" checked={selectedChanges.includes(change.id)} disabled={active.status === "applied_to_draft"} onChange={(event) => setSelectedChanges((current) => event.target.checked ? [...current, change.id] : current.filter((id) => id !== change.id))} /><span>{change.tone === "added" ? "+" : change.tone === "removed" ? "−" : "~"} {change.label}</span></label>)}<small>{active.validation.valid ? "Static validation passed." : `${active.validation.issues.filter((issue) => issue.blocking).length} blocking validation issue(s).`}</small>{active.setup_requirements.map((requirement) => <p className="setup-requirement" key={requirement}>{requirement}</p>)}</section>
      <section className="improvement-actions"><span className="eyebrow">Human approval boundary</span><div className="button-row"><button disabled={Boolean(busy) || !active.evaluation_run_ids.length || !active.validation.valid} onClick={compare}>{busy === "compare" ? "Comparing…" : "Run Before / After"}</button><button className="primary" disabled={Boolean(busy) || active.status !== "awaiting_human_approval" || !selectedChanges.length} onClick={apply}>{busy === "apply" ? "Applying…" : "Apply selected changes to Draft"}</button></div><small>Publishing remains a separate action in Test &amp; Activate.</small></section>
      {active.comparison && <section className={`geometry-comparison ${active.comparison.recommendation}`}><header><div><span className="eyebrow">Independent comparison</span><h3>{title(active.comparison.recommendation)}</h3></div><span>{title(active.comparison.evidence_sufficiency)} evidence</span></header><div><Metrics label="Before" value={active.comparison.baseline} /><Metrics label="After" value={active.comparison.candidate} /></div>{[...active.comparison.reasons, ...active.comparison.regressions].length > 0 && <ul>{[...active.comparison.reasons, ...active.comparison.regressions].map((reason) => <li key={reason}>{reason}</li>)}</ul>}</section>}
    </div>}
    <details className="geometry-calibration-panel">
      <summary><span><strong>Geometry calibration</strong><small>Exact Project, model revision, node configuration, prompt and preprocessing scope</small></span><b>{calibrations.length}</b></summary>
      <div className="improvement-setup-grid"><label>Geometry-producing step<select value={calibrationNode} onChange={(event) => setCalibrationNode(event.target.value)}><option value="">Choose a bound detection or segmentation step</option>{geometryNodes.map((node) => <option key={node.id} value={node.id}>{node.id} · {node.model_binding}</option>)}</select></label><button disabled={Boolean(busy) || !calibrationNode || !evidenceRunIds.length} onClick={calibrate}>{busy === "calibrate" ? "Evaluating…" : "Run geometry calibration"}</button></div>
      <div className="calibration-list">{calibrations.map((view) => <article key={view.report.id}><header><strong>{title(view.effective_status)}</strong><small>{view.report.key.task_id}{view.report.key.label_id ? ` · ${view.report.key.label_id}` : ""}</small></header><dl><div><dt>Samples</dt><dd>{view.report.sample_count}</dd></div><div><dt>Small objects</dt><dd>{view.report.small_object_sample_count}</dd></div><div><dt>Median IoU</dt><dd>{decimal(view.report.median_iou)}</dd></div><div><dt>P10 IoU</dt><dd>{decimal(view.report.p10_iou)}</dd></div><div><dt>Manual adjustment</dt><dd>{percent(view.report.manual_adjustment_rate)}</dd></div></dl>{view.staleness_reasons.length > 0 && <small>Stale because: {view.staleness_reasons.map(title).join(", ")}</small>}</article>)}</div>
      {!calibrations.length && <p>No calibration report exists. Semantic or detector scores alone cannot verify box geometry.</p>}
    </details>
  </section>;
}
