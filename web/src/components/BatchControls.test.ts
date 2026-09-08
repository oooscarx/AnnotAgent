import { describe, expect, it } from "vitest";
import { batchActions } from "./BatchControls";

describe("existing coordinator controls", () => {
  it("only offers supported explicit actions for runnable states", () => {
    expect(batchActions("running")).toEqual(["pause", "cancel"]);
    expect(batchActions("paused")).toEqual(["resume", "cancel"]);
    expect(batchActions("pending")).toEqual(["resume", "cancel"]);
  });
  it("does not restart review, completed or unknown states", () => {
    for (const status of ["awaiting_review", "completed", "partial", "failed", "cancelled", "budget_exceeded", "unknown"]) {
      expect(batchActions(status)).toEqual([]);
    }
  });
});
