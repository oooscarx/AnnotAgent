import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
export default defineConfig({
  root: 'ui-preview', plugins: [react(), {name:'preview-no-business-api', configureServer(server) {
    server.middlewares.use((req,res,next) => { if (req.url?.startsWith('/api')) {res.statusCode=403;res.end('UI preview cannot access business APIs');} else next(); });
  }}], server: { host:'127.0.0.1', port:5174, strictPort:true, fs:{allow:['..']} },
  build:{outDir:'../dist-ui-preview',emptyOutDir:true},
});
