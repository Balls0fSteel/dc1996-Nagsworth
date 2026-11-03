import { resolve } from "node:path";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Multi-page: the dashboard plus the nag windows the app opens on demand.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: {
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        index: resolve(__dirname, "index.html"),
        toast: resolve(__dirname, "toast.html"),
        overlay: resolve(__dirname, "overlay.html"),
        pair: resolve(__dirname, "pair.html"),
      },
    },
  },
});
