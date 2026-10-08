// Publishes the design tokens as CSS variables on <html>, following the system color scheme.
import { blur, layout, palette, radius, shadow, type as typeScale, type Scheme } from "./tokens";

type PlatformKey = "macos" | "windows";

const query = window.matchMedia("(prefers-color-scheme: dark)");

export function currentScheme(): Scheme {
  return query.matches ? "dark" : "light";
}

export function onSchemeChange(listener: (scheme: Scheme) => void): () => void {
  const handler = () => listener(currentScheme());
  query.addEventListener("change", handler);
  return () => query.removeEventListener("change", handler);
}

function kebab(name: string): string {
  return name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
}

export function applyTokens(scheme: Scheme, platform: PlatformKey) {
  const vars: Record<string, string> = {};
  for (const [k, v] of Object.entries(palette[scheme])) vars[`--c-${kebab(k)}`] = v;
  for (const [k, v] of Object.entries(radius[platform])) vars[`--r-${kebab(k)}`] = v;
  for (const [k, v] of Object.entries(shadow[scheme])) vars[`--shadow-${kebab(k)}`] = v;
  for (const [k, v] of Object.entries(blur)) vars[`--blur-${kebab(k)}`] = v;
  for (const [k, v] of Object.entries(typeScale)) vars[`--t-${kebab(k)}`] = v;
  vars["--sidebar-width"] = layout.sidebarWidth;
  vars["--titlebar"] = layout.titlebar[platform];
  vars["--scan-button"] = layout.scanButton;

  const root = document.documentElement;
  for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
  root.style.colorScheme = scheme;
  root.dataset.scheme = scheme;
}
