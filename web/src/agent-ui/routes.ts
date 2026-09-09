export function routeProject(url: URL): string | null {
  const match = /^\/projects\/([^/]+)\/work\/?$/.exec(url.pathname);
  if (!match) return null;
  try { return decodeURIComponent(match[1]); } catch { return "invalid-project-id"; }
}

export function taskLocation(url: URL, project: string): URL {
  const next = new URL(url);
  if(routeProject(url)!==project)next.search="";
  next.hash="";
  next.pathname = `/projects/${encodeURIComponent(project)}/work`;
  return next;
}
