import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./e2e",
  testMatch: ["agent-ui-history.spec.ts", "agent-ui-http.spec.ts", "agent-ui-project-management.spec.ts", "agent-ui-delivery.spec.ts", "agent-ui-mainline-integration.spec.ts", "p0-autonomous-sample.spec.ts", "p0-diagnostic-scenes.spec.ts", "p0-g3-capabilities.spec.ts", "p0-demo-onboarding.spec.ts"],
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  use: {
    baseURL: process.env.AGENT_UI_TEST_URL || "http://127.0.0.1:8796",
    viewport: { width: 1440, height: 960 },
    // Optional human-paced browser regression against the unchanged 120/minute server guard.
    launchOptions: { slowMo: Math.max(0, Math.min(1000, Number(process.env.ANNOTAGENT_E2E_SLOW_MO_MS) || 0)) },
    // Recording is opt-in. It changes evidence capture, never backend/model selection.
    video: process.env.ANNOTAGENT_RECORD_JOURNEY === "1"
      ? { mode: "on", size: { width: 1440, height: 960 } }
      : "off",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  reporter: "list",
});
