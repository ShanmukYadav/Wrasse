// Publishes the design tokens as CSS variables on <html>.
import { duration } from "../motion/tokens";
import { backdrop, blur, chips, d1, fontStack, hintOpacity, icon, layout, lensColors, palette, radius, reef, shadow, tones, type as typeScale, weight, type Scheme } from "./tokens";

type PlatformKey = "macos" | "windows";

const query = window.matchMedia("(prefers-color-scheme: dark)");

export function systemScheme(): Scheme {
  return query.matches ? "dark" : "light";
}

export function onSystemSchemeChange(listener: (scheme: Scheme) => void): () => void {
  const handler = () => listener(systemScheme());
  query.addEventListener("change", handler);
  return () => query.removeEventListener("change", handler);
}

const UNITLESS = new Set(["dimmed", "emphasis", "maxShare", "minCells", "cols", "rows", "maxTiles", "hoverLift"]);

/** Publishes nested tokens as --prefix-path variables; numbers become px unless unitless. */
function flatten(prefix: string, obj: object, vars: Record<string, string>) {
  for (const [k, v] of Object.entries(obj)) {
    const name = `${prefix}-${kebab(k)}`;
    if (v && typeof v === "object" && !Array.isArray(v)) flatten(name, v, vars);
    else if (typeof v === "number") vars[`--${name}`] = UNITLESS.has(k) ? String(v) : `${v}px`;
    else if (typeof v === "string") vars[`--${name}`] = v;
  }
}

function kebab(name: string): string {
  return name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
}

/** All CSS variables for a scheme (also used to preview the other theme inline). */
export function themeVars(scheme: Scheme, platform: PlatformKey = "macos"): Record<string, string> {
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
  for (const [k, v] of Object.entries(chips[scheme])) {
    vars[`--chip-${k}-bg`] = v.bg;
    vars[`--chip-${k}-text`] = v.text;
  }
  flatten("d1", d1, vars);
  // Scheme-specific D1 surfaces.
  vars["--d1-row-surface"] = scheme === "dark" ? d1.row.bgDark : d1.row.bg;
  vars["--d1-ghost-surface"] = scheme === "dark" ? d1.ghost.bgDark : d1.ghost.bg;
  vars["--d1-ghost-line"] = scheme === "dark" ? d1.ghost.borderDark : d1.ghost.border;
  // The day lens and detail sheet are white. At night that white bar hides All / Junk / Apps /
  // AI Models / Dev (inactive text is a light gray) and the detail text is light on white.
  vars["--d1-lens-bg"] = scheme === "dark" ? d1.lens.bgDark : d1.lens.bg;
  vars["--d1-lens-shadow"] = shadow[scheme].glass;
  vars["--d1-detail-bg"] = scheme === "dark" ? d1.detail.bgDark : d1.detail.bg;
  vars["--d1-command-shadow"] = scheme === "dark" ? shadow.dark.pill : d1.command.shadow;
  if (scheme === "dark") {
    const inset = `0 1px 0 ${palette.dark.glassHighlight} inset, ${shadow.dark.glass}`;
    vars["--d1-tile-shadow"] = inset;
    vars["--d1-tile-hover-shadow"] = inset;
  }
  for (const [k, v] of Object.entries(duration)) vars[`--d-${k}`] = `${v}s`;

  return vars;
}

export function applyTokens(scheme: Scheme, platform: PlatformKey) {
  const vars = themeVars(scheme, platform);
  const root = document.documentElement;
  for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
  root.style.colorScheme = scheme;
  root.dataset.scheme = scheme;
  root.dataset.platform = platform;
}
