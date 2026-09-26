import babel from "@rolldown/plugin-babel";
import tailwindcss from "@tailwindcss/vite";
import react, { reactCompilerPreset } from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// `pnpm dev` proxies the API to a running `distill ui`. The server only accepts requests
// from its own origin, so the proxy rewrites Origin to match. See docs/web-ui.md.
const target = `http://distill.localhost:${process.env.DISTILL_UI_PORT ?? "4777"}`;

export default defineConfig({
  plugins: [react(), babel({ presets: [reactCompilerPreset()] }), tailwindcss()],
  server: {
    proxy: {
      "/api": {
        target,
        changeOrigin: true,
        headers: { origin: target },
      },
      "/auth": { target, changeOrigin: true },
    },
  },
  // Served from loopback by `distill ui`, so one ~600 kB bundle loads instantly.
  build: { outDir: "dist", emptyOutDir: true, chunkSizeWarningLimit: 800 },
});
