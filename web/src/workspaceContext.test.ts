import { describe, expect, it } from "vitest";
import type { HistoryRun, ProjectSummary, ReviewItem } from "./types";
import { projectForReview, projectForRun, runsForContext } from "./workspaceContext";

const project = { id: "ball", name: "Ball Project" } as ProjectSummary;
const other = { id: "other", name: "Other Project" } as ProjectSummary;
const run = { id: "run-1", project_name: project.name, status: "completed" } as HistoryRun;
const running = { id: "run-2", project_name: project.name, status: "running" } as HistoryRun;
const foreign = { id: "run-3", project_name: other.name, status: "completed" } as HistoryRun;

describe("workspace Project context", () => {
  it("recovers a Project from a Run", () => {
    expect(projectForRun([project, other], run)?.id).toBe(project.id);
  });

  it("prefers the stable Project id for Review items", () => {
    const review = { project_id: project.id, project_name: "Old name" } as ReviewItem;
    expect(projectForReview([project, other], review)?.id).toBe(project.id);
  });

  it("shows every Run unless an explicit Project scope is supplied", () => {
    expect(runsForContext([run, running, foreign]).map((item) => item.id)).toEqual([
      "run-1",
      "run-2",
      "run-3",
    ]);
    expect(runsForContext([run, running, foreign], project).map((item) => item.id)).toEqual(["run-1", "run-2"]);
    expect(runsForContext([run, running, foreign], project, "running").map((item) => item.id)).toEqual(["run-2"]);
  });
});
