import { describe, it, expect } from "vitest";
import { isAgentEntry, routeProject, taskLocation } from "./routes";
const url = (path: string) => new URL(path, "http://localhost");
describe("production Agent routes and retained management", () => {
  it("uses the approved UI for normal entry, not only preview", () => {
    for (const path of ["/", "/projects", "/settings", "/projects/p/work?task=t"]) expect(isAgentEntry(url(path))).toBe(true);
  });
  it("keeps creation, history, restore, and settings management reachable", () => {
    for (const path of ["/projects?new=1", "/projects/p", "/projects/p/runs/r", "/projects/p/build/pipeline", "/trash", "/review/r", "/ui-preview"]) expect(isAgentEntry(url(path))).toBe(false);
  });
  it("never sends Settings subpages or invalid Settings URLs to the old App",()=>{
    for(const path of ["/settings/plugins","/settings/storage","/settings/agent-models","/settings/models","/settings/unknown"]) expect(isAgentEntry(url(path))).toBe(true);
  });
  it("uses native project creation, data and label management",()=>{
    for(const path of ["/projects/new","/projects/p/manage/data","/projects/p/manage/labels"]) expect(isAgentEntry(url(path))).toBe(true);
  });
  it("never derives project ownership from a display name or another task", () => {
    expect(routeProject(url("/projects/p/work?task=foreign"))).toBe("p");
    expect(routeProject(url("/projects/%broken/work"))).toBe("invalid-project-id");
    expect(taskLocation(url("/projects/old/work?task=t&pane=image"), "new").pathname).toBe("/projects/new/work");
    expect(taskLocation(url("/agent-integration.html?task=t"),"p").pathname).toBe("/agent-integration.html");
  });
  it("does not carry another project's task or image into a new workspace",()=>{
    expect(taskLocation(url("/projects/p/work?task=t&image=i&pane=image"),"other").search).toBe("");
    expect(taskLocation(url("/settings/plugins?return_project=p&return_task=t&workspace_return=/projects/p/work"),"p").search).toBe("");
    expect(taskLocation(url("/projects/p/work?task=t&image=i&pane=image"),"p").search).toBe("?task=t&image=i&pane=image");
  });
});
