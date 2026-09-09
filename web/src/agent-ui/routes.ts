/** The new workspace and existing management views share entities, not UI state. */
export function isAgentEntry(url: URL): boolean {
  return url.pathname === "/" ||
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
  if (isAgentEntry(url)) next.pathname = `/projects/${encodeURIComponent(project)}/work`;
  return next;
}

export function managementReturn(target:URL, source:URL):URL {
  const next=new URL(target);
  const owner=routeProject(next);
  if(!owner || source.pathname.split("/")[2]!==encodeURIComponent(owner))return next;
  for(const key of ["task","image","pane"]) {
    const value=source.searchParams.get(`return_${key}`);
    const valid=key==="pane" ? value==="image" : !!value && /^[a-f\d]{8}(?:-[a-f\d]{4}){3}-[a-f\d]{12}$/i.test(value);
    if(valid && !next.searchParams.has(key))next.searchParams.set(key,value!);
  }
  return next;
}

export function retainManagementContext(target:URL, source:URL):URL {
  const next=new URL(target);
  const owner=/^\/projects\/([^/]+)(?:\/|$)/.exec(source.pathname)?.[1];
  if(!owner || /^\/projects\/([^/]+)(?:\/|$)/.exec(next.pathname)?.[1]!==owner)return next;
  for(const key of ["task","image","pane"]) {
    const value=source.searchParams.get(`return_${key}`);
    const valid=key==="pane" ? value==="image" : !!value && /^[a-f\d]{8}(?:-[a-f\d]{4}){3}-[a-f\d]{12}$/i.test(value);
    if(valid)next.searchParams.set(`return_${key}`,value!);
  }
  return next;
}
