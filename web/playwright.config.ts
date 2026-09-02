import { defineConfig, devices } from "@playwright/test";
import { copyFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";

const e2eWorkspace = `/tmp/annotagent-guided-e2e-${process.pid}`;
const e2eImport = `${e2eWorkspace}/import`;
mkdirSync(e2eImport, { recursive: true });
copyFileSync(
  resolve(process.cwd(), "../examples/robocup/images/synthetic-robocup.png"),
  `${e2eImport}/synthetic-robocup.png`,
);
export default defineConfig({
  metadata: { e2eImport },
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
  webServer: [
    {
      command: "cd .. && cargo run -p annotagent-e2e-fixture",
      url: "http://127.0.0.1:8796/health",
      timeout: 120_000,
      reuseExistingServer: true,
    },
    {
      command:
        `cd .. && npm --prefix web run build && cargo run -p annotagent -- serve --workspace ${e2eWorkspace} --port 8791`,
      url: "http://127.0.0.1:8791/api/health",
      timeout: 120_000,
      reuseExistingServer: true,
    },
  ],
});
