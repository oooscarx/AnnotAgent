import {defineConfig} from '@playwright/test';
export default defineConfig({testDir:'./e2e/ui-preview',outputDir:'ui-preview-results',timeout:30000,workers:1,use:{baseURL:'http://127.0.0.1:5174',viewport:{width:1440,height:960},trace:'retain-on-failure'},webServer:{command:'npm run dev:ui-preview',url:'http://127.0.0.1:5174',reuseExistingServer:true,timeout:30000}});
