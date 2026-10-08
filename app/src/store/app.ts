import { create } from "zustand";
import { api, emptyDisk, GB, lensForFocus, throughSingles, type Disk } from "../api";
import { systemScheme } from "../design/apply";
import { categories, type LensId, type Scheme } from "../design/tokens";
import { t } from "../i18n";
import type { AppInfo } from "../lib/backend";
import { attached, detach, reattach, zeroOut } from "../mock/tree";
import { mapMotion } from "../motion/tokens";

export type Overlay = "startup" | "settings" | "audit" | "history" | "design" | null;
export type ThemePref = "system" | "dark" | "light";
export type MotionPref = "system" | "reduce" | "full";

export type Rect = { x: number; y: number; w: number; h: number };
export type TrayItem = { id: number; name: string; size: number };
export type AuditEntry = { at: number; action: string; target: string; size: number };
export type Batch = { id: number; at: number; items: TrayItem[]; state: "inTray" | "emptied" | "restored" };
export type Plan = { target: number; ids: number[]; total: number };
export type Flight = { key: number; id: number; from: Rect; color: string; delay: number };

type State = {
  info: AppInfo | null;
  disk: Disk;
  diskVersion: number;
  focus: number;
  lens: LensId;
  selected: number | null;
  detailOpen: boolean;
  tray: TrayItem[];
  batches: Batch[];
  audit: AuditEntry[];
  plan: Plan | null;
  flights: Flight[];
  /** Tiles that were just permanently deleted (they fade and collapse instead of flying). */
  deleting: number[];
  pulse: { id: number; key: number } | null;
  viewMode: "map" | "list";
  overlay: Overlay;
  commandOpen: boolean;
  scan: "idle" | "scanning" | "done";
  scanKey: number;
  toast: { key: number; text: string } | null;
  systemScheme: Scheme;
  theme: ThemePref;
  motion: MotionPref;
  /** The app whose uninstall sheet is open. */
  uninstall: number | null;
};

type Actions = {
  setInfo: (info: AppInfo) => void;
  setSystemScheme: (s: Scheme) => void;
  setFocus: (id: number) => void;
  zoomOut: () => void;
  setLens: (lens: LensId) => void;
  select: (id: number | null, open?: boolean) => void;
  closeDetail: () => void;
  addToTray: (ids: number[]) => void;
  deleteNow: (ids: number[]) => void;
  emptyTray: () => void;
  restoreTray: () => void;
  freeGhost: () => void;
  planFree: (bytes: number) => void;
  clearPlan: () => void;
  locate: (id: number) => void;
  startScan: () => void;
  scanDisk: () => Promise<void>;
  setViewMode: (mode: "map" | "list") => void;
  setOverlay: (o: Overlay) => void;
  setCommandOpen: (open: boolean) => void;
  notify: (text: string) => void;
  endFlight: (key: number) => void;
  setPrefs: (p: Partial<Pick<State, "theme" | "motion">>) => void;
  openUninstall: (appId: number) => void;
  closeUninstall: () => void;
  quitApp: (appId: number) => void;
  confirmUninstall: (appId: number, ids: number[]) => void;
};

let flightKey = 0;
let batchId = 0;
let scanTimer = 0;

/** The tile currently on screen that is, or contains, a node. */
export function visibleTileOf(disk: Disk, focus: number, id: number): number | null {
  for (let n = disk.nodes[id]; n; n = disk.nodes[n.parent]) {
    if (n.parent === focus) return n.id;
    if (n.parent < 0) return null;
  }
  return null;
}

function tileRect(id: number): Rect | null {
  const el = document.querySelector<HTMLElement>(`[data-tile-id="${id}"]`);
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.top, w: r.width, h: r.height };
}

