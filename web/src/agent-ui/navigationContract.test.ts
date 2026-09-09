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
  it("rejects retired Journey, registry, global history and query-owned legacy routes",()=>{
    for(const path of ["/settings/registry","/settings/capabilities","/models","/plugins","/runs/r","/review","/projects/p/export","/projects/p/review/r","/projects/p/batches/b","/projects/p/trash","/projects/p/build/data","/projects/p/build/labels","/projects/p/build/pipeline?draft=d","/projects/p/build/test?draft=d&test=t","/projects/p/task/goal","/projects/p/task/images","/projects/p/task/model","/projects/p/task/samples","/projects/p/task/confirm","/projects/p/task/revise","/runs/r?project=p"]){
      expect(parse(path),path).toEqual({kind:"not-found"});
    }
  });
  it("native ownership is unchanged by retired nested return and legacy object keys",()=>{
    const query="?return_project=foreign&workspace_return=%2Fprojects%2Fforeign%2Fwork&draft=old&test=old&result_image=old";
    expect(parse("/projects/p/manage/review/r"+query)).toEqual({kind:"detail",projectId:"p",page:"review",objectId:"r"});
    expect(parse("/projects/p/work"+query)).toEqual({kind:"work",projectId:"p"});
  });
});
