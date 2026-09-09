/** Clean-cut URL contract. This parser never redirects or interprets legacy return keys. */
export const settingsPages = ["general", "providers", "agent-models", "vision-models", "plugins", "storage", "privacy", "usage"] as const;
export type SettingsPage = typeof settingsPages[number];
export const managementPages = ["data", "labels", "pipelines", "runs", "review", "export", "trash"] as const;
export type ManagementPage = typeof managementPages[number];
export type AgentRoute =
  | { kind: "projects" }
  | { kind: "create-project" }
  | { kind: "work"; projectId: string }
  | { kind: "settings"; page: SettingsPage }
  | { kind: "management"; projectId: string; page: ManagementPage }
  | { kind: "detail"; projectId: string; page: "pipelines" | "runs" | "batches" | "review"; objectId: string }
  | { kind: "not-found" };

const validId = (value: string) => value.length > 0 && !/[\/\\\s\x00-\x1f]/.test(value) && value !== "." && value !== "..";
export function parseAgentRoute(url: URL): AgentRoute {
  if (url.hash) return { kind: "not-found" };
  if (url.pathname === "/" || url.pathname === "/projects") return { kind: "projects" };
  if (url.pathname === "/projects/new") return { kind: "create-project" };
  let parts: string[];
  try { parts = url.pathname.slice(1).split("/").map(decodeURIComponent); }
  catch { return { kind: "not-found" }; }
  if (parts[0] === "settings" && parts.length === 2 && settingsPages.includes(parts[1] as SettingsPage)) return { kind: "settings", page: parts[1] as SettingsPage };
  if (parts[0] !== "projects" || !validId(parts[1] || "")) return { kind: "not-found" };
  const projectId = parts[1];
  if (parts.length === 3 && parts[2] === "work") return { kind: "work", projectId };
  if (parts[2] !== "manage") return { kind: "not-found" };
  if (parts.length === 4 && managementPages.includes(parts[3] as ManagementPage)) return { kind: "management", projectId, page: parts[3] as ManagementPage };
  if (parts.length === 5 && ["pipelines", "runs", "batches", "review"].includes(parts[3]) && validId(parts[4])) return { kind: "detail", projectId, page: parts[3] as "pipelines" | "runs" | "batches" | "review", objectId: parts[4] };
  return { kind: "not-found" };
}

export function agentPath(route: Exclude<AgentRoute, { kind: "not-found" }>): string {
  const segment = (id: string) => { if (!validId(id)) throw new Error("Invalid route identifier"); return encodeURIComponent(id); };
  switch (route.kind) {
    case "projects": return "/projects";
    case "create-project": return "/projects/new";
    case "settings": return `/settings/${route.page}`;
    case "work": return `/projects/${segment(route.projectId)}/work`;
    case "management": return `/projects/${segment(route.projectId)}/manage/${route.page}`;
    case "detail": return `/projects/${segment(route.projectId)}/manage/${route.page}/${segment(route.objectId)}`;
  }
}

/** Stored by navigation state, never accepted as an arbitrary return URL.
 * Ownership/existence must be revalidated against the server before restoration. */
export type NavigationContext = {
  projectId: string;
  taskId?: string;
  draftId?: string;
  draftRevision?: number;
  imageId?: string;
  pane?: "thread" | "image";
};
