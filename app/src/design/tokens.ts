// Design tokens for "Reef Blocks" (D1, locked): every color, gradient, radius, shadow, blur,
// size and lens color. Components read them through CSS variables (apply.ts) or directly.
// Nothing else hardcodes a visual value. Day reef (light) is primary; night is provisional
// until the light theme is approved.

export type Scheme = "dark" | "light";

export type Category = "files" | "aiModels" | "developer" | "apps" | "system" | "junk" | "other";
export type LensId = "all" | "junk" | "apps" | "aiModels" | "dev" | "largeOld" | "grew";
/** One of these is selected. Choosing it opens that part of the disk. */
export type CategoryId = "all" | "junk" | "apps" | "aiModels" | "dev";
/** These combine with the category and with each other. Turning one on does not navigate. */
export type FilterId = "largeOld" | "grew";
export type DiskFilters = { largeOld: boolean; grew: boolean };
export type Safety = "safe" | "yourCall" | "keep";
type ChipTone = "amber" | "green" | "violet" | "protected";

// UI chrome colors.
export const palette = {
  light: {
    canvas: "#EEF5FA",
    text: "#0B1A2E",
    textMuted: "#5B6B7E",
    textFaint: "rgba(11, 26, 46, 0.40)",
    glass: "rgba(255, 255, 255, 0.75)",
    glassStrong: "rgba(255, 255, 255, 0.88)",
    glassBorder: "rgba(11, 26, 46, 0.08)",
    glassHighlight: "rgba(255, 255, 255, 0.9)",
    scrim: "rgba(11, 26, 46, 0.18)",
    divider: "rgba(11, 26, 46, 0.08)",
    track: "rgba(11, 26, 46, 0.08)",
    pill: "rgba(11, 26, 46, 0.06)",
    accent: "#0F766E",
    accentHover: "#0B5A54",
    onAccent: "#FFFFFF",
    navy: "#0B1A2E",
    onNavy: "#FFFFFF",
    inactive: "#33445A",
    safe: "#166534",
    yourCall: "#8A4B07",
    keep: "#334155",
    focus: "#2F5BD3",
    dev: "#DC2626",
    onDev: "#FFFFFF",
    kbdBorder: "rgba(11, 26, 46, 0.12)",
  },
  dark: {
    canvas: "#071320",
    text: "#E8F1FA",
    textMuted: "#93A3B6",
    textFaint: "rgba(232, 241, 250, 0.40)",
    glass: "rgba(16, 30, 50, 0.72)",
    glassStrong: "rgba(20, 36, 60, 0.9)",
    glassBorder: "rgba(255, 255, 255, 0.10)",
    glassHighlight: "rgba(255, 255, 255, 0.12)",
    scrim: "rgba(2, 8, 18, 0.5)",
    divider: "rgba(255, 255, 255, 0.08)",
    track: "rgba(255, 255, 255, 0.12)",
    pill: "rgba(255, 255, 255, 0.10)",
    accent: "#2DD4BF",
    accentHover: "#5EEAD4",
    onAccent: "#04201D",
    navy: "#E8F1FA",
    onNavy: "#0B1A2E",
    inactive: "#B4C2D3",
    safe: "#86EFAC",
    yourCall: "#FCD34D",
    keep: "#CBD5E1",
    focus: "#7DA2F5",
    dev: "#EF4444",
    onDev: "#FFFFFF",
    kbdBorder: "rgba(255, 255, 255, 0.16)",
  },
} as const;

// Window background.
export const backdrop: Record<Scheme, string> = {
  light: "linear-gradient(160deg, #F2FBFA 0%, #EAF4FB 60%, #EEF0FB 100%)",
  dark: "linear-gradient(160deg, #071A1F 0%, #0A1626 60%, #0D1330 100%)",
};

type CategoryStyle = {
  accent: string; // icon badge, size bars, selection outline
  ink: string; // big size number, row icons
  from: string; // tile gradient, 160deg
  to: string;
  border: string;
  glow: string; // faint circle top-right
  shadow: string; // badge shadow color
};

