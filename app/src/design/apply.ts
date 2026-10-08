// Publishes the design tokens as CSS variables on <html>.
import { duration } from "../motion/tokens";
import { backdrop, blur, fontStack, hintOpacity, icon, layout, lensColors, palette, radius, reef, shadow, tones, type as typeScale, weight, type Scheme } from "./tokens";

export type PlatformKey = "macos" | "windows";

const query = window.matchMedia("(prefers-color-scheme: dark)");

export function systemScheme(): Scheme {
  return query.matches ? "dark" : "light";
}

export function onSystemSchemeChange(listener: (scheme: Scheme) => void): () => void {
  const handler = () => listener(systemScheme());
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
  for (const [k, v] of Object.entries(layout)) vars[`--l-${kebab(k)}`] = v;
  vars["--backdrop"] = backdrop[scheme];
  vars["--caustics"] = reef.caustics[scheme];
  for (const [k, v] of Object.entries(lensColors[scheme])) vars[`--lens-${k}`] = v;
  for (const [k, v] of Object.entries(tones[scheme])) vars[`--tone-${kebab(k)}`] = v;
  for (const [k, v] of Object.entries(weight)) vars[`--w-${k}`] = String(v);
  vars["--hint-opacity"] = String(hintOpacity);
  vars["--icon-stroke"] = String(icon.stroke);
  vars["--font"] = fontStack;
  for (const [k, v] of Object.entries(duration)) vars[`--d-${k}`] = `${v}s`;

  const root = document.documentElement;
  for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
  root.style.colorScheme = scheme;
  root.dataset.scheme = scheme;
  root.dataset.platform = platform;
}
