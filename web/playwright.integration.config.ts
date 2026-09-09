import { defineConfig } from "@playwright/test";
export default defineConfig({testDir:"./e2e",testMatch:"agent-ui-http.spec.ts",fullyParallel:false,workers:1,timeout:60_000,use:{baseURL:"http://127.0.0.1:8794",viewport:{width:1440,height:960},trace:"retain-on-failure",screenshot:"only-on-failure"},reporter:"list"});
