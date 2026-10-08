// "Reef Map" design tokens: every color, gradient, radius, shadow, blur and type size.
// DOM components read these through CSS variables (see apply.ts); the Canvas map reads them
// directly. Nothing else hardcodes a visual value.

export type Scheme = "dark" | "light";

export type Category = "apps" | "users" | "library" | "developer" | "aiModels" | "system" | "other";
export type LensId = "all" | "junk" | "apps" | "aiModels" | "dev" | "largeOld" | "grew";
export type Safety = "safe" | "yourCall" | "keep";

// UI chrome colors. "Night reef" is the primary design; "Day reef" mirrors it.
export const palette = {
  dark: {
    canvas: "#060b18",
    text: "#e8f6ff",
    textMuted: "rgba(200, 228, 245, 0.66)",
    textFaint: "rgba(200, 228, 245, 0.40)",
    glass: "rgba(14, 24, 46, 0.42)",
    glassStrong: "rgba(20, 34, 62, 0.68)",
    glassBorder: "rgba(255, 255, 255, 0.14)",
    glassHighlight: "rgba(255, 255, 255, 0.32)",
    glassRefraction: "linear-gradient(135deg, rgba(255, 255, 255, 0.10) 0%, rgba(255, 255, 255, 0) 38%, rgba(120, 220, 255, 0.06) 72%, rgba(255, 255, 255, 0.04) 100%)",
    scrim: "rgba(2, 6, 16, 0.5)",
    divider: "rgba(255, 255, 255, 0.08)",
    track: "rgba(255, 255, 255, 0.12)",
    pill: "rgba(255, 255, 255, 0.14)",
    accent: "#22e5ff", // bioluminescent cyan
    accentDeep: "#0891b2",
    onAccent: "#03141f",
    safe: "#6ee7a8",
    yourCall: "#ff8a73",
    keep: "#9aa8bd",
    focus: "#7dd3fc",
    dev: "#ef4444",
    onDev: "#ffffff",
  },
  light: {
    canvas: "#eefbfd",
    text: "#06243a",
    textMuted: "rgba(6, 36, 58, 0.64)",
    textFaint: "rgba(6, 36, 58, 0.40)",
    glass: "rgba(255, 255, 255, 0.50)",
    glassStrong: "rgba(255, 255, 255, 0.80)",
    glassBorder: "rgba(255, 255, 255, 0.9)",
    glassHighlight: "rgba(255, 255, 255, 1)",
    glassRefraction: "linear-gradient(135deg, rgba(255, 255, 255, 0.55) 0%, rgba(255, 255, 255, 0) 40%, rgba(120, 220, 255, 0.10) 75%, rgba(255, 255, 255, 0.25) 100%)",
    scrim: "rgba(220, 240, 248, 0.45)",
    divider: "rgba(6, 36, 58, 0.08)",
    track: "rgba(6, 36, 58, 0.10)",
    pill: "rgba(6, 36, 58, 0.08)",
    accent: "#0891b2",
    accentDeep: "#0e7490",
    onAccent: "#ffffff",
    safe: "#15803d",
    yourCall: "#c2410c",
    keep: "#64748b",
    focus: "#0284c7",
    dev: "#dc2626",
    onDev: "#ffffff",
  },
} as const;

// Full-window backdrop behind the map (visible in gaps and around the map edges).
export const backdrop: Record<Scheme, string> = {
  dark: "radial-gradient(80% 70% at 85% 0%, #0f3a4a 0%, transparent 65%), radial-gradient(70% 60% at 0% 100%, #102a52 0%, transparent 70%), linear-gradient(160deg, #060b18 0%, #081428 60%, #0a2433 100%)",
  light: "radial-gradient(80% 70% at 88% 0%, #c6f3f7 0%, transparent 70%), radial-gradient(65% 60% at 0% 100%, #d4ecfb 0%, transparent 72%), linear-gradient(165deg, #d6f5fb 0%, #ffffff 75%)",
};

