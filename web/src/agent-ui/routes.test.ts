import { describe, it, expect } from "vitest";
import { isAgentEntry, routeProject, taskLocation, managementReturn, retainManagementContext } from "./routes";
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
  it("never derives project ownership from a display name or another task", () => {
    expect(routeProject(url("/projects/p/work?task=foreign"))).toBe("p");
    expect(routeProject(url("/projects/%broken/work"))).toBe("invalid-project-id");
    expect(taskLocation(url("/projects/old/work?task=t&pane=image"), "new").pathname).toBe("/projects/new/work");
    expect(taskLocation(url("/agent-integration.html?task=t"),"p").pathname).toBe("/agent-integration.html");
  });
  it("retains only typed same-owner management return keys through canonicalization",()=>{
    const id="6754265a-0cb2-4df3-96e5-ca67fe63e4ab";
    const source=url(`/projects/p?return_task=${id}&return_pane=image&return_to=https://evil.invalid`);
    const canonical=retainManagementContext(url("/projects/p"),source);
    expect(canonical.searchParams.get("return_task")).toBe(id);
    expect(canonical.searchParams.has("return_to")).toBe(false);
    expect(managementReturn(url("/projects/p/work"),canonical).searchParams.get("task")).toBe(id);
    expect(managementReturn(url("/projects/p/work"),url(`/projects/p-other?return_task=${id}`)).search).toBe("");
  });
});
