import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: [["list"]],
  use: {
    baseURL: "http://127.0.0.1:8791",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } },
    },
  ],
  webServer: {
    command:
      "cd .. && npm --prefix web run build && ANNOTAGENT_DISABLE_KEYCHAIN=1 cargo run -p annotagent -- serve --workspace workspace/e2e-guided --port 8791",
    url: "http://127.0.0.1:8791/api/health",
    timeout: 120_000,
    reuseExistingServer: true,
  },
});
