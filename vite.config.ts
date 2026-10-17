/// <reference types="vitest/config" />
import { fileURLToPath, URL } from "node:url";
import process from "node:process";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { rmSync } from "node:fs";
import type { Plugin } from "vite";

const host = process.env.TAURI_DEV_HOST;

/** `public/mock-plugins` (SDK de référence et plugin exemple) ne sert qu'en mode mock : retiré du build. */
function dropMockPlugins(): Plugin {
  let outDir = "dist";
  return {
    name: "cadence-drop-mock-plugins",
    apply: "build",
    configResolved(config) {
      outDir = config.build.outDir;
    },
    closeBundle() {
      rmSync(`${outDir}/mock-plugins`, { recursive: true, force: true });
    },
  };
}

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react(), tailwindcss(), dropMockPlugins()],

  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },

  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
}));
