import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
// Dedicated integration server: never proxy to the user's production 8787 service.
export default defineConfig({ plugins: [react()], server: { host: "127.0.0.1", port: 5175, strictPort: true, proxy: { "/api": "http://127.0.0.1:8792" } } });
