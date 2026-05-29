import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { visualizer } from "rollup-plugin-visualizer";
import path from "path";

const analyze = process.env.ANALYZE === "1";

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    analyze &&
      visualizer({
        filename: "dist/bundle-stats.html",
        template: "treemap",
        gzipSize: false,
        brotliSize: false,
        open: true,
      }),
  ].filter(Boolean),
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: ["es2022", "chrome100", "safari15"],
    minify: !process.env.TAURI_DEBUG ? "oxc" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    // Tauri ships locally, no network download; vite's default 500 kB cap is
    // calibrated for web. 800 still flags accidental bloat (a large lib pulled
    // into the main bundle by mistake) without nagging on normal growth.
    chunkSizeWarningLimit: 800,
  },
});
