import { describe, it, expect } from "vitest";
import { routeProject, taskLocation } from "./routes";
const url = (path: string) => new URL(path, "http://localhost");
describe("production task navigation uses the single native route tree", () => {
  it("never derives project ownership from a display name or another task", () => {
    expect(routeProject(url("/projects/p/work?task=foreign"))).toBe("p");
    expect(routeProject(url("/projects/%broken/work"))).toBe("invalid-project-id");
    expect(taskLocation(url("/projects/old/work?task=t&pane=image"), "new").pathname).toBe("/projects/new/work");
    expect(taskLocation(url("/unknown?task=t#old"),"p").pathname).toBe("/projects/p/work");
    expect(taskLocation(url("/unknown?task=t#old"),"p").hash).toBe("");
  });
  it("does not carry another project's task or image into a new workspace",()=>{
    expect(taskLocation(url("/projects/p/work?task=t&image=i&pane=image"),"other").search).toBe("");
    expect(taskLocation(url("/settings/plugins?return_project=p&return_task=t&workspace_return=/projects/p/work"),"p").search).toBe("");
    expect(taskLocation(url("/projects/p/work?task=t&image=i&pane=image"),"p").search).toBe("?task=t&image=i&pane=image");
  });
});
