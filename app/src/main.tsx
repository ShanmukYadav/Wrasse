import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { applyTokens, currentScheme } from "./design/apply";
import "./index.css";

// Publish tokens before the first paint so nothing flashes unstyled.
applyTokens(currentScheme(), navigator.userAgent.includes("Windows") ? "windows" : "macos");

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
