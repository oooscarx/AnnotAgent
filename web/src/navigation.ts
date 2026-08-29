export type BuildStep = "data" | "labels" | "pipeline" | "test";
export type SettingsSection = "general" | "models" | "capabilities";

export type WorkspaceRoute =
  | { kind: "home"; canonicalPath: string }
  | { kind: "projects"; canonicalPath: string; create?: boolean }
  | { kind: "project"; canonicalPath: string; projectId: string }
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
  "general",
  "models",
  "capabilities",
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
  if (clean === "/skills")
    return {
      kind: "settings",
      section: "capabilities",
      canonicalPath: "/settings/capabilities",
    };
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
    const candidate = (settings[1] ?? "general") as SettingsSection;
    const section = SETTINGS_SECTIONS.has(candidate) ? candidate : "general";
    return {
      kind: "settings",
      section,
      canonicalPath:
        section === "general" ? "/settings" : `/settings/${section}`,
    };
  }
  return { kind: "home", canonicalPath: "/" };
}
