// Command bar: parses what the user typed into ranked, runnable suggestions.
import type { IconName } from "../../design/icons/nodes";
import { iconFor, toneFor } from "../../design/icons/semantic";
import type { CategoryId, FilterId, Tone } from "../../design/tokens";
import { LENSES } from "../browse/lenses";
import { t } from "../../i18n";
import { formatBytes } from "../../lib/format";
import { api, MOCK_KEY, mockMode, type ExportTable } from "../../api";
import { exportTable, FORMATS, freePlan, history, scanSummary } from "../export/exports";
import { GB } from "../../api/types";
import { inTauri } from "../../api/env";
import { useApp, type Overlay } from "../../store/app";

type Suggestion = {
  key: string;
  label: string;
  hint?: string;
  icon: IconName;
  tone?: Tone;
  run: () => void;
};

/** Subsequence fuzzy score: higher is better, -1 when `query` doesn't match. */
function fuzzy(query: string, text: string): number {
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

const CATEGORY_WORDS: [CategoryId, string[]][] = [
  ["all", ["all", "everything", "reset"]],
  ["junk", ["junk", "caches", "cache", "trash", "logs"]],
  ["apps", ["apps", "applications"]],
  ["aiModels", ["models", "ai", "ai models", "llm"]],
  ["dev", ["dev", "developer", "code", "node_modules"]],
];

const FILTER_WORDS: [FilterId, string[]][] = [
  ["largeOld", ["large", "old", "large & old", "big files"]],
  ["grew", ["grew", "growth", "what grew"]],
];

const ACTION_ICONS: Record<string, IconName> = {
  scan: "scan",
  list: "list",
  map: "grid",
  out: "chevronRight",
  home: "system",
  empty: "trash",
  space: "info",
  restore: "restore",
  tray: "tray",
  undo: "restore",
  design: "settings",
  stop: "close",
  pause: "history",
  resume: "scan",
  permissions: "keep",
  mock: "settings",
};

/** Dev only: switch between the real scanner and mock data, then reload. */
function setMock(on: boolean) {
  try {
    localStorage.setItem(MOCK_KEY, on ? "1" : "0");
  } catch {
    return;
  }
  location.reload();
}

const OVERLAYS: [Exclude<Overlay, null | "design" | "onboarding">, string[]][] = [
  ["startup", ["startup items", "login items"]],
  ["settings", ["settings", "preferences"]],
  ["audit", ["audit log", "log", "history of actions"]],
  ["history", ["history", "deleted", "deleted items", "what did i delete", "quarantine history", "restore history"]],
  ["junk", ["system junk", "junk", "clean junk", "caches", "clean up"]],
  ["largeOld", ["large files", "old files", "large and old", "big files", "duplicates", "duplicate files"]],
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
    push({ key: "free", icon: "free", tone: "accent", label: t("cmd.free", { size: `${n} GB` }), hint: t("cmd.freeHint"), run: () => app.planFree(n * GB) }, 5000);
  } else if (!text || fuzzy(lower, "free 30 gb") > 0) {
    push({ key: "free30", icon: "free", tone: "accent", label: t("cmd.free", { size: "30 GB" }), hint: t("cmd.freeHint"), run: () => app.planFree(30 * GB) }, text ? fuzzy(lower, "free 30 gb") : 900);
  }

  // Categories open that part of the disk. Filters narrow the folder you're already in.
  const lensQuery = lower.replace(/^(show|lens|filter)\s+/, "");
  for (const [lens, words] of CATEGORY_WORDS) {
    const score = text ? Math.max(...words.map((w) => fuzzy(lensQuery, w))) : lens === "junk" ? 800 : -1;
    push({ key: `lens-${lens}`, icon: LENSES.find((l) => l.id === lens)!.icon, label: t("cmd.show", { lens: t(`lens.${lens}`) }), hint: t("cmd.lensHint"), run: () => app.setLens(lens) }, score);
  }
  for (const [lens, words] of FILTER_WORDS) {
    const score = text ? Math.max(...words.map((w) => fuzzy(lensQuery, w))) : lens === "grew" ? 700 : -1;
    push({ key: `filter-${lens}`, icon: LENSES.find((l) => l.id === lens)!.icon, label: lens === "grew" ? t("cmd.whatGrew") : t("cmd.filter", { name: t(`lens.${lens}`) }), hint: t("cmd.filterHint"), run: () => app.setFilter(lens, true) }, score);
  }

  // Scan, views, overlays, tray
  const actions: [string, string[], () => void][] = [
    ["scan", ["scan", "rescan", "sonar"], () => app.startScan()],
    ["list", ["list view", "list"], () => app.setViewMode("list")],
    ["map", ["map view", "map"], () => app.setViewMode("map")],
    ["out", ["zoom out", "back", "up"], () => app.zoomOut()],
    ["home", ["home", "whole disk", "top"], () => app.setFocus(app.disk.root)],
    ["tray", ["tray", "quarantine", "open tray", "restore", "put back"], () => app.setTrayOpen(true)],
    ["undo", ["undo", "undo delete"], () => app.undoLastMove()],
    ["empty", ["empty tray", "empty quarantine"], () => app.requestEmptyTray()],
    ["space", ["why isn't this free", "why isnt this free", "snapshots", "purgeable", "not back yet"], () => app.explainSpace()],
    ["restore", ["restore tray", "restore all", "restore everything"], () => app.restoreTray()],
    ["design", ["design system", "design"], () => app.setOverlay("design")],
    ["permissions", ["permissions", "full disk access", "welcome", "onboarding"], () => app.setOverlay("onboarding")],
  ];
  // Exports (PLAN.md §6.14): one command per report and format.
  const reports: [string, string[], () => ExportTable | null][] = [
    ["summary", ["export scan", "export summary", "export report", "export"], scanSummary],
    ["history", ["export history"], () => history(app.deleted)],
  ];
  if (app.plan) reports.push(["plan", ["export plan", "export free up space"], freePlan]);
  for (const [report, words, build] of reports) {
    for (const f of FORMATS) {
      const score = text ? Math.max(...words.map((w) => fuzzy(lower, `${w} ${f.label.toLowerCase()}`))) : -1;
      push({ key: `export-${report}-${f.id}`, icon: "audit", label: t(`export.cmd.${report}`, { format: f.label }), hint: t("export.hint"), run: () => { const table = build(); if (table) exportTable(table, f.id); } }, score);
    }
  }
  if (app.scan === "scanning") {
    actions.push(["stop", ["stop scan", "cancel scan", "stop"], () => app.cancelScan()]);
    actions.push(["pause", ["pause scan", "pause"], () => void api.pauseScan(true)]);
    actions.push(["resume", ["resume scan", "resume"], () => void api.pauseScan(false)]);
  }
  if (inTauri && app.info?.dev) {
    actions.push(["mock", mockMode() ? ["use real data", "real data", "real scan"] : ["use mock data", "mock data", "mock"], () => setMock(!mockMode())]);
  }
  for (const [key, words, run] of actions) {
    const score = text ? Math.max(...words.map((w) => fuzzy(lower, w))) : key === "scan" ? 600 : -1;
    push({ key, icon: ACTION_ICONS[key] ?? "info", label: key === "mock" ? t(mockMode() ? "cmd.realData" : "cmd.mockData") : t(`cmd.${key}`), run }, score);
  }
  for (const [overlay, words] of OVERLAYS) {
    const score = text ? Math.max(...words.map((w) => fuzzy(lower.replace(/^open\s+/, ""), w))) : overlay === "settings" ? 300 : -1;
    push({ key: `overlay-${overlay}`, icon: overlay, label: t(`overlay.${overlay}`), hint: t("cmd.openHint"), run: () => app.setOverlay(overlay) }, score);
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
      push({ key: `where-${n.id}`, icon: iconFor(n), tone: toneFor(n), label: t("cmd.whereIs", { name: n.name }), hint: `${n.path} · ${f.value} ${f.unit}`, run: () => app.locate(n.id) }, (where ? 2000 : 400) + h.score);
    }
  } else if (!text) {
    push({ key: "where-hint", icon: "search", label: t("cmd.whereIs", { name: "Xcode" }), hint: t("cmd.whereHint"), run: () => app.locate(app.disk.nodes.find((n) => n.name === "Xcode.app")?.id ?? app.disk.root) }, 500);
  }

  return out
    .sort((a, b) => b.score - a.score)
    .slice(0, 8)
    .map((o) => o.s);
}
