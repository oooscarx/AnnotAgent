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
} from "./types";

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: { "content-type": "application/json", ...init?.headers },
  });
  if (!response.ok) {
    const body = (await response.json().catch(() => ({}))) as { error?: string };
    throw new Error(body.error ?? `${response.status} ${response.statusText}`);
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
  startRun: (projectId: string, provider?: string) =>
    request<{ run_id: string; image_path: string }>(
      `/api/projects/${projectId}/runs`,
      {
        method: "POST",
        body: JSON.stringify(provider ? { provider } : {}),
      },
    ),
  control: (runId: string, action: "pause" | "resume" | "cancel") =>
    request(`/api/runs/${runId}/${action}`, { method: "POST" }),
  runEvents: (runId: string) =>
    request<{ events: RunEvent[] }>(`/api/runs/${runId}/events`),
  runs: () => request<{ runs: HistoryRun[] }>("/api/runs"),
  workflows: () => request<{ workflows: ProjectWorkflow[] }>("/api/workflows"),
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

export function subscribeEvents(onEvent: (event: RunEvent) => void): () => void {
  const source = new EventSource("/api/events");
  const kinds = [
    "run_created",
    "run_started",
    "task_started",
    "model_call_started",
    "model_call_completed",
    "tool_call_started",
    "tool_call_completed",
    "validation_completed",
    "refinement_started",
    "refinement_completed",
    "retry_scheduled",
    "annotation_committed",
    "review_requested",
    "usage_updated",
    "run_paused",
    "run_resumed",
    "run_cancelled",
    "run_completed",
    "run_failed",
  ];
  for (const kind of kinds) {
    source.addEventListener(kind, (message) => {
      onEvent(JSON.parse((message as MessageEvent<string>).data) as RunEvent);
    });
  }
  return () => source.close();
}
