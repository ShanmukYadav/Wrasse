import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { applyTokens, systemScheme } from "./design/apply";
import { devLog, inTauri } from "./lib/backend";
import "./index.css";

// In dev, surface webview errors in the terminal running `tauri dev`.
if (import.meta.env.DEV && inTauri) {
  window.addEventListener("error", (e) => void devLog(`[webview error] ${e.message} at ${e.filename}:${e.lineno}`));
  window.addEventListener("unhandledrejection", (e) => void devLog(`[webview rejection] ${String(e.reason)}`));
  void devLog(`[webview] loaded, visibility=${document.visibilityState}`);
}

// Publish tokens before the first paint so nothing flashes unstyled.
applyTokens(systemScheme(), navigator.userAgent.includes("Windows") ? "windows" : "macos");

// Backdrop blur waits until the first paint has landed, then the glass fills stay as they are.
requestAnimationFrame(() => {
  requestAnimationFrame(() => {
    document.documentElement.classList.add("is-glass");
  });
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
