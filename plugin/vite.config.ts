// The Pengu Loader plugin: one ES module, `index.js`, for the client's Chromium 108. Its first line
// names the version, the app's own (the root package.json); the desktop app reads it to tell an
// outdated install from a current one.
import { readFileSync } from "node:fs";

import { defineConfig } from "vitest/config";

const { version } = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
) as { version: string };

export default defineConfig({
  define: { WINER_PLUGIN_VERSION: JSON.stringify(version) },
  build: {
    target: "chrome108",
    outDir: "dist",
    emptyOutDir: true,
    lib: { entry: "src/index.ts", formats: ["es"], fileName: () => "index.js" },
    rollupOptions: { output: { banner: `/*! winer-plugin ${version} */` } },
  },
  test: { environment: "jsdom", include: ["src/**/*.test.ts"] },
});