// Category colors, exactly as in the D1 mockup.
export const categories: Record<Scheme, Record<Category, CategoryStyle>> = {
  light: {
    files: { accent: "#0F766E", ink: "#0B4F4A", from: "#E3F6F3", to: "#CDEDE8", border: "rgba(15, 118, 110, 0.18)", glow: "rgba(15, 118, 110, 0.10)", shadow: "rgba(15, 118, 110, 0.35)" },
    aiModels: { accent: "#6D4FD8", ink: "#3F2A9E", from: "#F0ECFE", to: "#E2DAFC", border: "rgba(109, 79, 216, 0.20)", glow: "rgba(109, 79, 216, 0.12)", shadow: "rgba(109, 79, 216, 0.35)" },
    developer: { accent: "#0E7490", ink: "#0B4C5E", from: "#E4F4FA", to: "#CFEAF4", border: "rgba(14, 116, 144, 0.20)", glow: "rgba(14, 116, 144, 0.10)", shadow: "rgba(14, 116, 144, 0.35)" },
    apps: { accent: "#2F5BD3", ink: "#1E3F99", from: "#E8EFFE", to: "#D6E2FC", border: "rgba(47, 91, 211, 0.20)", glow: "rgba(47, 91, 211, 0.10)", shadow: "rgba(47, 91, 211, 0.35)" },
    system: { accent: "#475569", ink: "#2B3646", from: "#EEF1F5", to: "#E1E6EE", border: "rgba(71, 85, 105, 0.18)", glow: "rgba(71, 85, 105, 0.08)", shadow: "rgba(71, 85, 105, 0.30)" },
    junk: { accent: "#B45309", ink: "#7A3A06", from: "#FEF6E7", to: "#FCE9C6", border: "rgba(180, 83, 9, 0.22)", glow: "rgba(180, 83, 9, 0.10)", shadow: "rgba(180, 83, 9, 0.35)" },
    other: { accent: "#475569", ink: "#2B3646", from: "#F1F4F8", to: "#E6EBF1", border: "rgba(71, 85, 105, 0.16)", glow: "rgba(71, 85, 105, 0.06)", shadow: "rgba(71, 85, 105, 0.25)" },
  },
  // Provisional night values: same hues, deeper gradients.
  dark: {
    files: { accent: "#14B8A6", ink: "#99F6E4", from: "#0E2E2C", to: "#0B2422", border: "rgba(45, 212, 191, 0.20)", glow: "rgba(45, 212, 191, 0.10)", shadow: "rgba(0, 0, 0, 0.45)" },
    aiModels: { accent: "#8B6CF0", ink: "#D4C8FE", from: "#221A44", to: "#1A1436", border: "rgba(139, 108, 240, 0.24)", glow: "rgba(139, 108, 240, 0.12)", shadow: "rgba(0, 0, 0, 0.45)" },
    developer: { accent: "#0EA5C6", ink: "#A5E8F7", from: "#0C2A36", to: "#0A202B", border: "rgba(14, 165, 198, 0.22)", glow: "rgba(14, 165, 198, 0.10)", shadow: "rgba(0, 0, 0, 0.45)" },
    apps: { accent: "#4F7BF0", ink: "#C3D3FD", from: "#152548", to: "#101C38", border: "rgba(79, 123, 240, 0.22)", glow: "rgba(79, 123, 240, 0.10)", shadow: "rgba(0, 0, 0, 0.45)" },
    system: { accent: "#64748B", ink: "#CBD5E1", from: "#1A2230", to: "#141B27", border: "rgba(148, 163, 184, 0.18)", glow: "rgba(148, 163, 184, 0.06)", shadow: "rgba(0, 0, 0, 0.45)" },
    junk: { accent: "#D97706", ink: "#FCD9A0", from: "#33230C", to: "#2A1C09", border: "rgba(217, 119, 6, 0.24)", glow: "rgba(217, 119, 6, 0.10)", shadow: "rgba(0, 0, 0, 0.45)" },
    other: { accent: "#64748B", ink: "#CBD5E1", from: "#172030", to: "#121A27", border: "rgba(148, 163, 184, 0.14)", glow: "rgba(148, 163, 184, 0.05)", shadow: "rgba(0, 0, 0, 0.45)" },
  },
};

