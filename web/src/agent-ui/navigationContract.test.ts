import { describe, expect, it } from "vitest";
import { agentPath, parseAgentRoute, settingsPages, managementPages } from "./navigationContract";
import entry from "../main.tsx?raw";
const parse = (path: string) => parseAgentRoute(new URL(path, "http://localhost"));
describe("single UI route contract (cutover target)", () => {
  it("round-trips every new Settings page without a project or task", () => {
    for (const page of settingsPages) expect(parse(agentPath({kind:"settings",page}))).toEqual({kind:"settings",page});
  });
  it("names management by stable owner and object IDs, never display names", () => {
    for (const page of managementPages) expect(parse(agentPath({kind:"management",projectId:"p",page}))).toEqual({kind:"management",projectId:"p",page});
    for (const page of ["pipelines","runs","batches","review"] as const) expect(parse(agentPath({kind:"detail",projectId:"p",page,objectId:"uuid"}))).toEqual({kind:"detail",projectId:"p",page,objectId:"uuid"});
  });
  it("does not alias legacy pages, hashes, or malformed identifiers", () => {
    for (const path of ["/settings", "/settings/models", "/runs", "/review/r", "/trash", "/projects/p", "/projects/p/build/test", "/projects/p/runs/r", "/projects/p/work#legacy", "/projects/%broken/work", "/projects/a%2Fb/work", "/projects/p/manage/runs/a%5Cb", "/unknown"]) expect(parse(path)).toEqual({kind:"not-found"});
  });
  it("ignores query-string legacy return protocols rather than granting them ownership", () => {
    expect(parse("/settings/plugins?return_project=p&return_task=t&workspace_return=https://evil.invalid")).toEqual({kind:"settings",page:"plugins"});
    expect(parse("/projects?new=1")).toEqual({kind:"projects"});
    expect(parse("/projects/new")).toEqual({kind:"create-project"});
    expect(()=>agentPath({kind:"work",projectId:"a/b"})).toThrow();
  });
  it("production entry always loads the native HTTP application, never old root/styles or fixture",()=>{
    expect(entry).toContain('import("./agent-ui/App")');expect(entry).toContain('import("./agent-ui/http")');
    expect(entry).not.toMatch(/import\(["']\.\/(App|styles\.css)["']\)/);
    expect(entry).not.toMatch(/isAgentEntry|FixtureAdapter|fixture\.ts/);
  });
});
