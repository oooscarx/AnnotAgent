export type BuildStep = "data" | "labels" | "pipeline" | "test";
export type SettingsSection =
  | "providers"
  | "models"
  | "plugins"
  | "vision-workers"
  | "storage"
  | "usage";

export type WorkspaceRoute =
  | { kind: "home"; canonicalPath: string }
  | { kind: "projects"; canonicalPath: string; create?: boolean }
  | { kind: "project"; canonicalPath: string; projectId: string }
  | { kind: "export"; canonicalPath: string; projectId: string }
  | {
      kind: "build";
      canonicalPath: string;
      projectId: string;
      step: BuildStep;
      draftId?: string;
      workflowId?: string;
      workflowVersion?: number;
    }
  | {
      kind: "runs";
      canonicalPath: string;
      runId?: string;
      projectId?: string;
      status?: string;
      imageId?: string;
      nodeId?: string;
      artifactId?: string;
      view?: "results" | "debug";
    }
  | { kind: "projectRuns"; canonicalPath: string; projectId: string; status?: string }
  | {
      kind: "projectRun";
      canonicalPath: string;
      projectId: string;
      runId: string;
      imageId?: string;
      nodeId?: string;
      artifactId?: string;
      view?: "results" | "debug";
    }
  | { kind: "projectBatch"; canonicalPath: string; projectId: string; batchId: string }
  | { kind: "projectReview"; canonicalPath: string; projectId: string; reviewItemId?: string }
  | {
      kind: "review";
      canonicalPath: string;
      reviewItemId?: string;
      projectId?: string;
    }
  | {
      kind: "settings";
      canonicalPath: string;
      section: SettingsSection;
    }
  | { kind: "notFound"; canonicalPath: string; invalidPath: string };

type RunUrlContext = {
  imageId?: string;
  nodeId?: string;
  artifactId?: string;
  view?: "results" | "debug";
};

function runContextSearch(context: RunUrlContext): string {
  const params = new URLSearchParams();
  if (context.view === "debug" || context.nodeId || context.artifactId) params.set("view", "debug");
  if (context.imageId) params.set("image", context.imageId);
  if (context.nodeId) params.set("node", context.nodeId);
  if (context.artifactId) params.set("artifact", context.artifactId);
  return params.size ? `?${canonicalSearch(params)}` : "";
}

function canonicalSearch(params: URLSearchParams): string {
  return params.toString().replaceAll("+", "%20");
}

function decodePathSegment(value: string): string | undefined {
  try {
    return decodeURIComponent(value);
  } catch {
    return undefined;
  }
}

export function projectRunsPath(projectId: string, status?: string): string {
  const base = `/projects/${encodeURIComponent(projectId)}/runs`;
  return status && status !== "all" ? `${base}?status=${encodeURIComponent(status)}` : base;
}

export function projectRunPath(projectId: string, runId: string, context: RunUrlContext = {}): string {
  return `/projects/${encodeURIComponent(projectId)}/runs/${encodeURIComponent(runId)}${runContextSearch(context)}`;
}

export function projectBatchPath(projectId: string, batchId: string): string {
  return `/projects/${encodeURIComponent(projectId)}/batches/${encodeURIComponent(batchId)}`;
}

export function projectReviewPath(projectId: string, reviewItemId?: string): string {
  const base = `/projects/${encodeURIComponent(projectId)}/review`;
  return reviewItemId ? `${base}/${encodeURIComponent(reviewItemId)}` : base;
}

const BUILD_STEPS = new Set<BuildStep>([
  "data",
  "labels",
  "pipeline",
  "test",
]);
const SETTINGS_SECTIONS = new Set<SettingsSection>([
  "providers",
  "models",
  "plugins",
  "vision-workers",
  "storage",
  "usage",
]);

