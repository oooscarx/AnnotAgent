import { defineConfig } from "@playwright/test";
const port=process.env.UI_PREVIEW_PORT || "5174";
export default defineConfig({
  testDir: "./e2e/ui-preview",
  outputDir: "ui-preview-results",
  timeout: 30000,
  workers: 1,
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    viewport: { width: 1440, height: 960 },
    trace: "retain-on-failure",
  },
  webServer: {
    command: `npm run dev:ui-preview -- --port ${port}`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: true,
    timeout: 30000,
  },
});
