import { describe, expect, it } from "vitest";
import { deriveProjectNextAction } from "./projectWorkspace";
import type { ProjectSummary } from "./types";

const project = {
  image_count: 5,
  task_count: 1,
  annotation_schema: [{ labels: ["thing"] }],
  blocking_issues: [],
  readiness: "ready",
  review_count: 0,
} as unknown as ProjectSummary;

describe("Project Workspace next action", () => {
  it("prioritizes active server work over starting again", () => {
    expect(
      deriveProjectNextAction({
        ...project,
        active_run: { id: "run-1" },
      } as ProjectSummary),
    ).toEqual({ kind: "active_run", runId: "run-1", label: "Open active run" });
  });

  it("guides incomplete setup in order", () => {
    expect(deriveProjectNextAction({ ...project, image_count: 0 }).label).toBe(
      "Add images",
    );
    expect(
      deriveProjectNextAction({
        ...project,
        task_count: 0,
        annotation_schema: [],
      }).label,
    ).toBe("Define labels");
  });

  it("uses backend blocking issues and review counts", () => {
    expect(
      deriveProjectNextAction({
        ...project,
        readiness: "configuration_issue",
        blocking_issues: [
          { code: "invalid", message: "Invalid", next_step: "pipeline" },
        ],
      }).label,
    ).toBe("Fix pipeline");
    expect(deriveProjectNextAction({ ...project, review_count: 2 }).label).toBe(
      "Review results",
    );
  });
});