export function parseWorkspaceRoute(
  pathname: string,
  search = "",
  hash = "",
): WorkspaceRoute {
  const legacyHash = hash.replace(/^#/, "");
  const legacyPath =
    pathname === "/" && legacyHash
      ? legacyHash === "dashboard"
        ? "/"
        : `/${legacyHash}`
      : pathname;
  const clean = legacyPath.replace(/\/+$/, "") || "/";
  const params = new URLSearchParams(search);

  if (clean === "/" || clean === "/home" || clean === "/dashboard")
    return { kind: "home", canonicalPath: "/" };
  if (clean === "/workflows") {
    const projectId = params.get("project_id") ?? params.get("project");
    return projectId
      ? {
          kind: "build",
          projectId,
          step: "pipeline",
          canonicalPath: `/projects/${encodeURIComponent(projectId)}/build/pipeline`,
        }
      : { kind: "projects", canonicalPath: "/projects" };
  }
  if (clean === "/models")
    return {
      kind: "settings",
      section: "models",
      canonicalPath: "/settings/models",
    };
  if (clean === "/providers")
    return {
      kind: "settings",
      section: "providers",
      canonicalPath: "/settings",
    };
  if (clean === "/skills")
    return {
      kind: "settings",
      section: "plugins",
      canonicalPath: "/settings/plugins",
    };
  const legacyArtifact = clean.match(/^\/(?:artifacts|artifact-inspector)(?:\/([^/]+))?$/);
  if (legacyArtifact) {
    const context = new URLSearchParams();
    const projectId = params.get("project_id") ?? params.get("project") ?? undefined;
    if (projectId)
      return {
        kind: "projectRuns",
        projectId,
        canonicalPath: projectRunsPath(projectId),
      };
    context.set("view", "debug");
    const artifactId = legacyArtifact[1]
      ? decodePathSegment(legacyArtifact[1])
      : undefined;
    if (legacyArtifact[1] && !artifactId)
      return {
        kind: "notFound",
        invalidPath: `${clean}${search}`,
        canonicalPath: `${clean}${search}`,
      };
    if (artifactId) context.set("artifact", artifactId);
    return {
      kind: "runs",
      artifactId,
      view: "debug",
      canonicalPath: `/runs?${canonicalSearch(context)}`,
    };
  }
  if (clean === "/projects")
    return {
      kind: "projects",
      create: params.get("new") === "1",
      canonicalPath: params.get("new") === "1" ? "/projects?new=1" : "/projects",
    };

  const projectRun = clean.match(/^\/projects\/([^/]+)\/runs\/([^/]+)$/);
  if (projectRun) {
    const projectId = decodePathSegment(projectRun[1]);
    const runId = decodePathSegment(projectRun[2]);
    if (!projectId || !runId)
      return {
        kind: "notFound",
        invalidPath: `${clean}${search}`,
        canonicalPath: `${clean}${search}`,
      };
    const context: RunUrlContext = {
      imageId: params.get("image") ?? undefined,
      nodeId: params.get("node") ?? undefined,
      artifactId: params.get("artifact") ?? undefined,
      view: params.get("view") === "debug" || params.has("node") || params.has("artifact") ? "debug" : undefined,
    };
    return {
      kind: "projectRun",
      projectId,
      runId,
      ...context,
      canonicalPath: projectRunPath(projectId, runId, context),
    };
  }
  const projectRuns = clean.match(/^\/projects\/([^/]+)\/runs$/);
  if (projectRuns) {
    const projectId = decodePathSegment(projectRuns[1]);
    if (!projectId)
      return {
        kind: "notFound",
        invalidPath: `${clean}${search}`,
        canonicalPath: `${clean}${search}`,
      };
    const status = params.get("status") ?? undefined;
    return { kind: "projectRuns", projectId, status, canonicalPath: projectRunsPath(projectId, status) };
  }
  const projectBatch = clean.match(/^\/projects\/([^/]+)\/batches\/([^/]+)$/);
  if (projectBatch) {
    const projectId = decodePathSegment(projectBatch[1]);
    const batchId = decodePathSegment(projectBatch[2]);
    if (!projectId || !batchId)
      return {
        kind: "notFound",
        invalidPath: `${clean}${search}`,
        canonicalPath: `${clean}${search}`,
      };
    return {
      kind: "projectBatch",
      projectId,
      batchId,
      canonicalPath: projectBatchPath(projectId, batchId),
    };
  }
  const projectReview = clean.match(/^\/projects\/([^/]+)\/review(?:\/([^/]+))?$/);
  if (projectReview) {
    const projectId = decodePathSegment(projectReview[1]);
    const reviewItemId = projectReview[2]
      ? decodePathSegment(projectReview[2])
      : undefined;
    if (!projectId || (projectReview[2] && !reviewItemId))
      return {
        kind: "notFound",
        invalidPath: `${clean}${search}`,
        canonicalPath: `${clean}${search}`,
      };
    return {
      kind: "projectReview",
      projectId,
      reviewItemId,
      canonicalPath: projectReviewPath(projectId, reviewItemId),
    };
  }

  const build = clean.match(/^\/projects\/([^/]+)\/build\/([^/]+)$/);
  if (build) {
    const projectId = decodePathSegment(build[1]);
    const candidate = build[2] as BuildStep;
    if (!projectId || !BUILD_STEPS.has(candidate))
      return {
        kind: "notFound",
        invalidPath: `${clean}${search}`,
        canonicalPath: `${clean}${search}`,
      };
    const step = candidate;
    const draftId = step === "test" || step === "pipeline" ? params.get("draft") ?? undefined : undefined;
    const workflowId = step === "pipeline" ? params.get("workflow") ?? undefined : undefined;
    const parsedVersion = step === "pipeline" && params.get("version") ? Number(params.get("version")) : undefined;
    const workflowVersion =
      parsedVersion !== undefined &&
      Number.isInteger(parsedVersion) &&
      parsedVersion > 0
        ? parsedVersion
        : undefined;
    const context = new URLSearchParams();
    if (draftId) context.set("draft", draftId);
    if (workflowId && workflowVersion) {
      context.set("workflow", workflowId);
      context.set("version", String(workflowVersion));
    }
    const draftContext = context.size ? `?${canonicalSearch(context)}` : "";
    return {
      kind: "build",
      projectId,
      step,
      draftId,
      workflowId,
      workflowVersion,
      canonicalPath: `/projects/${encodeURIComponent(projectId)}/build/${step}${draftContext}`,
    };
  }
  const projectExport = clean.match(/^\/projects\/([^/]+)\/export$/);
  if (projectExport) {
    const projectId = decodePathSegment(projectExport[1]);
    if (!projectId)
      return {
        kind: "notFound",
        invalidPath: `${clean}${search}`,
        canonicalPath: `${clean}${search}`,
      };
    return {
      kind: "export",
      projectId,
      canonicalPath: `/projects/${encodeURIComponent(projectId)}/export`,
    };
  }
  const project = clean.match(/^\/projects\/([^/]+)$/);
  if (project) {
    const projectId = decodePathSegment(project[1]);
    if (!projectId)
      return {
        kind: "notFound",
        invalidPath: `${clean}${search}`,
        canonicalPath: `${clean}${search}`,
      };
    return {
      kind: "project",
      projectId,
      canonicalPath: `/projects/${encodeURIComponent(projectId)}`,
    };
  }
  const run = clean.match(/^\/runs(?:\/([^/]+))?$/);
  if (run) {
    const context = new URLSearchParams();
    const projectId = params.get("project_id") ?? params.get("project") ?? undefined;
    if (!run[1] && projectId) context.set("project_id", projectId);
    const status = !run[1] ? params.get("status") ?? undefined : undefined;
    if (status && status !== "all") context.set("status", status);
    const view = params.get("view") === "debug" || params.has("node") || params.has("artifact")
      ? "debug"
      : undefined;
    if (view) context.set("view", view);
    for (const key of ["image", "node", "artifact"] as const) {
      const value = params.get(key);
      if (value) context.set(key, value);
    }
    const suffix = context.size ? `?${canonicalSearch(context)}` : "";
    if (projectId) {
      if (!run[1])
        return {
          kind: "projectRuns",
          projectId,
          status,
          canonicalPath: projectRunsPath(projectId, status),
        };
      const runId = decodePathSegment(run[1]);
      if (!runId)
        return {
          kind: "notFound",
          invalidPath: `${clean}${search}`,
          canonicalPath: `${clean}${search}`,
        };
      const runContext: RunUrlContext = {
        imageId: params.get("image") ?? undefined,
        nodeId: params.get("node") ?? undefined,
        artifactId: params.get("artifact") ?? undefined,
        view,
      };
      return {
        kind: "projectRun",
        projectId,
        runId,
        ...runContext,
        canonicalPath: projectRunPath(projectId, runId, runContext),
      };
    }
    const runId = run[1] ? decodePathSegment(run[1]) : undefined;
    if (run[1] && !runId)
      return {
        kind: "notFound",
        invalidPath: `${clean}${search}`,
        canonicalPath: `${clean}${search}`,
      };
    return {
      kind: "runs",
      runId,
      projectId,
      status,
      imageId: params.get("image") ?? undefined,
      nodeId: params.get("node") ?? undefined,
      artifactId: params.get("artifact") ?? undefined,
      view,
      canonicalPath: runId
        ? `/runs/${encodeURIComponent(runId)}${suffix}`
        : `/runs${suffix}`,
    };
  }
  const review = clean.match(/^\/review(?:\/([^/]+))?$/);
  if (review) {
    const projectId = params.get("project_id") ?? params.get("project") ?? undefined;
    const context = new URLSearchParams();
    if (projectId) context.set("project_id", projectId);
    const suffix = context.size ? `?${context.toString()}` : "";
    const reviewItemId = review[1] ? decodePathSegment(review[1]) : undefined;
    if (review[1] && !reviewItemId)
      return {
        kind: "notFound",
        invalidPath: `${clean}${search}`,
        canonicalPath: `${clean}${search}`,
      };
    if (projectId)
      return {
        kind: "projectReview",
        projectId,
        reviewItemId,
        canonicalPath: projectReviewPath(projectId, reviewItemId),
      };
    return {
      kind: "review",
      reviewItemId,
      projectId,
      canonicalPath: reviewItemId
        ? `/review/${encodeURIComponent(reviewItemId)}${suffix}`
        : `/review${suffix}`,
    };
  }
  const settings = clean.match(/^\/settings(?:\/([^/]+))?$/);
  if (settings) {
    const legacy = settings[1] === "general" ? "storage" : settings[1];
    const candidate = (legacy ?? "providers") as SettingsSection;
    if (!SETTINGS_SECTIONS.has(candidate))
      return {
        kind: "notFound",
        invalidPath: `${clean}${search}`,
        canonicalPath: `${clean}${search}`,
      };
    const section = candidate;
    return {
      kind: "settings",
      section,
      canonicalPath:
        section === "providers" ? "/settings" : `/settings/${section}`,
    };
  }
  return { kind: "notFound", invalidPath: `${clean}${search}`, canonicalPath: `${clean}${search}` };
}