export const useApp = create<State & Actions>((set, get) => {
  /** After removals, a level with nothing left goes back up to the nearest level that has items. */
  function ascendIfEmpty() {
    setTimeout(() => {
      const { disk, focus } = get();
      let f = focus;
      while (f !== disk.root && disk.nodes[f].children.every((c) => disk.nodes[c].size === 0 || !attached(disk, c))) f = disk.nodes[f].parent;
      if (f !== focus) set({ focus: f, lens: lensForFocus(disk, f), selected: null, detailOpen: false });
    }, 350);
  }

  function log(action: string, target: string, size: number) {
    set((s) => ({ audit: [{ at: Date.now(), action, target, size }, ...s.audit].slice(0, 200) }));
  }
  const scheme = () => {
    const s = get();
    return s.theme === "system" ? s.systemScheme : s.theme;
  };

  const disk = emptyDisk();
  return {
    info: null,
    disk,
    diskVersion: 0,
    focus: disk.root,
    lens: "all",
    selected: null,
    detailOpen: false,
    tray: [],
    batches: [],
    audit: [],
    plan: null,
    flights: [],
    deleting: [],
    pulse: null,
    viewMode: "map",
    overlay: null,
    commandOpen: false,
    scan: "idle",
    scanKey: 0,
    toast: null,
    systemScheme: systemScheme(),
    theme: "light", // Day reef first; night comes after the light theme is approved
    motion: "system",
    uninstall: null,

    setInfo: (info) => set({ info }),
    setSystemScheme: (systemScheme) => set({ systemScheme }),

    setFocus: (id) => {
      const n = get().disk.nodes[id];
      if (!n || (n.children.length === 0 && n.kind !== "volume")) return;
      const focus = throughSingles(get().disk, id);
      // The lens bar follows the category you open (AI Models → AI Models, Files → All).
      set({ focus, lens: lensForFocus(get().disk, focus), plan: null, selected: null, detailOpen: false });
    },
    zoomOut: () => {
      const { disk, focus, viewMode } = get();
      // Back from the list view returns to the tiles of the same folder.
      if (viewMode === "list") return set({ viewMode: "map", selected: null, detailOpen: false });
      // Skip back over single-child folders too, so going up always changes the view.
      let parent = disk.nodes[focus].parent;
      while (parent >= 0 && disk.nodes[parent].children.length === 1 && disk.nodes[parent].parent >= 0) parent = disk.nodes[parent].parent;
      if (parent >= 0) set({ focus: parent, lens: lensForFocus(disk, parent), plan: null, selected: null, detailOpen: false });
    },

    setLens: (lens) =>
      // "All" also takes you back home.
      lens === "all"
        ? set((s) => ({ lens, plan: null, focus: s.disk.root, viewMode: "map", selected: null, detailOpen: false }))
        : set({ lens, plan: null }),

    select: (id, open = true) => set({ selected: id, detailOpen: id !== null && open }),
    closeDetail: () => set({ detailOpen: false, selected: null }),

    addToTray: (ids) => {
      const { disk, focus } = get();
      const items = ids.map((id) => disk.nodes[id]).filter((n) => n && n.safety !== "keep" && n.kind !== "ghost" && n.parent >= 0);
      if (!items.length) return;
      // One flight per tile involved (many files can share a tile).
      const flights: Flight[] = [];
      const seen = new Set<number>();
      for (const n of items) {
        const tile = visibleTileOf(disk, focus, n.id);
        if (tile === null || seen.has(tile) || flights.length >= 12) continue;
        seen.add(tile);
        const rect = tileRect(tile);
        if (!rect) continue;
        const whole = tile === n.id;
        const from = whole ? rect : { x: rect.x + rect.w / 2 - 22, y: rect.y + rect.h / 2 - 22, w: 44, h: 44 };
        flights.push({ key: ++flightKey, id: n.id, from, color: categories[scheme()][disk.nodes[tile].category].accent, delay: flights.length * 0.05 });
      }
      detach(disk, items.map((n) => n.id));
      const trayItems = items.map((n) => ({ id: n.id, name: n.name, size: n.size }));
      set((s) => ({
        tray: [...s.tray, ...trayItems],
        batches: [{ id: ++batchId, at: Date.now(), items: trayItems, state: "inTray" }, ...s.batches],
        flights: [...s.flights, ...flights],
        diskVersion: s.diskVersion + 1,
        selected: null,
        detailOpen: false,
        plan: remainingPlan(s.plan, items.map((n) => n.id)),
      }));
      for (const n of items) log(t("audit.quarantined"), n.path, n.size);
      ascendIfEmpty();
    },

    deleteNow: (ids) => {
      const { disk } = get();
      const items = ids.map((id) => disk.nodes[id]).filter((n) => n && n.safety === "safe" && n.parent >= 0);
      if (!items.length) return;
      // Fade and collapse first, then remove (no flight, so the difference is visible).
      set({ deleting: items.map((n) => n.id), selected: null, detailOpen: false });
      setTimeout(() => {
        detach(disk, items.map((n) => n.id));
        set((s) => ({ deleting: [], diskVersion: s.diskVersion + 1 }));
        ascendIfEmpty();
      }, 260);
      for (const n of items) log(t("audit.deleted"), n.path, n.size);
    },

    emptyTray: () => {
      const { tray } = get();
      if (!tray.length) return;
      const total = tray.reduce((s, i) => s + i.size, 0);
      set((s) => ({ tray: [], batches: s.batches.map((b) => (b.state === "inTray" ? { ...b, state: "emptied" } : b)) }));
      log(t("audit.emptied"), t("tray.items", { count: tray.length }), total);
      get().notify(t("tray.emptiedToast"));
    },

    restoreTray: () => {
      const { tray, disk } = get();
      if (!tray.length) return;
      reattach(disk, tray.map((i) => i.id));
      set((s) => ({
        tray: [],
        diskVersion: s.diskVersion + 1,
        batches: s.batches.map((b) => (b.state === "inTray" ? { ...b, state: "restored" } : b)),
      }));
      log(t("audit.restored"), t("tray.items", { count: tray.length }), tray.reduce((s, i) => s + i.size, 0));
    },

    freeGhost: () => {
      const { disk } = get();
      const ghost = disk.nodes[disk.ghostId];
      if (!ghost || ghost.size === 0) return;
      const size = ghost.size;
      zeroOut(disk, disk.ghostId);
      set((s) => ({ diskVersion: s.diskVersion + 1, selected: null, detailOpen: false }));
      log(t("audit.snapshotsThinned"), "/", size);
      ascendIfEmpty();
    },

    planFree: (bytes) => {
      const { disk } = get();
      const ranked = disk.nodes
        .filter((n) => n.children.length === 0 && n.size > 0 && n.safety !== "keep" && n.kind !== "ghost" && n.parent >= 0 && attached(disk, n.id))
        .map((n) => ({ n, score: n.size * unusedFactor(n.lastOpenedDays) * (n.safety === "safe" ? 1 : 0.7) }))
        .sort((a, b) => b.score - a.score);
      const ids: number[] = [];
      let total = 0;
      for (const { n } of ranked) {
        if (total >= bytes) break;
        ids.push(n.id);
        total += n.size;
      }
      set({ plan: { target: bytes, ids, total }, lens: "all" });
    },
    clearPlan: () => set({ plan: null }),

    locate: (id) => {
      const { disk } = get();
      const n = disk.nodes[id];
      if (!n || n.parent < 0) return;
      set({ focus: n.parent, lens: lensForFocus(disk, n.parent), selected: id, detailOpen: false });
      setTimeout(() => set({ pulse: { id, key: Date.now() } }), 450);
    },

    startScan: () => {
      clearTimeout(scanTimer);
      set((s) => ({ scan: "scanning", scanKey: s.scanKey + 1, focus: s.disk.root, lens: "all", plan: null, selected: null, detailOpen: false }));
      scanTimer = window.setTimeout(() => set({ scan: "done" }), mapMotion.sonar * 1000);
    },
    scanDisk: async () => {
      const disk = await api.scan();
      set((s) => ({ disk, focus: disk.root, tray: [], plan: null, diskVersion: s.diskVersion + 1 }));
      get().startScan();
    },

    setViewMode: (viewMode) => set({ viewMode }),
    setOverlay: (overlay) => set({ overlay, commandOpen: false }),
    setCommandOpen: (commandOpen) => set({ commandOpen }),
    notify: (text) => set({ toast: { key: Date.now(), text } }),
    endFlight: (key) => set((s) => ({ flights: s.flights.filter((f) => f.key !== key) })),
    setPrefs: (p) => set(p),

    openUninstall: (appId) => set({ uninstall: appId, detailOpen: false }),
    closeUninstall: () => set({ uninstall: null }),
    quitApp: (appId) => {
      // Mock: Stage B asks the app to quit and waits for it.
      get().disk.nodes[appId].running = false;
      set((s) => ({ diskVersion: s.diskVersion + 1 }));
      log(t("audit.quit"), get().disk.nodes[appId].name, 0);
    },
    confirmUninstall: (appId, ids) => {
      const { disk } = get();
      set({ uninstall: null });
      get().addToTray(ids);
      log(t("audit.uninstalled"), disk.nodes[appId].path, ids.reduce((s, id) => s + disk.nodes[id].size, 0));
    },
  };
});

/** Drops tray-bound candidates from the plan; the plan ends once nothing is left. */
function remainingPlan(plan: Plan | null, removed: number[]): Plan | null {
  if (!plan) return null;
  const ids = plan.ids.filter((id) => !removed.includes(id));
  return ids.length ? { ...plan, ids } : null;
}

export function unusedFactor(days: number): number {
  if (days <= 0) return 0.1;
  if (days <= 30) return 0.1 + (0.4 * days) / 30;
  if (days <= 180) return 0.5 + (0.5 * (days - 30)) / 150;
  return 1;
}

export function usedBytes(disk: Disk): number {
  return disk.nodes[disk.root].size;
}

export function trayBytes(tray: TrayItem[]): number {
  return tray.reduce((s, i) => s + i.size, 0);
}

export const DEFAULT_FREE_TARGET = 30 * GB;
