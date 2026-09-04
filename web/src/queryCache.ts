export interface QuerySnapshot<T> {
  data?: T;
  error?: Error;
  loading: boolean;
  stale: boolean;
  updatedAt?: number;
}

export interface QueryLoadOptions {
  force?: boolean;
  staleTime?: number;
}

interface QueryEntry<T> extends QuerySnapshot<T> {
  generation: number;
  controller?: AbortController;
  promise?: Promise<T>;
}

type QueryListener = () => void;

function abortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === "AbortError";
}

/**
 * A small route-resource cache. It deliberately owns request generations and
 * AbortControllers so a response started for an older route can never replace
 * the resource selected by a newer URL.
 */
export class RouteQueryCache {
  private readonly entries = new Map<string, QueryEntry<unknown>>();
  private readonly listeners = new Map<string, Set<QueryListener>>();

  snapshot<T>(key: string): QuerySnapshot<T> {
    const entry = this.entries.get(key) as QueryEntry<T> | undefined;
    return entry
      ? {
          data: entry.data,
          error: entry.error,
          loading: entry.loading,
          stale: entry.stale,
          updatedAt: entry.updatedAt,
        }
      : { loading: false, stale: true };
  }

  subscribe(key: string, listener: QueryListener): () => void {
    const listeners = this.listeners.get(key) ?? new Set<QueryListener>();
    listeners.add(listener);
    this.listeners.set(key, listeners);
    return () => {
      listeners.delete(listener);
      if (!listeners.size) this.listeners.delete(key);
    };
  }

  async load<T>(
    key: string,
    loader: (signal: AbortSignal) => Promise<T>,
    options: QueryLoadOptions = {},
  ): Promise<T> {
    const staleTime = options.staleTime ?? 0;
    const now = Date.now();
    const existing = this.entries.get(key) as QueryEntry<T> | undefined;
    if (
      !options.force &&
      existing?.data !== undefined &&
      !existing.stale &&
      now - (existing.updatedAt ?? 0) <= staleTime
    )
      return existing.data;
    if (!options.force && existing?.promise) return existing.promise;

    if (existing?.controller) existing.controller.abort();
    const controller = new AbortController();
    const generation = (existing?.generation ?? 0) + 1;
    const entry: QueryEntry<T> = {
      data: existing?.data,
      error: undefined,
      loading: true,
      stale: existing?.data !== undefined,
      updatedAt: existing?.updatedAt,
      generation,
      controller,
    };
    this.entries.set(key, entry as QueryEntry<unknown>);
    this.emit(key);

    const promise = loader(controller.signal)
      .then((data) => {
        const current = this.entries.get(key) as QueryEntry<T> | undefined;
        if (current?.generation === generation) {
          this.entries.set(key, {
            data,
            loading: false,
            stale: false,
            updatedAt: Date.now(),
            generation,
          });
          this.emit(key);
        }
        return data;
      })
      .catch((error: unknown) => {
        const current = this.entries.get(key) as QueryEntry<T> | undefined;
        if (current?.generation === generation) {
          this.entries.set(key, {
            data: current.data,
            error: abortError(error) ? undefined : error as Error,
            loading: false,
            stale: true,
            updatedAt: current.updatedAt,
            generation,
          });
          this.emit(key);
        }
        throw error;
      });
    entry.promise = promise;
    return promise;
  }

  invalidate(keyOrPrefix: string): void {
    for (const [key, entry] of this.entries) {
      if (key !== keyOrPrefix && !key.startsWith(`${keyOrPrefix}/`)) continue;
      entry.stale = true;
      entry.error = undefined;
      this.emit(key);
    }
  }

  abort(key: string): void {
    const entry = this.entries.get(key);
    if (!entry?.controller) return;
    entry.controller.abort();
    entry.controller = undefined;
    entry.promise = undefined;
    entry.loading = false;
    entry.generation += 1;
    this.emit(key);
  }

  clear(): void {
    for (const entry of this.entries.values()) entry.controller?.abort();
    this.entries.clear();
    for (const key of this.listeners.keys()) this.emit(key);
  }

  private emit(key: string): void {
    for (const listener of this.listeners.get(key) ?? []) listener();
  }
}

export const workspaceQueries = new RouteQueryCache();

export const queryKeys = {
  dashboard: "workspace/dashboard",
  projects: "projects",
  project: (projectId: string) => `project/${projectId}`,
  projectSummary: (projectId: string) => `project/${projectId}/summary`,
  projectRuns: (projectId: string) => `project/${projectId}/runs`,
  projectImages: (projectId: string) => `project/${projectId}/images`,
  run: (runId: string) => `run/${runId}`,
  runResults: (runId: string) => `run/${runId}/results`,
  runDebug: (runId: string) => `run/${runId}/debug`,
  runAnnotations: (runId: string) => `run/${runId}/annotations`,
  reviewQueue: (projectId?: string) => `review-queue/${projectId ?? "global"}`,
  review: (reviewId: string) => `review/${reviewId}`,
  workflowDrafts: (projectId: string) => `project/${projectId}/workflow-drafts`,
  workflowDraft: (draftId: string) => `workflow-draft/${draftId}`,
  sampleTest: (draftId: string) => `sample-test/${draftId}`,
  agentSessions: (projectId: string) => `project/${projectId}/agent-sessions`,
  improvementSessions: (projectId: string) => `project/${projectId}/improvement-sessions`,
};

export function isAbortError(error: unknown): boolean {
  return abortError(error);
}
