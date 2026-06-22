// @ts-nocheck
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

const r = (p) => fileURLToPath(new URL(p, import.meta.url));

// https://vite.dev/config/
export default defineConfig(async ({ mode }) => {
  const mock = mode === "mock";
  const mockAlias = mock
    ? {
        "@tauri-apps/api/core": r("src/lib/mock/core.ts"),
        "@tauri-apps/api/event": r("src/lib/mock/event.ts"),
        "@tauri-apps/api/webviewWindow": r("src/lib/mock/webviewWindow.ts"),
        "@tauri-apps/plugin-os": r("src/lib/mock/os.ts"),
      }
    : {};
  return {
    plugins: [tailwindcss(), sveltekit()],
    resolve: { alias: mockAlias },
    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent Vite from obscuring rust errors
    clearScreen: false,
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
      port: mock ? 1430 : 1420,
      strictPort: true,
      host: host || false,
      hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
      watch: {
        // 3. tell Vite to ignore watching `src-tauri`
        ignored: ["**/src-tauri/**"],
      },
    },
  };
});
