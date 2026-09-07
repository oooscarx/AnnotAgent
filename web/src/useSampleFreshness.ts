import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import { isAbortError, workspaceQueries } from "./queryCache";

// Revalidate identity, never replace the user's in-progress canvas or feedback.
export function useSampleFreshness(projectId: string, draftId: string, testId: string) {
  const [status, setStatus] = useState<"checking" | "current" | "stale" | "unavailable">("checking");
  const [attempt, setAttempt] = useState(0);
  const retry = useCallback(() => setAttempt((value) => value + 1), []);
  useEffect(() => {
    let active = true;
    const key = `sample-freshness/${projectId}/${draftId}/${testId}`;
    const check = () => {
      if (document.visibilityState === "hidden") return;
      setStatus("checking");
      void workspaceQueries.load(key, (signal) => api.workflowSampleTest(draftId, signal, testId))
        .then(({ sample_test: sample, current }) => {
          if (active) setStatus(current && sample?.id === testId && sample.project_id === projectId && sample.draft_id === draftId ? "current" : "stale");
        })
        .catch((error: unknown) => { if (active && !isAbortError(error)) setStatus("unavailable"); });
    };
    check();
    window.addEventListener("focus", check);
    document.addEventListener("visibilitychange", check);
    return () => {
      active = false;
      window.removeEventListener("focus", check);
      document.removeEventListener("visibilitychange", check);
      workspaceQueries.abort(key);
    };
  }, [projectId, draftId, testId, attempt]);
  return { status, retry };
}
