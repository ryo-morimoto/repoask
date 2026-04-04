import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";

export default defineConfig({
  plugins: [wasm()],
  optimizeDeps: {
    exclude: ["repoask-wasm"],
  },
  server: {
    fs: {
      // Allow serving wasm files from the workspace root
      allow: ["../.."],
    },
  },
});
