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
    };

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
      section: "vision-workers",
      canonicalPath: "/settings/vision-workers",
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
      section: "vision-workers",
      canonicalPath: "/settings/vision-workers",
    };
  const legacyArtifact = clean.match(/^\/(?:artifacts|artifact-inspector)(?:\/([^/]+))?$/);
  if (legacyArtifact) {
    const context = new URLSearchParams();
    const projectId = params.get("project_id") ?? params.get("project") ?? undefined;
    if (projectId) context.set("project_id", projectId);
    context.set("view", "debug");
    if (legacyArtifact[1]) context.set("artifact", decodeURIComponent(legacyArtifact[1]));
    return {
      kind: "runs",
      projectId,
      artifactId: legacyArtifact[1]
        ? decodeURIComponent(legacyArtifact[1])
        : undefined,
      view: "debug",
      canonicalPath: `/runs?${context.toString()}`,
    };
  }
  if (clean === "/projects")
    return {
      kind: "projects",
      create: params.get("new") === "1",
      canonicalPath: params.get("new") === "1" ? "/projects?new=1" : "/projects",
    };

  const build = clean.match(/^\/projects\/([^/]+)\/build\/([^/]+)$/);
  if (build) {
    const projectId = decodeURIComponent(build[1]);
    const candidate = build[2] as BuildStep;
    const step = BUILD_STEPS.has(candidate) ? candidate : "data";
    return {
      kind: "build",
      projectId,
      step,
      canonicalPath: `/projects/${encodeURIComponent(projectId)}/build/${step}`,
    };
  }
  const projectExport = clean.match(/^\/projects\/([^/]+)\/export$/);
  if (projectExport) {
    const projectId = decodeURIComponent(projectExport[1]);
    return {
      kind: "export",
      projectId,
      canonicalPath: `/projects/${encodeURIComponent(projectId)}/export`,
    };
  }
  const project = clean.match(/^\/projects\/([^/]+)$/);
  if (project) {
    const projectId = decodeURIComponent(project[1]);
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
    const suffix = context.size ? `?${context.toString()}` : "";
    return {
      kind: "runs",
      runId: run[1] ? decodeURIComponent(run[1]) : undefined,
      projectId,
      status,
      imageId: params.get("image") ?? undefined,
      nodeId: params.get("node") ?? undefined,
      artifactId: params.get("artifact") ?? undefined,
      view,
      canonicalPath: run[1] ? `/runs/${run[1]}${suffix}` : `/runs${suffix}`,
    };
  }
  const review = clean.match(/^\/review(?:\/([^/]+))?$/);
  if (review) {
    const projectId = params.get("project_id") ?? params.get("project") ?? undefined;
    const context = new URLSearchParams();
    if (projectId) context.set("project_id", projectId);
    const suffix = context.size ? `?${context.toString()}` : "";
    return {
      kind: "review",
      reviewItemId: review[1] ? decodeURIComponent(review[1]) : undefined,
      projectId,
      canonicalPath: review[1] ? `/review/${review[1]}${suffix}` : `/review${suffix}`,
    };
  }
  const settings = clean.match(/^\/settings(?:\/([^/]+))?$/);
  if (settings) {
    const legacy = settings[1] === "general" ? "storage" : settings[1];
    const candidate = (legacy ?? "providers") as SettingsSection;
    const section = SETTINGS_SECTIONS.has(candidate) ? candidate : "providers";
    return {
      kind: "settings",
      section,
      canonicalPath:
        section === "providers" ? "/settings" : `/settings/${section}`,
    };
  }
  return { kind: "home", canonicalPath: "/" };
}
