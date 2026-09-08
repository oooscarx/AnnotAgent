import { api } from "../api";
import { parseWorkspaceRoute, projectBatchPath, type ConversationResultsContext } from "../navigation";
import { queryKeys } from "../queryCache";
import { useRouteQuery } from "../useRouteQuery";
import type { ProjectSummary } from "../types";
import { JourneyBatch } from "./JourneyBatch";

/** Same terminal projection and editor as the canonical Batch page, with a workspace URL. */
export function ConversationBatchResults({ project, context, onSelect, onNavigate, onNavigationGuardChange }: {
  project: ProjectSummary; context: ConversationResultsContext;
  onSelect: (context: ConversationResultsContext) => void;
  onNavigate: (path: string) => void;
  onNavigationGuardChange: (guard?: () => boolean) => void;
}) {
  const query = useRouteQuery(queryKeys.batch(context.batchId), signal => api.batch(context.batchId, signal));
  const batch = query.data?.batch;
  if (!batch || batch.id !== context.batchId || batch.project_id !== project.id) return <section role="status">
    <p>{query.loading ? "Loading formal processing results…" : query.error?.message ?? "This Batch does not belong to this Project or is unavailable. No other result was substituted."}</p>
    {query.error && <button onClick={() => void query.retry().catch(() => undefined)}>Reload results</button>}
  </section>;
  const navigate = (path: string) => {
    const url = new URL(path, window.location.origin);
    const route = parseWorkspaceRoute(url.pathname, url.search);
    if (route.kind === "projectBatch" && route.projectId === project.id && route.batchId === context.batchId) {
      onSelect({ batchId: route.batchId, imageId: route.imageId, status: route.status, annotationId: route.annotationId, canvasView: route.canvasView });
    } else onNavigate(path);
  };
  return <div className="conversation-formal-results">
    <p className="journey-risk">Formal dataset results · Pending candidates still require review. This is separate from Sample Test corrections.</p>
    <JourneyBatch project={project} batch={batch} route={{kind:"projectBatch",projectId:project.id,...context,canonicalPath:projectBatchPath(project.id,batch.id,context)}} onNavigate={navigate} onReload={query.retry} onNavigationGuardChange={onNavigationGuardChange} />
  </div>;
}