// Insight chips: amber = attention, green = safe, violet = AI, slate = protected.
export const chips: Record<Scheme, Record<ChipTone, { bg: string; text: string }>> = {
  light: {
    amber: { bg: "#FCE7C3", text: "#8A4B07" },
    green: { bg: "#D3F2DE", text: "#166534" },
    violet: { bg: "#E2DAFC", text: "#4C2DB0" },
    protected: { bg: "#E1E6EE", text: "#334155" },
  },
  dark: {
    amber: { bg: "rgba(252, 211, 77, 0.16)", text: "#FCD34D" },
    green: { bg: "rgba(134, 239, 172, 0.14)", text: "#86EFAC" },
    violet: { bg: "rgba(196, 181, 253, 0.16)", text: "#C4B5FD" },
    protected: { bg: "rgba(203, 213, 225, 0.12)", text: "#CBD5E1" },
  },
};

// Lens colors (used for row dots and list markers; the lens itself dims non-matching tiles).
export const lensColors: Record<Scheme, Record<Exclude<LensId, "all">, string>> = {
  light: { junk: "#B45309", apps: "#2F5BD3", aiModels: "#6D4FD8", dev: "#0E7490", largeOld: "#C2410C", grew: "#15803D" },
  dark: { junk: "#F59E0B", apps: "#7DA2F5", aiModels: "#A78BFA", dev: "#22D3EE", largeOld: "#FB923C", grew: "#4ADE80" },
};

// Mac window controls, drawn in the browser preview (the Tauri window shows the real ones).
export const trafficLights = ["#FF5F57", "#FEBC2E", "#28C840"] as const;

// D1 layout and component metrics (px unless noted).
export const d1 = {
  header: { height: 72, padX: 24, gap: 16, titleSize: 17 },
  traffic: { size: 12, gap: 7 },
  command: { maxWidth: 520, height: 40, radius: 12, padX: 14, gap: 10, iconSize: 16, fontSize: 14, kbdSize: 12, shadow: "0 4px 18px rgba(11, 26, 46, 0.06)" },
  free: { size: 30, unitSize: 14 },
  button: { height: 40, padX: 18, radius: 12, fontSize: 14 },
  // Tile area follows size^emphasis: 0.5 (square root) keeps the biggest block biggest without
  // squeezing the rest. maxShare caps the biggest block's share of the grid; minCells is the
  // smallest tile (2 × 3 cells), enough for badge, name, size and a chip.
  grid: { cols: 12, rows: 8, gap: 12, padX: 24, padBottom: 92, emphasis: 0.5, maxShare: 0.4, minCells: 6, maxTiles: 12 },
  tile: {
    radius: 20,
    padding: "16px 16px 14px 16px",
    gap: 10,
    shadow: "0 1px 0 rgba(255, 255, 255, 0.9) inset, 0 10px 30px rgba(11, 26, 46, 0.07)",
    hoverShadow: "0 1px 0 rgba(255, 255, 255, 0.9) inset, 0 18px 40px rgba(11, 26, 46, 0.13)",
    hoverLift: -3,
    glowSize: 140,
    glowOffset: -30,
    badge: 34,
    badgeRadius: 10,
    badgeShadow: "0 6px 14px",
    badgeIcon: 20,
    nameSize: 15,
    metaSize: 12,
    unitSize: 13,
    sizeXL: 44,
    sizeL: 38,
    sizeM: 32,
    sizeS: 24,
    selectionWidth: 2.5,
    shortBelow: 100, // px tile height: below this, one line (name + size)
    chipsFrom: 150, // px tile height needed to show insight chips
    chipsMinWidth: 150, // px tile width needed for chips
    openMinWidth: 240, // px tile width needed for "Open →" beside the chips
    rowBarMinWidth: 300, // px tile width needed for the size bar in item rows
    inlineSize: 15,
  },
  row: { height: 38, radius: 10, padX: 10, gap: 10, bg: "rgba(255, 255, 255, 0.72)", bgDark: "rgba(255, 255, 255, 0.06)", fontSize: 13, iconSize: 16, barW: 54, barH: 5, barRadius: 3, sizeW: 58, stackGap: 6 },
  chip: { fontSize: 12, padding: "5px 10px", gap: 6 },
  ghost: { radius: 16, border: "1.5px dashed rgba(11, 26, 46, 0.25)", borderDark: "1.5px dashed rgba(255, 255, 255, 0.25)", bg: "rgba(255, 255, 255, 0.4)", bgDark: "rgba(255, 255, 255, 0.04)", padX: 14, titleSize: 13, hintSize: 11, actionSize: 12 },
  lens: { bottom: 22, gap: 4, pad: 6, radius: 18, bg: "rgba(255, 255, 255, 0.82)", bgDark: palette.dark.glass, shadow: "0 10px 30px rgba(11, 26, 46, 0.12)", pillHeight: 40, pillPadX: 16, pillRadius: 12, fontSize: 14 },
  detail: { width: 330, bg: "rgba(255, 255, 255, 0.86)", bgDark: palette.dark.glassStrong },
  level: { pad: 14 }, // inset of child tiles inside an opened tile
  dimmed: 0.35,
  frameRadius: 14,
} as const;

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
export const weight = { regular: 400, name: 500, size: 600, strong: 700 } as const;
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

