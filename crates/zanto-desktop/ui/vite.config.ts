import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Plain Vite SPA (no SvelteKit) — builds to static assets embedded in the Tauri binary.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2021", outDir: "dist", emptyOutDir: true },
});
