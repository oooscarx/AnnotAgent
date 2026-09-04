import { expect, test } from "@playwright/test";

test("localhost API rejects cross-origin access and protects mutations", async ({ request }) => {
  const health = await request.get("/api/health");
  expect(health.ok()).toBeTruthy();
  const healthBody = await health.json();
  expect(healthBody).toMatchObject({ status: "ok", service: "AnnotAgent" });
  expect(healthBody).not.toHaveProperty("workspace");
  expect(healthBody).not.toHaveProperty("database");

  const maliciousRead = await request.get("/api/projects", {
    headers: { Origin: "https://untrusted.example" },
  });
  expect(maliciousRead.status()).toBe(403);
  expect(maliciousRead.headers()).not.toHaveProperty("access-control-allow-origin");

  const maliciousPreflight = await request.fetch("/api/settings", {
    method: "OPTIONS",
    headers: {
      Origin: "https://untrusted.example",
      "Access-Control-Request-Method": "PUT",
    },
  });
  expect(maliciousPreflight.status()).toBe(403);

  const missingSession = await request.post("/api/session/privileged-confirmation", {
    data: { action: "PUT /api/settings", confirmed: true },
  });
  expect(missingSession.status()).toBe(401);
  expect((await missingSession.json()).code).toBe("local_session_required");

  const session = await request.get("/api/session");
  expect(session.ok()).toBeTruthy();
  expect(session.headers()["set-cookie"]).toContain("SameSite=Strict");
  const { csrf_token: csrfToken } = await session.json();

  const wrongCsrf = await request.post("/api/session/privileged-confirmation", {
    headers: { "x-annotagent-csrf": "wrong" },
    data: { action: "PUT /api/settings", confirmed: true },
  });
  expect(wrongCsrf.status()).toBe(403);
  expect((await wrongCsrf.json()).code).toBe("csrf_token_invalid");

  const confirmed = await request.post("/api/session/privileged-confirmation", {
    headers: { "x-annotagent-csrf": csrfToken },
    data: { action: "PUT /api/settings", confirmed: true },
  });
  expect(confirmed.ok()).toBeTruthy();
  expect(await confirmed.json()).toMatchObject({
    action: "PUT /api/settings",
    single_use: true,
  });

});
