import { defineConfig } from "@playwright/test";
export default defineConfig({testDir:"./e2e",testMatch:["agent-ui-http.spec.ts","agent-ui-project-management.spec.ts"],fullyParallel:false,workers:1,timeout:60_000,use:{baseURL:process.env.AGENT_UI_TEST_URL || "http://127.0.0.1:8796",viewport:{width:1440,height:960},trace:"retain-on-failure",screenshot:"only-on-failure"},reporter:"list"});
