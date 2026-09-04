import { describe, expect, it } from "vitest";
import {
  parseWorkspaceRoute,
  projectBuildPath,
  projectBatchPath,
  projectReviewPath,
  projectRunPath,
  projectRunsPath,
  routeFocusKey,
} from "./navigation";

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
    expect(parseWorkspaceRoute("/providers").canonicalPath).toBe("/settings");
    expect(parseWorkspaceRoute("/settings/providers").canonicalPath).toBe(
      "/settings",
    );
    expect(parseWorkspaceRoute("/skills").canonicalPath).toBe(
      "/settings/plugins",
    );
    expect(
      parseWorkspaceRoute("/workflows", "?project_id=alpha").canonicalPath,
    ).toBe("/projects/alpha/build/pipeline");
    expect(parseWorkspaceRoute("/artifacts/artifact-1").canonicalPath).toBe(
      "/runs?view=debug&artifact=artifact-1",
    );
    expect(
      parseWorkspaceRoute("/artifact-inspector", "?project=alpha").canonicalPath,
    ).toBe("/projects/alpha/runs");
  });

  it("keeps Expert Model Plugins as a durable Settings destination", () => {
    expect(parseWorkspaceRoute("/settings/plugins")).toEqual({
      kind: "settings",
      section: "plugins",
      canonicalPath: "/settings/plugins",
    });
  });

  it("keeps project build context in the path", () => {
    expect(parseWorkspaceRoute("/projects/demo/build/labels")).toEqual({
      kind: "build",
      projectId: "demo",
      step: "labels",
      canonicalPath: "/projects/demo/build/labels",
    });
  });

  it("keeps the selected Draft in the Test page URL", () => {
    expect(
      parseWorkspaceRoute(
        "/projects/demo/build/test",
        "?draft=draft with spaces",
      ),
    ).toEqual({
      kind: "build",
      projectId: "demo",
      step: "test",
      draftId: "draft with spaces",
      canonicalPath: "/projects/demo/build/test?draft=draft%20with%20spaces",
    });
    expect(
      parseWorkspaceRoute(
        "/projects/demo/build/labels",
        "?draft=ignored",
      ),
    ).not.toHaveProperty("draftId", "ignored");
  });

  it("keeps the guided export destination in Project context", () => {
    expect(parseWorkspaceRoute("/projects/demo/export")).toEqual({
      kind: "export",
      projectId: "demo",
      canonicalPath: "/projects/demo/export",
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

  it("migrates query-scoped legacy links into the Project hierarchy", () => {
    expect(parseWorkspaceRoute("/runs", "?project_id=alpha&status=failed")).toMatchObject({
      kind: "projectRuns",
      projectId: "alpha",
      status: "failed",
      canonicalPath: "/projects/alpha/runs?status=failed",
    });
    expect(parseWorkspaceRoute("/review", "?project_id=alpha")).toMatchObject({
      kind: "projectReview",
      projectId: "alpha",
      canonicalPath: "/projects/alpha/review",
    });
    expect(parseWorkspaceRoute("/review/item-1", "?project_id=alpha")).toMatchObject({
      kind: "projectReview",
      reviewItemId: "item-1",
      projectId: "alpha",
      canonicalPath: "/projects/alpha/review/item-1",
    });
    expect(parseWorkspaceRoute("/runs")).not.toHaveProperty("projectId", "alpha");
    expect(parseWorkspaceRoute("/review")).not.toHaveProperty("projectId", "alpha");
  });

  it("models the Project-owned Run, Batch, and Review hierarchy in paths", () => {
    expect(parseWorkspaceRoute("/projects/project-a/runs")).toMatchObject({
      kind: "projectRuns",
      projectId: "project-a",
    });
    expect(parseWorkspaceRoute("/projects/project-a/runs/run-1")).toMatchObject({
      kind: "projectRun",
      projectId: "project-a",
      runId: "run-1",
    });
    expect(parseWorkspaceRoute("/projects/project-a/batches/batch-1")).toMatchObject({
      kind: "projectBatch",
      projectId: "project-a",
      batchId: "batch-1",
    });
    expect(parseWorkspaceRoute("/projects/project-a/review/review-1")).toMatchObject({
      kind: "projectReview",
      projectId: "project-a",
      reviewItemId: "review-1",
    });
  });

  it("keeps Pipeline Draft and Published Version identity in the URL", () => {
    expect(
      parseWorkspaceRoute(
        "/projects/project-a/build/pipeline",
        "?draft=draft-1",
      ),
    ).toMatchObject({
      kind: "build",
      step: "pipeline",
      draftId: "draft-1",
    });
    expect(
      parseWorkspaceRoute(
        "/projects/project-a/build/pipeline",
        "?workflow=workflow-1&version=3",
      ),
    ).toMatchObject({
      kind: "build",
      step: "pipeline",
      workflowId: "workflow-1",
      workflowVersion: 3,
      canonicalPath: "/projects/project-a/build/pipeline?version=workflow-1%403",
    });
    expect(
      parseWorkspaceRoute(
        "/projects/project-a/build/pipeline",
        "?draft=draft-1&session=session-7&improvement=improve-4",
      ),
    ).toMatchObject({
      draftId: "draft-1",
      agentSessionId: "session-7",
      improvementSessionId: "improve-4",
    });
    expect(projectBuildPath("project-a", "pipeline", {
      workflowId: "workflow-1",
      workflowVersion: 3,
    })).toBe("/projects/project-a/build/pipeline?version=workflow-1%403");
  });

  it("keeps Sample Test identity alongside the exact Draft", () => {
    const path = projectBuildPath("project-a", "test", {
      draftId: "draft-1",
      sampleTestId: "test-9",
    });
    expect(path).toBe("/projects/project-a/build/test?draft=draft-1&test=test-9");
    expect(parseWorkspaceRoute("/projects/project-a/build/test", "?draft=draft-1&test=test-9"))
      .toMatchObject({ draftId: "draft-1", sampleTestId: "test-9" });
  });

  it("does not treat in-page selections as a new heading focus target", () => {
    const firstDraft = parseWorkspaceRoute(
      "/projects/project-a/build/pipeline",
      "?draft=draft-1",
    );
    const secondDraft = parseWorkspaceRoute(
      "/projects/project-a/build/pipeline",
      "?draft=draft-2&session=session-2",
    );
    const firstReview = parseWorkspaceRoute("/projects/project-a/review/review-1");
    const secondReview = parseWorkspaceRoute("/projects/project-a/review/review-2");
    expect(routeFocusKey(firstDraft)).toBe(routeFocusKey(secondDraft));
    expect(routeFocusKey(firstReview)).toBe(routeFocusKey(secondReview));
  });

  it("does not silently rewrite an unknown deep link to Home", () => {
    expect(parseWorkspaceRoute("/missing/deep-link")).toMatchObject({
      kind: "notFound",
      invalidPath: "/missing/deep-link",
    });
  });

  it("redirects legacy model and capability destinations to their real owners", () => {
    expect(parseWorkspaceRoute("/models").canonicalPath).toBe("/settings/models");
    expect(parseWorkspaceRoute("/skills").canonicalPath).toBe("/settings/plugins");
  });

  it("round-trips typed Project child route builders", () => {
    const routes = [
      projectRunsPath("project / one", "failed"),
      projectRunPath("project / one", "run / one", {
        imageId: "image / one",
        nodeId: "detect ball",
        artifactId: "artifact / one",
        view: "debug",
      }),
      projectBatchPath("project / one", "batch / one"),
      projectReviewPath("project / one", "review / one"),
    ];
    for (const path of routes) {
      const url = new URL(path, "http://annotagent.local");
      expect(parseWorkspaceRoute(url.pathname, url.search).canonicalPath).toBe(path);
    }
  });

  it("keeps malformed path segments and build steps out of valid workspaces", () => {
    expect(parseWorkspaceRoute("/projects/%E0%A4%A/runs").kind).toBe("notFound");
    expect(parseWorkspaceRoute("/projects/demo/build/not-a-step").kind).toBe(
      "notFound",
    );
  });
});
