// Command bar: parses what the user typed into ranked, runnable suggestions.
import type { LensId } from "../design/tokens";
import { t } from "../i18n";
import { formatBytes } from "../lib/format";
import { GB } from "../api/types";
import { useApp, type Overlay } from "../store/app";

export type Suggestion = {
  key: string;
  label: string;
  hint?: string;
  run: () => void;
};

/** Subsequence fuzzy score: higher is better, -1 when `query` doesn't match. */
export function fuzzy(query: string, text: string): number {
  const q = query.toLowerCase();
  const s = text.toLowerCase();
  if (!q) return 0;
  const direct = s.indexOf(q);
  if (direct >= 0) return 1000 - direct * 2 - (s.length - q.length) * 0.1 + (direct === 0 ? 200 : 0);
  let score = 0;
  let last = -1;
  for (const ch of q) {
    const i = s.indexOf(ch, last + 1);
    if (i < 0) return -1;
    score += i === last + 1 ? 12 : 2;
    if (i === 0 || " -_./".includes(s[i - 1])) score += 8;
    last = i;
  }
  return score - s.length * 0.05;
}

const LENS_WORDS: [LensId, string[]][] = [
  ["all", ["all", "everything", "reset"]],
  ["junk", ["junk", "caches", "cache", "trash", "logs"]],
  ["apps", ["apps", "applications"]],
  ["aiModels", ["models", "ai", "ai models", "llm"]],
  ["dev", ["dev", "developer", "code", "node_modules"]],
  ["largeOld", ["large", "old", "large & old", "big files"]],
  ["grew", ["grew", "growth", "what grew"]],
];

const OVERLAYS: [Exclude<Overlay, null>, string[]][] = [
  ["startup", ["startup items", "login items"]],
  ["settings", ["settings", "preferences"]],
  ["audit", ["audit log", "log", "history of actions"]],
  ["history", ["quarantine history", "restore history"]],
];

export function suggest(raw: string): Suggestion[] {
  const app = useApp.getState();
  const text = raw.trim();
  const lower = text.toLowerCase();
  const out: { s: Suggestion; score: number }[] = [];
  const push = (s: Suggestion, score: number) => score >= 0 && out.push({ s, score });

  // free 30 GB
  const free = /^(?:free|f|fr|fre)\s*(\d+(?:\.\d+)?)\s*(tb|t|gb|g)?$/i.exec(text);
  if (free) {
    const n = parseFloat(free[1]) * ((free[2] ?? "gb").toLowerCase().startsWith("t") ? 1000 : 1);
    push({ key: "free", label: t("cmd.free", { size: `${n} GB` }), hint: t("cmd.freeHint"), run: () => app.planFree(n * GB) }, 5000);
  } else if (!text || fuzzy(lower, "free 30 gb") > 0) {
    push({ key: "free30", label: t("cmd.free", { size: "30 GB" }), hint: t("cmd.freeHint"), run: () => app.planFree(30 * GB) }, text ? fuzzy(lower, "free 30 gb") : 900);
  }

  // Lenses: "show models", "junk", "what grew"
  const lensQuery = lower.replace(/^(show|lens)\s+/, "");
  for (const [lens, words] of LENS_WORDS) {
    const score = text ? Math.max(...words.map((w) => fuzzy(lensQuery, w))) : lens === "junk" ? 800 : lens === "grew" ? 700 : -1;
    push({ key: `lens-${lens}`, label: lens === "grew" ? t("cmd.whatGrew") : t("cmd.show", { lens: t(`lens.${lens}`) }), hint: t("cmd.lensHint"), run: () => app.setLens(lens) }, score);
  }

  // Scan, views, overlays, tray
  const actions: [string, string[], () => void][] = [
    ["scan", ["scan", "rescan", "sonar"], () => app.startScan()],
    ["list", ["list view", "list"], () => app.setViewMode("list")],
    ["map", ["map view", "map"], () => app.setViewMode("map")],
    ["out", ["zoom out", "back", "up"], () => app.zoomOut()],
    ["home", ["home", "whole disk", "top"], () => app.setFocus(app.disk.root)],
    ["empty", ["empty tray", "empty quarantine"], () => app.emptyTray()],
    ["restore", ["restore tray", "undo"], () => app.restoreTray()],
  ];
  for (const [key, words, run] of actions) {
    const score = text ? Math.max(...words.map((w) => fuzzy(lower, w))) : key === "scan" ? 600 : -1;
    push({ key, label: t(`cmd.${key}`), run }, score);
  }
  for (const [overlay, words] of OVERLAYS) {
    const score = text ? Math.max(...words.map((w) => fuzzy(lower.replace(/^open\s+/, ""), w))) : overlay === "settings" ? 300 : -1;
    push({ key: `overlay-${overlay}`, label: t(`overlay.${overlay}`), hint: t("cmd.openHint"), run: () => app.setOverlay(overlay) }, score);
  }

  // where is <name> (also any free text that matches a file or folder name)
  const where = /^(?:where\s*is|find|locate|where)\s+(.+)$/i.exec(text);
  const nameQuery = where ? where[1] : text.length >= 2 ? text : "";
  if (nameQuery) {
    const units = app.info?.units ?? "decimal";
    const hits: { id: number; score: number }[] = [];
    for (const n of app.disk.nodes) {
      if (n.parent < 0 || n.size === 0 || n.kind === "ghost") continue;
      const s = fuzzy(nameQuery, n.name);
      if (s > 0) hits.push({ id: n.id, score: s + Math.log10(n.size) * 4 - n.depth * 3 });
    }
    hits.sort((a, b) => b.score - a.score);
    for (const h of hits.slice(0, 6)) {
      const n = app.disk.nodes[h.id];
      const f = formatBytes(n.size, units);
      push({ key: `where-${n.id}`, label: t("cmd.whereIs", { name: n.name }), hint: `${n.path} · ${f.value} ${f.unit}`, run: () => app.locate(n.id) }, (where ? 2000 : 400) + h.score);
    }
  } else if (!text) {
    push({ key: "where-hint", label: t("cmd.whereIs", { name: "Xcode" }), hint: t("cmd.whereHint"), run: () => app.locate(app.disk.nodes.find((n) => n.name === "Xcode.app")?.id ?? app.disk.root) }, 500);
  }

  return out
    .sort((a, b) => b.score - a.score)
    .slice(0, 8)
    .map((o) => o.s);
}
