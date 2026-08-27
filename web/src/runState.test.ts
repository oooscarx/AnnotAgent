import { describe, expect, it } from "vitest";

import { deriveProjectRunView } from "./runState";
import type { ProjectSummary } from "./types";

describe("Project navigation run restore", () => {
  it("restores the active run from backend Project state and disables Start", () => {
    const project = {
      id: "generic-project",
      active_run: {
        id: "run-from-server",
        provider: "mock",
        model: "mock",
        status: "running",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:01Z",
      },
    } as ProjectSummary;

    expect(deriveProjectRunView(undefined).startDisabled).toBe(false); // navigated away
    const restored = deriveProjectRunView(project); // navigated back after API refresh
    expect(restored.activeRunId).toBe("run-from-server");
    expect(restored.activeStatus).toBe("running");
    expect(restored.startDisabled).toBe(true);
  });

  it("also locks Start for a persisted active Dataset Batch", () => {
    const project = {
      active_batch: { id: "batch-1", status: "paused" },
    } as ProjectSummary;
    expect(deriveProjectRunView(project)).toMatchObject({
      activeRunId: "",
      activeBatchId: "batch-1",
      activeStatus: "paused",
      startDisabled: true,
    });
  });
});
