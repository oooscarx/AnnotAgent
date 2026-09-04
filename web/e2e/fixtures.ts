import {
  expect,
  test as base,
  type APIRequestContext,
} from "@playwright/test";

type RequestOptions = Parameters<APIRequestContext["post"]>[1];

function cleanPath(path: string) {
  return path.split("?", 1)[0];
}

function privilegedAction(method: string, path: string): string | undefined {
  const target = cleanPath(path);
  const privileged = method === "DELETE"
    || target === "/api/settings"
    || target.includes("/credential")
    || target.endsWith("/active-probe")
    || target === "/api/model-bundles/install"
    || target === "/api/model-bundles/import"
    || target === "/api/model-bundles/gc"
    || target === "/api/model-installations"
    || (target.startsWith("/api/model-bundles/") && ["/verify", "/test", "/enable", "/disable", "/license-acceptance"].some((suffix) => target.endsWith(suffix)))
    || (target.startsWith("/api/model-instances/") && target.endsWith("/test"))
    || target === "/api/plugins/packages/install"
    || (target.startsWith("/api/plugins/") && ["/test", "/enable", "/disable", "/weights", "/legacy-model-bundle"].some((suffix) => target.endsWith(suffix)));
  return privileged ? `${method} ${target}` : undefined;
}

function protectedRequestContext(request: APIRequestContext): APIRequestContext {
  let session: Promise<string> | undefined;
  const csrfToken = () => {
    session ??= request.get("/api/session").then(async (response) => {
      if (!response.ok()) throw new Error(`local session failed: ${await response.text()}`);
      const payload = await response.json() as { csrf_token?: string };
      if (!payload.csrf_token) throw new Error("local session omitted its CSRF token");
      return payload.csrf_token;
    });
    return session;
  };
  const mutate = async (method: string, path: string, options?: RequestOptions) => {
    const csrf = await csrfToken();
    const headers: Record<string, string> = {
      ...(options?.headers ?? {}),
      "x-annotagent-csrf": csrf,
    };
    const action = privilegedAction(method, path);
    if (action) {
      const confirmation = await request.post("/api/session/privileged-confirmation", {
        headers: { "x-annotagent-csrf": csrf },
        data: { action, confirmed: true },
      });
      if (!confirmation.ok()) {
        throw new Error(`privileged confirmation failed: ${await confirmation.text()}`);
      }
      const payload = await confirmation.json() as { confirmation_token?: string };
      if (!payload.confirmation_token) throw new Error("privileged confirmation omitted its token");
      headers["x-annotagent-privileged-confirmation"] = payload.confirmation_token;
    }
    return request.fetch(path, { ...options, method, headers });
  };
  return new Proxy(request, {
    get(target, property, receiver) {
      if (property === "post") return (path: string, options?: RequestOptions) => mutate("POST", path, options);
      if (property === "put") return (path: string, options?: RequestOptions) => mutate("PUT", path, options);
      if (property === "patch") return (path: string, options?: RequestOptions) => mutate("PATCH", path, options);
      if (property === "delete") return (path: string, options?: RequestOptions) => mutate("DELETE", path, options);
      const value = Reflect.get(target, property, receiver);
      return typeof value === "function" ? value.bind(target) : value;
    },
  });
}

export const test = base.extend({
  request: async ({ request }, use) => {
    await use(protectedRequestContext(request));
  },
});

export { expect };
