// The desktop webview. Port and output directory are the ones app/src-tauri/tauri.conf.json names.
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 7419, strictPort: true },
  build: { outDir: "dist", emptyOutDir: true, target: "es2022" },
});
