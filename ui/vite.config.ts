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
        index: `${import.meta.dirname}/index.html`,
        toast: `${import.meta.dirname}/toast.html`,
        overlay: `${import.meta.dirname}/overlay.html`,
        pair: `${import.meta.dirname}/pair.html`,
      },
    },
  },
});
