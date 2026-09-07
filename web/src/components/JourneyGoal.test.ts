import { describe, expect, it } from "vitest";
import { splitGoalLabels } from "./JourneyGoal";
import { parseWorkspaceRoute, projectJourneyPath, routeFocusKey } from "../navigation";

describe("guided task presentation", () => {
  it("preserves each explicit category without asking an LLM to parse the goal", () => {
    expect(splitGoalLabels("杯子，盘子\nbottle, cup, cup")).toEqual(["杯子", "盘子", "bottle", "cup"]);
    expect(splitGoalLabels(" , \n")).toEqual([]);
  });
  it("keeps exact project, sample and image identity on refresh", () => {
    const path = projectJourneyPath("my project", "samples", { draftId: "d1", sampleTestId: "s1", imageId: "i2" });
    const url = new URL(path, "http://localhost");
    const route = parseWorkspaceRoute(url.pathname, url.search);
    expect(route).toMatchObject({ kind: "journey", projectId: "my project", scene: "samples", draftId: "d1", sampleTestId: "s1", imageId: "i2", canonicalPath: path });
    expect(routeFocusKey(route)).toBe("journey:my project:samples");
    expect(routeFocusKey(parseWorkspaceRoute(url.pathname, "?draft=d1&test=s1&image=i3"))).toBe(routeFocusKey(route));
  });
  it("does not interpret unknown task scenes as another project", () => {
    expect(parseWorkspaceRoute("/projects/p/task/publish").kind).toBe("notFound");
  });
  it("restores a planning session without changing page focus or accepting an external return", () => {
    const path = projectJourneyPath("p1", "goal", { agentSessionId: "session1" });
    expect(parseWorkspaceRoute("/projects/p1/task/goal", "?session=session1&return_to=https://example.com")).toMatchObject({ kind: "journey", agentSessionId: "session1", canonicalPath: path });
    expect(routeFocusKey(parseWorkspaceRoute("/projects/p1/task/goal", "?session=session1"))).toBe(routeFocusKey(parseWorkspaceRoute("/projects/p1/task/goal")));
  });
});
