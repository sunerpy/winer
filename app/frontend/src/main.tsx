import { isTauri } from "@tauri-apps/api/core";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { applyAppearanceHint } from "./lib/appearance";
import { type Backend, tauriBackend } from "./lib/backend";
import { installContextMenuPolicy } from "./lib/contextMenu";
import "./styles/app.css";

applyAppearanceHint();

async function backend(): Promise<Backend> {
  // A browser tab under `pnpm dev` gets an in-memory demo; `import.meta.env.DEV` is a build-time
  // constant, so a release bundle carries neither this branch nor the demo module.
  if (import.meta.env.DEV && !isTauri()) {
    const { demoBackend } = await import("./lib/demo");
    return demoBackend();
  }
  return tauriBackend;
}

if (isTauri()) installContextMenuPolicy(document);

const container = document.getElementById("root");
if (!container) throw new Error("#root is missing");
createRoot(container).render(
  <StrictMode>
    <App backend={await backend()} />
  </StrictMode>,
);
