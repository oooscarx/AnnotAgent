import { useCallback, useEffect, useRef, useState } from "react";
import {
  isAbortError,
  workspaceQueries,
  type QueryLoadOptions,
  type QuerySnapshot,
} from "./queryCache";

export interface RouteQueryResult<T> extends QuerySnapshot<T> {
  retry: () => Promise<T | undefined>;
}

export function useRouteQuery<T>(
  key: string | undefined,
  loader: (signal: AbortSignal) => Promise<T>,
  options: QueryLoadOptions = {},
): RouteQueryResult<T> {
  const loaderRef = useRef(loader);
  const optionsRef = useRef(options);
  loaderRef.current = loader;
  optionsRef.current = options;
  const [snapshot, setSnapshot] = useState<QuerySnapshot<T>>(() =>
    key ? workspaceQueries.snapshot<T>(key) : { loading: false, stale: true },
  );

  const execute = useCallback(async (force = false) => {
    if (!key) return undefined;
    try {
      return await workspaceQueries.load(
        key,
        (signal) => loaderRef.current(signal),
        { ...optionsRef.current, force },
      );
    } catch (error) {
      if (!isAbortError(error)) throw error;
      return undefined;
    }
  }, [key]);

  useEffect(() => {
    if (!key) {
      setSnapshot({ loading: false, stale: true });
      return;
    }
    let mounted = true;
    const update = () => {
      if (!mounted) return;
      const next = workspaceQueries.snapshot<T>(key);
      setSnapshot(next);
      if (next.stale && !next.loading && !next.error)
        void execute(true).catch(() => undefined);
    };
    const unsubscribe = workspaceQueries.subscribe(key, update);
    update();
    void execute().catch(() => undefined);
    return () => {
      mounted = false;
      unsubscribe();
      workspaceQueries.abort(key);
    };
  }, [key, execute]);

  return {
    ...snapshot,
    retry: () => execute(true),
  };
}
