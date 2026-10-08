import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri expects a fixed port and must see Rust errors in the terminal.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // Only src/ should trigger updates. Toolchain, caches, build output and the Rust side change
    // on their own and must never reload the page.
    watch: {
      ignored: ["**/src-tauri/**", "**/dist/**", "**/target/**", "**/.toolchain/**", "**/.npm-cache/**", "**/.DS_Store", "**/*.log"],
    },
  },
  build: {
    target: "es2022",
  },
});
