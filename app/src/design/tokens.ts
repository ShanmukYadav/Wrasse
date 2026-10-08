// "Reef at Night" design tokens: every color, gradient, radius, shadow, blur and type size.
// Components read these through CSS variables (see apply.ts); nothing else hardcodes a value.

export type Scheme = "dark" | "light";

export type Hue =
  | "teal"
  | "emerald"
  | "indigo"
  | "blue"
  | "violet"
  | "cyan"
  | "ocean"
  | "amber"
  | "coral"
  | "slate";

// Each section hue: a deep base for gradients, a bright glow, and a pale tint for light mode.
export const hues: Record<Hue, { base: string; glow: string; tint: string }> = {
  teal: { base: "#0f766e", glow: "#2dd4bf", tint: "#c6f6ee" },
  emerald: { base: "#047857", glow: "#34d399", tint: "#cdf5e2" },
  indigo: { base: "#3730a3", glow: "#818cf8", tint: "#dfe1ff" },
  blue: { base: "#1d4ed8", glow: "#60a5fa", tint: "#d6e7ff" },
  violet: { base: "#6d28d9", glow: "#a78bfa", tint: "#e8deff" },
  cyan: { base: "#0e7490", glow: "#22d3ee", tint: "#cdf4fb" },
  ocean: { base: "#1e3a8a", glow: "#38bdf8", tint: "#d3ecfb" },
  amber: { base: "#b45309", glow: "#fbbf24", tint: "#fcecc5" },
  coral: { base: "#be3a2c", glow: "#fb7f6b", tint: "#ffdcd5" },
  slate: { base: "#334155", glow: "#94a3b8", tint: "#e2e8f0" },
};

export const palette = {
  dark: {
    abyss: "#050b1a", // window base, deep navy
    deep: "#0a1a33",
    text: "#e8f6ff",
    textMuted: "rgba(200, 228, 245, 0.64)",
    textFaint: "rgba(200, 228, 245, 0.38)",
    glass: "rgba(255, 255, 255, 0.06)",
    glassStrong: "rgba(255, 255, 255, 0.11)",
    glassBorder: "rgba(255, 255, 255, 0.14)",
    sidebar: "rgba(255, 255, 255, 0.045)",
    divider: "rgba(255, 255, 255, 0.08)",
    track: "rgba(255, 255, 255, 0.10)",
    accent: "#22e5ff", // bioluminescent cyan
    accentBright: "#8af5ff",
    accentDeep: "#0891b2",
    accentGlow: "rgba(34, 229, 255, 0.45)",
    onAccent: "#03141f",
    coral: "#ff7a66", // warnings, "Your call"
    safe: "#6ee7a8", // "Safe"
    keep: "#94a3b8",
    focus: "#7dd3fc",
    dev: "#ef4444",
    onTile: "#ffffff",
  },
  light: {
    abyss: "#ffffff",
    deep: "#d6f5fb", // pale aqua
    text: "#06243a",
    textMuted: "rgba(6, 36, 58, 0.62)",
    textFaint: "rgba(6, 36, 58, 0.38)",
    glass: "rgba(255, 255, 255, 0.55)",
    glassStrong: "rgba(255, 255, 255, 0.78)",
    glassBorder: "rgba(255, 255, 255, 0.9)",
    sidebar: "rgba(255, 255, 255, 0.42)",
    divider: "rgba(6, 36, 58, 0.08)",
    track: "rgba(6, 36, 58, 0.08)",
    accent: "#0891b2",
    accentBright: "#3fdcf5",
    accentDeep: "#0e7490",
    accentGlow: "rgba(8, 145, 178, 0.32)",
    onAccent: "#ffffff",
    coral: "#e2553f",
    safe: "#16a34a",
    keep: "#64748b",
    focus: "#0284c7",
    dev: "#dc2626",
    onTile: "#ffffff",
  },
} as const;

// Hex color + alpha (0–1) → #rrggbbaa.
function alpha(hex: string, a: number): string {
  return hex + Math.round(a * 255).toString(16).padStart(2, "0");
}

// Full-window section backdrop: navy-to-teal depth with the section's hue glowing through.
export function sectionGradient(hue: Hue, scheme: Scheme): string {
  const h = hues[hue];
  const p = palette[scheme];
  if (scheme === "dark") {
    return [
      `radial-gradient(70% 60% at 82% 8%, ${alpha(h.glow, 0.34)} 0%, transparent 70%)`,
      `radial-gradient(40% 45% at 0% 10%, ${alpha(h.glow, 0.22)} 0%, transparent 70%)`,
      `radial-gradient(60% 55% at 10% 95%, ${alpha(h.base, 0.55)} 0%, transparent 70%)`,
      `radial-gradient(45% 40% at 45% 55%, ${alpha(hues.teal.base, 0.18)} 0%, transparent 75%)`,
      `linear-gradient(160deg, ${p.abyss} 0%, ${p.deep} 55%, ${alpha(h.base, 0.6)} 100%)`,
    ].join(", ");
  }
  return [
    `radial-gradient(80% 70% at 88% 0%, ${h.tint} 0%, transparent 72%)`,
    `radial-gradient(65% 60% at 0% 100%, ${h.tint} 0%, transparent 72%)`,
    `linear-gradient(165deg, ${p.deep} 0%, ${p.abyss} 75%)`,
  ].join(", ");
}

// Icon tile behind each section icon.
export function hueTile(hue: Hue): string {
  const h = hues[hue];
  return `linear-gradient(145deg, ${h.glow} 0%, ${h.base} 100%)`;
}

export const radius = {
  macos: { sm: "8px", md: "12px", lg: "20px", tile: "9px", heroTile: "20px", pill: "999px" },
  windows: { sm: "5px", md: "8px", lg: "12px", tile: "6px", heroTile: "12px", pill: "999px" },
} as const;

export const shadow = {
  dark: {
    card: "0 1px 0 rgba(255, 255, 255, 0.06) inset, 0 12px 40px rgba(0, 4, 16, 0.45)",
    tile: "0 4px 14px rgba(0, 0, 0, 0.35)",
    glow: "0 0 60px rgba(34, 229, 255, 0.35), 0 0 120px rgba(34, 229, 255, 0.18)",
  },
  light: {
    card: "0 1px 0 rgba(255, 255, 255, 0.9) inset, 0 12px 32px rgba(14, 80, 110, 0.12)",
    tile: "0 4px 12px rgba(14, 80, 110, 0.18)",
    glow: "0 0 50px rgba(8, 145, 178, 0.28), 0 0 100px rgba(8, 145, 178, 0.12)",
  },
} as const;

// Blur only on the sidebar and cards (PLAN.md §7.4).
export const blur = {
  sidebar: "saturate(170%) blur(30px)",
  card: "saturate(160%) blur(24px)",
} as const;

export const type = {
  hero: "76px", // huge size numbers
  display: "34px",
  title: "22px",
  body: "15px",
  label: "13.5px",
  caption: "12px",
} as const;

export const layout = {
  sidebarWidth: "236px",
  titlebar: { macos: "44px", windows: "12px" },
  scanButton: "216px",
} as const;
