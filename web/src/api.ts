import type {
  Annotation,
  DashboardData,
  HistoryRun,
  ImageItem,
  ModelBinding,
  ProjectWorkflow,
  ProjectSummary,
  ReviewItem,
  RunEvent,
  SkillDetail,
  WorkflowDraft,
  WorkflowCatalog,
  WorkflowDryRunReport,
  WorkflowVersionComparison,
} from "./types";

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: { "content-type": "application/json", ...init?.headers },
  });
  if (!response.ok) {
    const body = (await response.json().catch(() => ({}))) as {
      error?: string;
      code?: string;
      active_run_id?: string;
      status?: string;
    };
    const active =
      body.code === "active_run_exists"
        ? `Project already has active Run ${body.active_run_id ?? "unknown"} (${body.status ?? "active"}).`
        : undefined;
    throw new Error(
      active ?? body.error ?? `${response.status} ${response.statusText}`,
    );
  }
  return response.json() as Promise<T>;
}

export const api = {
  health: () => request<{ status: string }>("/api/health"),
  dashboard: () => request<DashboardData>("/api/projects"),
  createProject: (id: string, yaml: string) =>
    request<ProjectSummary>("/api/projects", {
      method: "POST",
      body: JSON.stringify({ id, yaml }),
    }),
  images: (projectId: string) =>
    request<{ images: ImageItem[] }>(`/api/projects/${projectId}/images`),
  startRun: (
    projectId: string,
    provider?: string,
    idempotencyKey = crypto.randomUUID(),
    workflow?: { workflow_id: string; version: number },
  ) =>
    request<{
      run_id: string;
      image_path: string;
      status: string;
      idempotent: boolean;
    }>(`/api/projects/${projectId}/runs`, {
      method: "POST",
      headers: { "idempotency-key": idempotencyKey },
      body: JSON.stringify({ ...(provider ? { provider } : {}), ...workflow }),
    }),
  control: (runId: string, action: "pause" | "resume" | "cancel") =>
    request(`/api/runs/${runId}/${action}`, { method: "POST" }),
  runEvents: (runId: string) =>
    request<{ events: RunEvent[] }>(`/api/runs/${runId}/events`),
  runs: () => request<{ runs: HistoryRun[] }>("/api/runs"),
  workflows: () => request<{ workflows: ProjectWorkflow[] }>("/api/workflows"),
  workflowDrafts: (projectId?: string) =>
    request<{ drafts: WorkflowDraft[] }>(
      `/api/workflow-drafts${projectId ? `?project_id=${encodeURIComponent(projectId)}` : ""}`,
    ),
  workflowCatalog: (projectId: string) =>
    request<WorkflowCatalog>(`/api/projects/${projectId}/workflow-catalog`),
  createWorkflowDraft: (
    projectId: string,
    fromTemplate = false,
    templateId?: string,
  ) =>
    request<WorkflowDraft>("/api/workflow-drafts", {
      method: "POST",
      body: JSON.stringify({
        project_id: projectId,
        from_template: fromTemplate,
        ...(templateId ? { template_id: templateId } : {}),
      }),
    }),
  suggestWorkflow: (projectId: string, advisor: "mock" | "llm" = "mock") =>
    request<{
      draft: WorkflowDraft;
      rationale: string[];
      warnings: string[];
      alternatives: string[];
      unresolved_model_bindings: string[];
    }>("/api/workflow-drafts/suggest", {
      method: "POST",
      body: JSON.stringify({
        project_id: projectId,
        advisor,
        constraints: { require_review_gate: true },
      }),
    }),
  saveWorkflowDraft: (draft: WorkflowDraft) =>
    request<WorkflowDraft>(`/api/workflow-drafts/${draft.id}`, {
      method: "PATCH",
      body: JSON.stringify(draft),
    }),
  dryRunWorkflow: (draftId: string, imageIndices: number[] = []) =>
    request<WorkflowDryRunReport>(`/api/workflow-drafts/${draftId}/dry-run`, {
      method: "POST",
      body: JSON.stringify({ image_indices: imageIndices }),
    }),
  publishWorkflow: (draftId: string) =>
    request<{ workflow_id: string; version: number }>(
      `/api/workflow-drafts/${draftId}/publish`,
      { method: "POST" },
    ),
  archiveWorkflowDraft: (draftId: string) =>
    request<WorkflowDraft>(`/api/workflow-drafts/${draftId}/archive`, {
      method: "POST",
    }),
  cloneWorkflowVersion: (workflowId: string, version: number) =>
    request<WorkflowDraft>(
      `/api/workflows/${workflowId}/versions/${version}/clone`,
      { method: "POST" },
    ),
  compareWorkflowVersions: (
    left: { workflow_id: string; version: number },
    right: { workflow_id: string; version: number },
  ) =>
    request<WorkflowVersionComparison>("/api/workflows/compare", {
      method: "POST",
      body: JSON.stringify({
        left_workflow_id: left.workflow_id,
        left_version: left.version,
        right_workflow_id: right.workflow_id,
        right_version: right.version,
      }),
    }),
  models: () => request<{ models: ModelBinding[] }>("/api/models"),
  reviews: () => request<{ reviews: ReviewItem[] }>("/api/reviews"),
  review: (id: string) => request<ReviewItem>(`/api/reviews/${id}`),
  revise: (annotation: Annotation, reason: string) =>
    request(`/api/annotations/${annotation.id}`, {
      method: "PATCH",
      body: JSON.stringify({ annotation, reason }),
    }),
  decide: (
    id: string,
    projectId: string,
    decision: "accept" | "reject" | "delete",
    reasonCode: string,
    note: string,
  ) =>
    request(`/api/reviews/${id}/decision`, {
      method: "POST",
      body: JSON.stringify({
        project_id: projectId,
        decision,
        reason_code: reasonCode,
        note,
      }),
    }),
  revisions: (id: string) =>
    request<{ revisions: unknown[] }>(`/api/annotations/${id}/revisions`),
  skills: () => request<SkillDetail[]>("/api/skills"),
  settings: () => request<Record<string, unknown>>("/api/settings"),
  saveSettings: (value: Record<string, unknown>) =>
    request<Record<string, unknown>>("/api/settings", {
      method: "PUT",
      body: JSON.stringify(value),
    }),
  export: (projectId: string, format: string) =>
    request(`/api/projects/${projectId}/export`, {
      method: "POST",
      body: JSON.stringify({ format }),
    }),
};

export function subscribeEvents(
  onEvent: (event: RunEvent) => void,
  onReconnect: () => void,
): () => void {
  const source = new EventSource("/api/events");
  const kinds = [
    "run_created",
    "run_started",
    "task_started",
    "task_completed",
    "task_failed",
    "model_call_started",
    "model_call_completed",
    "model_call_failed",
    "tool_call_started",
    "tool_call_completed",
    "validation_completed",
    "refinement_started",
    "refinement_completed",
    "artifact_created",
    "artifact_validated",
    "artifact_committed",
    "retry_scheduled",
    "annotation_committed",
    "review_requested",
    "usage_updated",
    "run_paused",
    "run_resumed",
    "run_cancelled",
    "run_budget_exceeded",
    "run_completed",
    "run_failed",
    "run_interrupted",
  ];
  for (const kind of kinds) {
    source.addEventListener(kind, (message) => {
      onEvent(JSON.parse((message as MessageEvent<string>).data) as RunEvent);
    });
  }
  source.onerror = () => onReconnect();
  return () => source.close();
}
