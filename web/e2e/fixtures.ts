import {
  expect,
  test as base,
  type APIRequestContext,
  type APIResponse,
  type Route,
} from "@playwright/test";

type RequestOptions = Parameters<APIRequestContext["post"]>[1];

function cleanPath(path: string) {
  return path.split("?", 1)[0];
}

export async function fetchWithinMutationLimit(route: Route): Promise<APIResponse> {
  const deadline = Date.now() + 45_000;
  const original = route.request();
  let headers = original.headers();
  while (true) {
    const response = await route.fetch({ headers });
    if (response.status() !== 429 || Date.now() >= deadline) return response;
    if ((await response.json().catch(() => ({}))).code !== "mutation_rate_limited") return response;
    await new Promise((resolve) => setTimeout(resolve, 1_000));
    // This is a proven pre-execution rejection. Renew the short-lived nonce
    // rather than replay an expired one after the real rate window clears.
    const url = new URL(original.url());
    const action = privilegedAction(original.method(), url.pathname);
    if (action && headers["x-annotagent-csrf"]) {
      const confirmation = await original.frame().page().request.post(`${url.origin}/api/session/privileged-confirmation`, { headers: { "x-annotagent-csrf": headers["x-annotagent-csrf"] }, data: { action, confirmed: true } });
      if (confirmation.ok()) headers = { ...headers, "x-annotagent-privileged-confirmation": (await confirmation.json()).confirmation_token };
      else if (confirmation.status() !== 429 || (await confirmation.json().catch(() => ({}))).code !== "mutation_rate_limited") return confirmation;
    }
  }
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
    || target.endsWith("/management/actions")
    || (target.startsWith("/api/model-bundles/") && ["/verify", "/test", "/enable", "/disable", "/license-acceptance"].some((suffix) => target.endsWith(suffix)))
    || (target.startsWith("/api/model-instances/") && target.endsWith("/test"))
    || target === "/api/plugins/packages/install"
    || (target.startsWith("/api/plugins/") && ["/test", "/enable", "/disable", "/weights", "/legacy-model-bundle"].some((suffix) => target.endsWith(suffix)));
  return privileged ? `${method} ${target}` : undefined;
}

function protectedRequestContext(request: APIRequestContext): APIRequestContext {
  // The full suite can exceed the real local API's 120 mutations/minute guard.
  // Retry only that pre-execution rejection, never Provider errors or executed model actions.
  const withinLocalRateLimit = async (send: () => Promise<APIResponse>) => {
    const deadline = Date.now() + 45_000;
    while (true) {
      const response = await send();
      if (response.status() !== 429 || Date.now() >= deadline) return response;
      const body = await response.json().catch(() => ({}));
      if (body.code !== "mutation_rate_limited") return response;
      await new Promise((resolve) => setTimeout(resolve, 1_000));
    }
  };
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
      const confirmation = await withinLocalRateLimit(() => request.post("/api/session/privileged-confirmation", {
        headers: { "x-annotagent-csrf": csrf },
        data: { action, confirmed: true },
      }));
      if (!confirmation.ok()) {
        throw new Error(`privileged confirmation failed: ${await confirmation.text()}`);
      }
      const payload = await confirmation.json() as { confirmation_token?: string };
      if (!payload.confirmation_token) throw new Error("privileged confirmation omitted its token");
      headers["x-annotagent-privileged-confirmation"] = payload.confirmation_token;
    }
    return withinLocalRateLimit(() => request.fetch(path, { ...options, method, headers }));
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
  page: async ({ page }, use) => {
    // Test-only pacing for the shared isolated server. Security tests import
    // Playwright directly and still exercise the unmodified rejection paths.
    await page.route("**/api/**", async (route) => {
      if (["GET", "HEAD", "OPTIONS"].includes(route.request().method())) return route.fallback();
      const response = await fetchWithinMutationLimit(route);
      return route.fulfill({ response });
    });
    await use(page);
  },
  request: async ({ request }, use) => {
    await use(protectedRequestContext(request));
  },
});

export { expect };
