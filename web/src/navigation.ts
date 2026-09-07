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
  | { kind: "journey"; canonicalPath: string; projectId: string; scene: "images" | "goal" | "samples" | "model" | "confirm"; draftId?: string; sampleTestId?: string; imageId?: string; agentSessionId?: string; sampleOperationId?: string; processingOperationId?: string; sampleView?: "authorize"; modelPurpose?: "vision" }
  | { kind: "export"; canonicalPath: string; projectId: string }
  | {
      kind: "build";
      canonicalPath: string;
      projectId: string;
      step: BuildStep;
      draftId?: string;
      workflowId?: string;
      workflowVersion?: number;
      agentSessionId?: string;
      improvementSessionId?: string;
      sampleTestId?: string;
      imageId?: string;
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
  | { kind: "projectTrash"; canonicalPath: string; projectId: string; objectKind?: string }
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
  | { kind: "projectBatch"; canonicalPath: string; projectId: string; batchId: string; imageId?: string; status?: string }
  | { kind: "projectReview"; canonicalPath: string; projectId: string; reviewItemId?: string; view?: "audit" }
  | {
      kind: "review";
      canonicalPath: string;
      reviewItemId?: string;
      projectId?: string;
      view?: "audit";
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

export type BuildUrlContext = {
  modelPurpose?: "vision";
  processingOperationId?: string;
  sampleView?: "authorize";
  sampleOperationId?: string;
  draftId?: string;
  workflowId?: string;
  workflowVersion?: number;
  agentSessionId?: string;
  improvementSessionId?: string;
  sampleTestId?: string;
  imageId?: string;
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

export function projectBatchPath(projectId: string, batchId: string, context: { imageId?: string; status?: string } = {}): string {
  const search = new URLSearchParams();
  if (context.imageId) search.set("image", context.imageId);
  if (context.status && context.status !== "all") search.set("status", context.status);
  return `/projects/${encodeURIComponent(projectId)}/batches/${encodeURIComponent(batchId)}${search.size ? `?${search}` : ""}`;
}

export function projectTrashPath(projectId: string, objectKind?: string): string {
  const base = `/projects/${encodeURIComponent(projectId)}/manage/trash`;
  return objectKind && objectKind !== "all"
    ? `${base}?kind=${encodeURIComponent(objectKind)}`
    : base;
}

export function projectReviewPath(projectId: string, reviewItemId?: string, view?: "audit"): string {
  const base = `/projects/${encodeURIComponent(projectId)}/review`;
  return reviewItemId ? `${base}/${encodeURIComponent(reviewItemId)}${view === "audit" ? "?view=audit" : ""}` : base;
}

export function projectBuildPath(
  projectId: string,
  step: BuildStep,
  context: BuildUrlContext = {},
): string {
  const params = new URLSearchParams();
  if ((step === "pipeline" || step === "test") && context.draftId)
    params.set("draft", context.draftId);
  if (step === "pipeline" && context.workflowId && context.workflowVersion)
    params.set("version", `${context.workflowId}@${context.workflowVersion}`);
  if (step === "pipeline" && context.agentSessionId)
    params.set("session", context.agentSessionId);
  if (step === "pipeline" && context.improvementSessionId)
    params.set("improvement", context.improvementSessionId);
  if (step === "test" && context.sampleTestId)
    params.set("test", context.sampleTestId);
  if (step === "test" && context.imageId) params.set("image", context.imageId);
  const suffix = params.size ? `?${canonicalSearch(params)}` : "";
  return `/projects/${encodeURIComponent(projectId)}/build/${step}${suffix}`;
}

export function projectJourneyPath(projectId: string, scene: "images" | "goal" | "samples" | "model" | "confirm", context: BuildUrlContext = {}): string {
  const params = new URLSearchParams();
  if (scene === "model" && context.modelPurpose === "vision") params.set("purpose", "vision");
  if (context.sampleOperationId) params.set("operation", context.sampleOperationId);
  if (scene === "confirm" && context.processingOperationId) params.set("operation", context.processingOperationId);
  if (scene === "samples" && context.sampleView === "authorize") params.set("view", "authorize");
  if (context.agentSessionId) params.set("session", context.agentSessionId);
  if (context.draftId) params.set("draft", context.draftId);
  if (context.sampleTestId) params.set("test", context.sampleTestId);
  if (context.imageId) params.set("image", context.imageId);
  return `/projects/${encodeURIComponent(projectId)}/task/${scene}${params.size ? `?${canonicalSearch(params)}` : ""}`;
}

export function routeFocusKey(route: WorkspaceRoute): string {
  switch (route.kind) {
    case "projects":
      return route.create ? "project-create" : "projects";
    case "build":
      return `build:${route.projectId}:${route.step}`;
    case "journey":
      return `journey:${route.projectId}:${route.scene}`;
    case "projectRun":
      return `project-run:${route.projectId}:${route.runId}`;
    case "runs":
      return route.runId ? `run:${route.runId}` : "runs";
    case "projectReview":
      return `project-review:${route.projectId}`;
    case "review":
      return "review";
    case "projectRuns":
      return `project-runs:${route.projectId}`;
    case "projectBatch":
      return `project-batch:${route.projectId}:${route.batchId}`;
    case "projectTrash":
      return `project-trash:${route.projectId}`;
    case "project":
    case "export":
      return `${route.kind}:${route.projectId}`;
    case "settings":
      return `settings:${route.section}`;
    case "notFound":
      return "not-found";
    default:
      return route.kind;
  }
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
    return { kind: "projects", create: params.get("new") === "1", canonicalPath: params.get("new") === "1" ? "/projects?new=1" : "/projects" };
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

  const journey = clean.match(/^\/projects\/([^/]+)\/task\/(images|goal|samples|model|confirm)$/);
  if (journey) {
    const projectId = decodePathSegment(journey[1]);
    if (!projectId) return { kind: "notFound", canonicalPath: `${clean}${search}`, invalidPath: `${clean}${search}` };
    const scene = journey[2] as "images" | "goal" | "samples" | "model" | "confirm";
    const context = { draftId: params.get("draft") ?? undefined, sampleTestId: params.get("test") ?? undefined, imageId: params.get("image") ?? undefined, agentSessionId: params.get("session") ?? undefined, sampleOperationId: scene !== "confirm" ? params.get("operation") ?? undefined : undefined, processingOperationId: scene === "confirm" ? params.get("operation") ?? undefined : undefined };
    const sampleView = scene === "samples" && params.get("view") === "authorize" ? "authorize" as const : undefined;
    const modelPurpose = scene === "model" && params.get("purpose") === "vision" ? "vision" as const : undefined;
    return { kind: "journey", projectId, scene, ...context, sampleView, modelPurpose, canonicalPath: projectJourneyPath(projectId, scene, { ...context, sampleView, modelPurpose }) };
  }
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
      imageId: params.get("image") ?? undefined,
      status: params.get("status") ?? undefined,
      canonicalPath: projectBatchPath(projectId, batchId, { imageId: params.get("image") ?? undefined, status: params.get("status") ?? undefined }),
    };
  }
  const projectTrash = clean.match(/^\/projects\/([^/]+)\/manage\/trash$/);
  if (projectTrash) {
    const projectId = decodePathSegment(projectTrash[1]);
    if (!projectId)
      return {
        kind: "notFound",
        invalidPath: `${clean}${search}`,
        canonicalPath: `${clean}${search}`,
      };
    const objectKind = params.get("kind") ?? undefined;
    return {
      kind: "projectTrash",
      projectId,
      objectKind,
      canonicalPath: projectTrashPath(projectId, objectKind),
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
      view: params.get("view") === "audit" ? "audit" : undefined,
      canonicalPath: projectReviewPath(projectId, reviewItemId, params.get("view") === "audit" ? "audit" : undefined),
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
    const combinedVersion = step === "pipeline" ? params.get("version") : null;
    const combinedMatch = combinedVersion?.match(/^(.+)@(\d+)$/);
    const workflowId = step === "pipeline"
      ? combinedMatch?.[1] ?? params.get("workflow") ?? undefined
      : undefined;
    const parsedVersion = step === "pipeline"
      ? Number(combinedMatch?.[2] ?? (params.has("workflow") ? params.get("version") : undefined))
      : undefined;
    const workflowVersion =
      parsedVersion !== undefined &&
      Number.isInteger(parsedVersion) &&
      parsedVersion > 0
        ? parsedVersion
        : undefined;
    const agentSessionId = step === "pipeline" ? params.get("session") ?? undefined : undefined;
    const improvementSessionId = step === "pipeline" ? params.get("improvement") ?? undefined : undefined;
    const sampleTestId = step === "test" ? params.get("test") ?? undefined : undefined;
    const imageId = step === "test" ? params.get("image") ?? undefined : undefined;
    const canonicalPath = projectBuildPath(projectId, step, {
      draftId,
      workflowId,
      workflowVersion,
      agentSessionId,
      improvementSessionId,
      sampleTestId,
      imageId,
    });
    return {
      kind: "build",
      projectId,
      step,
      draftId,
      workflowId,
      workflowVersion,
      agentSessionId,
      improvementSessionId,
      sampleTestId,
      imageId,
      canonicalPath,
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
    if (params.get("view") === "audit") context.set("view", "audit");
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
        view: params.get("view") === "audit" ? "audit" : undefined,
        canonicalPath: projectReviewPath(projectId, reviewItemId, params.get("view") === "audit" ? "audit" : undefined),
      };
    return {
      kind: "review",
      reviewItemId,
      projectId,
      view: params.get("view") === "audit" ? "audit" : undefined,
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
