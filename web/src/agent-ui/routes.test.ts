import { describe, it, expect } from "vitest";
import { isAgentEntry, routeProject, taskLocation } from "./routes";
const url = (path: string) => new URL(path, "http://localhost");
describe("production Agent routes and retained management", () => {
  it("uses the approved UI for normal entry, not only preview", () => {
    for (const path of ["/", "/projects", "/settings", "/projects/p/work?task=t"]) expect(isAgentEntry(url(path))).toBe(true);
  });
  it("keeps creation, history, restore, and settings management reachable", () => {
    for (const path of ["/projects?new=1", "/projects/p", "/projects/p/runs/r", "/projects/p/build/pipeline", "/trash", "/review/r", "/settings/plugins", "/ui-preview"]) expect(isAgentEntry(url(path))).toBe(false);
  });
  it("never derives project ownership from a display name or another task", () => {
    expect(routeProject(url("/projects/p/work?task=foreign"))).toBe("p");
    expect(routeProject(url("/projects/%broken/work"))).toBe("invalid-project-id");
    expect(taskLocation(url("/projects/old/work?task=t&pane=image"), "new").pathname).toBe("/projects/new/work");
    expect(taskLocation(url("/agent-integration.html?task=t"),"p").pathname).toBe("/agent-integration.html");
  });
});
