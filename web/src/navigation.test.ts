import { describe, expect, it } from "vitest";
import { parseWorkspaceRoute } from "./navigation";

describe("guided workspace routing", () => {
  it("maps the five primary destinations", () => {
    expect(parseWorkspaceRoute("/").kind).toBe("home");
    expect(parseWorkspaceRoute("/projects").kind).toBe("projects");
    expect(parseWorkspaceRoute("/runs").kind).toBe("runs");
    expect(parseWorkspaceRoute("/review").kind).toBe("review");
    expect(parseWorkspaceRoute("/settings").kind).toBe("settings");
  });

  it("migrates legacy registry routes", () => {
    expect(parseWorkspaceRoute("/dashboard").canonicalPath).toBe("/");
    expect(parseWorkspaceRoute("/models").canonicalPath).toBe("/settings/models");
    expect(parseWorkspaceRoute("/skills").canonicalPath).toBe(
      "/settings/capabilities",
    );
    expect(
      parseWorkspaceRoute("/workflows", "?project_id=alpha").canonicalPath,
    ).toBe("/projects/alpha/build/pipeline");
  });

  it("keeps project build context in the path", () => {
    expect(parseWorkspaceRoute("/projects/demo/build/labels")).toEqual({
      kind: "build",
      projectId: "demo",
      step: "labels",
      canonicalPath: "/projects/demo/build/labels",
    });
  });

  it("restores Run artifact context from the URL", () => {
    expect(
      parseWorkspaceRoute(
        "/runs/run-1",
        "?view=debug&image=3&node=detector&artifact=det-1",
      ),
    ).toMatchObject({
      kind: "runs",
      runId: "run-1",
      imageId: "3",
      nodeId: "detector",
      artifactId: "det-1",
      view: "debug",
    });
  });

  it("keeps global Project filters explicit in the URL", () => {
    expect(parseWorkspaceRoute("/runs", "?project_id=alpha")).toMatchObject({
      kind: "runs",
      projectId: "alpha",
      canonicalPath: "/runs?project_id=alpha",
    });
    expect(parseWorkspaceRoute("/review", "?project_id=alpha")).toMatchObject({
      kind: "review",
      projectId: "alpha",
      canonicalPath: "/review?project_id=alpha",
    });
    expect(parseWorkspaceRoute("/runs")).not.toHaveProperty("projectId", "alpha");
    expect(parseWorkspaceRoute("/review")).not.toHaveProperty("projectId", "alpha");
  });
});
