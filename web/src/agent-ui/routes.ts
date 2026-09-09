/** The new workspace and existing management views share entities, not UI state. */
export function isAgentEntry(url: URL): boolean {
  return url.pathname === "/" ||
    /^\/projects\/[^/]+\/manage\/(pipelines|runs)$/.test(url.pathname) ||
    /^\/projects\/[^/]+\/manage\/pipelines\/[^/]+$/.test(url.pathname) ||
    /^\/projects\/[^/]+\/manage\/batches\/[^/]+$/.test(url.pathname) ||
    (url.pathname === "/projects" && url.searchParams.get("new") !== "1") ||
    /^\/projects\/[^/]+\/work\/?$/.test(url.pathname) ||
    url.pathname === "/settings" || url.pathname.startsWith("/settings/") ||
    /^\/projects\/[^/]+\/manage\/runs\/[^/]+$/.test(url.pathname) || url.pathname === "/projects/new" || /^\/projects\/[^/]+\/manage\/(data|labels|trash|export)$/.test(url.pathname) || /^\/projects\/[^/]+\/manage\/review(?:\/[^/]+)?$/.test(url.pathname);
}

export function routeProject(url: URL): string | null {
  const match = /^\/projects\/([^/]+)\/work\/?$/.exec(url.pathname);
  if (!match) return null;
  try { return decodeURIComponent(match[1]); } catch { return "invalid-project-id"; }
}

export function taskLocation(url: URL, project: string): URL {
  const next = new URL(url);
  if (isAgentEntry(url)) {
    if(routeProject(url)!==project)next.search="";
    next.pathname = `/projects/${encodeURIComponent(project)}/work`;
  }
  return next;
}
