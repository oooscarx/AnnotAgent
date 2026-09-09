import { expect, it } from "vitest";
import { assertInspection } from "./RunInspector";
import type { RunNodeArtifactInspection } from "../types";
import source from "./RunInspector.tsx?raw";
it("rejects foreign project or run artifact responses",()=>{
  const value={project_id:"p",run_id:"r"} as RunNodeArtifactInspection;
  expect(assertInspection(value,"p","r")).toBe(value);
  expect(()=>assertInspection(value,"other","r")).toThrow();
  expect(()=>assertInspection(value,"p","other")).toThrow();
});
it("keeps saved inspection independent of old root and legacy synchronous Replay",()=>{
  expect(source).not.toContain("../App");expect(source).not.toContain("replayNode");
  expect(source).toContain("controller.abort()");expect(source).toContain("popstate");
  expect(source).toContain("<NodeReplay");
});
