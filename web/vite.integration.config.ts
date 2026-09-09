import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
// Dedicated integration server: never proxy to the user's production 8787 service.
export default defineConfig({ plugins: [react(), {name:"integration-spa-entry",enforce:"post",generateBundle(_options,bundle){const html=bundle["agent-integration.html"];if(html?.type==="asset")this.emitFile({type:"asset",fileName:"index.html",source:html.source});}}], build:{outDir:"dist-integration",rollupOptions:{input:"agent-integration.html"}}, server: { host: "127.0.0.1", port: 5175, strictPort: true, proxy: { "/api": "http://127.0.0.1:8792" } } });