export type Tone = Category | "accent" | "safe" | "yourCall" | "keep" | "warning" | "neutral" | "ghost" | "white";

// Icon tones per scheme.
export const tones: Record<Scheme, Record<Tone, string>> = {
  light: {
    files: "#0B4F4A",
    aiModels: "#3F2A9E",
    developer: "#0B4C5E",
    apps: "#1E3F99",
    system: "#2B3646",
    junk: "#7A3A06",
    other: "#2B3646",
    accent: "#0F766E",
    safe: "#166534",
    yourCall: "#8A4B07",
    keep: "#334155",
    warning: "#B45309",
    neutral: "#0B1A2E",
    ghost: "#5B6B7E",
    white: "#FFFFFF",
  },
  dark: {
    files: "#99F6E4",
    aiModels: "#D4C8FE",
    developer: "#A5E8F7",
    apps: "#C3D3FD",
    system: "#CBD5E1",
    junk: "#FCD9A0",
    other: "#CBD5E1",
    accent: "#2DD4BF",
    safe: "#86EFAC",
    yourCall: "#FCD34D",
    keep: "#CBD5E1",
    warning: "#F59E0B",
    neutral: "#E8F1FA",
    ghost: "#93A3B6",
    white: "#FFFFFF",
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
    light: "radial-gradient(40% 30% at 20% 30%, rgba(15, 118, 110, 0.035) 0%, transparent 70%), radial-gradient(30% 25% at 70% 60%, rgba(47, 91, 211, 0.03) 0%, transparent 70%), radial-gradient(35% 28% at 45% 85%, rgba(15, 118, 110, 0.03) 0%, transparent 70%)",
    dark: "radial-gradient(40% 30% at 20% 30%, rgba(120, 230, 255, 0.05) 0%, transparent 70%), radial-gradient(30% 25% at 70% 60%, rgba(120, 230, 255, 0.04) 0%, transparent 70%), radial-gradient(35% 28% at 45% 85%, rgba(120, 230, 255, 0.035) 0%, transparent 70%)",
  },
  bubble: { light: "rgba(15, 118, 110, 0.35)", dark: "rgba(160, 235, 255, 0.35)" },
  bubbleCount: 24,
  sonar: { light: "rgba(15, 118, 110, 0.45)", dark: "rgba(45, 212, 191, 0.5)" },
} as const;

export const radius = {
  macos: { sm: "8px", md: "12px", lg: "20px", bar: "18px", pill: "999px" },
  windows: { sm: "5px", md: "8px", lg: "12px", bar: "12px", pill: "999px" },
} as const;

export const shadow = {
  light: {
    glass: "0 10px 30px rgba(11, 26, 46, 0.12)",
    pill: "0 4px 12px rgba(11, 26, 46, 0.10)",
    flyer: "0 10px 26px rgba(11, 26, 46, 0.22)",
  },
  dark: {
    glass: "0 18px 50px rgba(0, 4, 16, 0.55), 0 2px 8px rgba(0, 4, 16, 0.35)",
    pill: "0 4px 14px rgba(0, 0, 0, 0.35)",
    flyer: "0 10px 30px rgba(0, 0, 0, 0.5)",
  },
} as const;

// Blur only on glass elements.
export const blur = {
  glass: "saturate(160%) blur(20px)",
} as const;

export const layout = {
  edge: "24px",
  detailWidth: "330px",
} as const;
