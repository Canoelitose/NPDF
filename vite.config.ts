import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

// The Tauri CLI sets these when it drives the dev server for a mobile target,
// where the app has to reach the host machine over the network.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  // Tauri expects a fixed port and must fail rather than silently pick another.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 5174 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/target/**", "**/vendor/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    // Safari 13 is the oldest engine on the iOS versions we support.
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: process.env.TAURI_ENV_DEBUG ? false : "esbuild",
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
