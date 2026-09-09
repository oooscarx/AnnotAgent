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

/** Called only with a task resolved from server-owned navigation metadata. */
export function settingsTaskReturn(url:URL,project:string,taskId:string):URL {
  const next=taskLocation(url,project);
  next.search="";
  for(const key of ["image","pane","conversation"]) {const value=url.searchParams.get(key);if(value!==null)next.searchParams.set(key,value);}
  next.searchParams.set("task",taskId);
  return next;
}