// The Canvas map.
export const map = {
  dark: {
    background: "rgba(6, 11, 24, 0.86)", // slightly translucent so native window glass shows in the gaps
    blockStroke: "rgba(255, 255, 255, 0.07)", // soft inner glow
    blockHover: "rgba(255, 255, 255, 0.16)",
    label: "rgba(235, 248, 255, 0.92)",
    labelMuted: "rgba(235, 248, 255, 0.58)",
    header: "rgba(235, 248, 255, 0.72)",
    groupFrame: "rgba(255, 255, 255, 0.035)",
    selection: "#22e5ff",
    ghostStroke: "rgba(200, 228, 245, 0.55)",
    ghostFill: "rgba(200, 228, 245, 0.04)",
    ghostLabel: "rgba(200, 228, 245, 0.85)",
    sonar: "rgba(34, 229, 255, 0.55)",
    candidate: "#22e5ff",
    tooltipBg: "rgba(10, 20, 40, 0.92)",
  },
  light: {
    background: "rgba(238, 251, 253, 0.9)",
    blockStroke: "rgba(255, 255, 255, 0.55)",
    blockHover: "rgba(255, 255, 255, 0.35)",
    label: "rgba(6, 36, 58, 0.9)",
    labelMuted: "rgba(6, 36, 58, 0.55)",
    header: "rgba(6, 36, 58, 0.7)",
    groupFrame: "rgba(6, 36, 58, 0.035)",
    selection: "#0891b2",
    ghostStroke: "rgba(6, 36, 58, 0.45)",
    ghostFill: "rgba(6, 36, 58, 0.03)",
    ghostLabel: "rgba(6, 36, 58, 0.8)",
    sonar: "rgba(8, 145, 178, 0.5)",
    candidate: "#0891b2",
    tooltipBg: "rgba(255, 255, 255, 0.95)",
  },
  radius: 6,
  gap: 2,
  headerHeight: 18,
  // One level at a time: tiles inside an area kept clear of the floating chrome.
  inset: { top: 64, right: 16, bottom: 76, left: 16 },
  tileGap: 6,
  tileRadius: 14,
  tileFont: '500 15px -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI Variable", "Segoe UI", sans-serif',
  tileSizeFont: '600 28px -apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI Variable", "Segoe UI", sans-serif',
  tileMetaFont: '400 12px -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI Variable", "Segoe UI", sans-serif',
  tileIcon: 20,
  textureAlpha: 0.16,
  labelFont: '500 13px -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI Variable", "Segoe UI", sans-serif',
  sizeFont: '600 12px -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI Variable", "Segoe UI", sans-serif',
  headerFont: '600 12px -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI Variable", "Segoe UI", sans-serif',
  ghostDash: [6, 5],
  dimmedAlpha: 0.25,
} as const;

// Subtle hue per top-level category: three shades each, picked per block for texture.
export const categoryShades: Record<Scheme, Record<Category, readonly [string, string, string]>> = {
  dark: {
    apps: ["#1e3a6e", "#24477f", "#2b5390"],
    users: ["#134e5e", "#175c6d", "#1b6a7c"],
    library: ["#2a3557", "#313d63", "#38466f"],
    developer: ["#0f4a5c", "#11566a", "#146378"],
    aiModels: ["#3b2a6b", "#45317a", "#503889"],
    system: ["#262c3d", "#2c3346", "#323a4f"],
    other: ["#22303f", "#283848", "#2e4051"],
  },
  light: {
    apps: ["#cfe0fb", "#c4d8f8", "#b9d0f5"],
    users: ["#c9eef0", "#bde8ea", "#b1e2e5"],
    library: ["#dde3f1", "#d3dbec", "#c9d2e7"],
    developer: ["#c6eef6", "#b9e8f2", "#acE2ee"],
    aiModels: ["#e3dafb", "#d9cef8", "#cfc2f5"],
    system: ["#e0e3ea", "#d7dbe4", "#ced3de"],
    other: ["#dce5ec", "#d2dde6", "#c8d5e0"],
  },
};

// Lens colors (PLAN.md lens bar): Junk amber, Apps blue, AI Models violet, Dev cyan,
// Large & Old coral, Grew green.
export const lensColors: Record<Scheme, Record<Exclude<LensId, "all">, string>> = {
  dark: {
    junk: "#f5b83d",
    apps: "#4f8dfd",
    aiModels: "#a78bfa",
    dev: "#22d3ee",
    largeOld: "#ff7a66",
    grew: "#4ade80",
  },
  light: {
    junk: "#f2b13a",
    apps: "#6b9ffd",
    aiModels: "#b9a2fb",
    dev: "#4fd8ef",
    largeOld: "#ff9482",
    grew: "#6be397",
  },
};

export const radius = {
  macos: { sm: "8px", md: "12px", lg: "18px", bar: "22px", pill: "999px" },
  windows: { sm: "5px", md: "8px", lg: "12px", bar: "12px", pill: "999px" },
} as const;

