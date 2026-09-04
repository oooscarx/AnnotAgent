import { describe, expect, it } from "vitest";
import type { HistoryRun, ProjectSummary, ReviewItem } from "./types";
import { projectForReview, projectForRun, runsForContext } from "./workspaceContext";

const project = { id: "ball", project_id: "ball", name: "Ball Project" } as ProjectSummary;
const other = { id: "other", project_id: "other", name: "Other Project" } as ProjectSummary;
const run = { id: "run-1", project_id: project.project_id, project_name: project.name, status: "completed" } as HistoryRun;
const running = { id: "run-2", project_id: project.project_id, project_name: project.name, status: "running" } as HistoryRun;
const foreign = { id: "run-3", project_id: other.project_id, project_name: other.name, status: "completed" } as HistoryRun;

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

  it("keeps duplicate Project names isolated by stable Run.project_id", () => {
    const first = { id: "project-a", project_id: "project-a", name: "Duplicate" } as ProjectSummary;
    const second = { id: "project-b", project_id: "project-b", name: "Duplicate" } as ProjectSummary;
    const ownedRun = {
      id: "run-b",
      project_id: second.id,
      project_name: "Duplicate",
      status: "completed",
    } as unknown as HistoryRun;

    expect(projectForRun([first, second], ownedRun)?.id).toBe(second.id);
    expect(runsForContext([ownedRun], first)).toEqual([]);
    expect(runsForContext([ownedRun], second)).toEqual([ownedRun]);
  });

  it("does not lose Run ownership when a Project is renamed", () => {
    const renamed = { id: "project-a", project_id: "project-a", name: "Renamed" } as ProjectSummary;
    const historicalRun = {
      id: "run-a",
      project_id: renamed.id,
      project_name: "Old name",
      status: "completed",
    } as unknown as HistoryRun;

    expect(projectForRun([renamed], historicalRun)?.id).toBe(renamed.id);
    expect(runsForContext([historicalRun], renamed)).toEqual([historicalRun]);
  });
});