export const shadow = {
  dark: {
    glass: "0 18px 50px rgba(0, 4, 16, 0.55), 0 2px 8px rgba(0, 4, 16, 0.35)",
    pill: "0 4px 14px rgba(0, 0, 0, 0.35)",
    flyer: "0 10px 30px rgba(0, 0, 0, 0.5)",
  },
  light: {
    glass: "0 18px 44px rgba(14, 80, 110, 0.16), 0 2px 8px rgba(14, 80, 110, 0.10)",
    pill: "0 4px 12px rgba(14, 80, 110, 0.16)",
    flyer: "0 10px 26px rgba(14, 80, 110, 0.22)",
  },
} as const;

// Blur only on glass elements (PLAN.md §7.4).
export const blur = {
  glass: "saturate(180%) blur(28px)",
  scrim: "blur(2px)",
} as const;

// Fixed type scale (px). Names medium, sizes semibold, hints regular at 70% opacity.
export const typeScale = [12, 13, 15, 17, 22, 28, 40, 64] as const;
export const type = {
  caption: "12px",
  label: "13px",
  body: "15px",
  heading: "17px",
  title: "22px",
  display: "28px",
  hero: "40px",
  giant: "64px",
} as const;
export const weight = { regular: 400, name: 500, size: 600, strong: 650 } as const;
export const hintOpacity = 0.7;
export const fontStack = '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI Variable", "Segoe UI", sans-serif';

// Icons: one duotone family on a 24×24 grid with a 2px safe margin.
export const icon = {
  sizes: [16, 20, 24, 32] as const,
  stroke: 1.75,
  fillOpacity: 0.2,
  grid: 24,
} as const;
export type IconSize = (typeof icon.sizes)[number];

export type Tone =
  | Category
  | "accent"
  | "safe"
  | "yourCall"
  | "keep"
  | "warning"
  | "neutral"
  | "ghost";

// Icon tones: bright enough to read on tiles and glass in each scheme.
export const tones: Record<Scheme, Record<Tone, string>> = {
  dark: {
    apps: "#7fb0ff",
    users: "#5fd8cc",
    library: "#a3afff",
    developer: "#4fdcf0",
    aiModels: "#b9a4ff",
    system: "#aab6c8",
    other: "#9fb3c6",
    accent: "#22e5ff",
    safe: "#6ee7a8",
    yourCall: "#ff8a73",
    keep: "#9aa8bd",
    warning: "#f5b83d",
    neutral: "#e8f6ff",
    ghost: "#c8e4f5",
  },
  light: {
    apps: "#2f6fd6",
    users: "#0f8f86",
    library: "#4f5bd5",
    developer: "#0b8aa5",
    aiModels: "#7048d8",
    system: "#5b6678",
    other: "#556b80",
    accent: "#0891b2",
    safe: "#15803d",
    yourCall: "#c2410c",
    keep: "#64748b",
    warning: "#b7791f",
    neutral: "#06243a",
    ghost: "#4a6378",
  },
};

// Brand: the Wrasse mark and app icon colors.
export const brand = {
  iconFrom: "#0a1a3a", // deep navy
  iconTo: "#0e7c86", // teal
  fish: "#ffffff",
  stripe: "#22e5ff",
  highlight: "rgba(255, 255, 255, 0.22)",
  ink: { dark: "#e8f6ff", light: "#06243a" }, // mono mark on each scheme
  trayPreview: { light: "#ececec", dark: "#1c1c1e" }, // menu bar backgrounds for previews
} as const;

// Reef motif: caustic light on the backdrop (very low contrast) and scan bubbles.
export const reef = {
  caustics: {
    dark: "radial-gradient(40% 30% at 20% 30%, rgba(120, 230, 255, 0.05) 0%, transparent 70%), radial-gradient(30% 25% at 70% 60%, rgba(120, 230, 255, 0.04) 0%, transparent 70%), radial-gradient(35% 28% at 45% 85%, rgba(120, 230, 255, 0.035) 0%, transparent 70%)",
    light: "radial-gradient(40% 30% at 20% 30%, rgba(8, 145, 178, 0.06) 0%, transparent 70%), radial-gradient(30% 25% at 70% 60%, rgba(8, 145, 178, 0.05) 0%, transparent 70%), radial-gradient(35% 28% at 45% 85%, rgba(8, 145, 178, 0.04) 0%, transparent 70%)",
  },
  bubble: { dark: "rgba(160, 235, 255, 0.35)", light: "rgba(8, 145, 178, 0.28)" },
  bubbleCount: 36,
} as const;

export const layout = {
  edge: "16px",
  barHeight: "44px",
  detailWidth: "340px",
  ring: "38px",
} as const;
