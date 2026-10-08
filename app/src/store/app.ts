import { create } from "zustand";
import { systemScheme } from "../design/apply";
import type { LensId, Scheme } from "../design/tokens";
import { t } from "../i18n";
import type { AppInfo } from "../lib/backend";
import { GB, generateDisk, type Disk, type DiskNode } from "../map/data";
import { renderer } from "../map/controller";
import type { Rect } from "../map/layout";
import type { RemovalMode } from "../map/renderer";
import { detach, reattach, zeroOut } from "../map/tree";

export type Overlay = "startup" | "settings" | "audit" | "history" | null;
export type ThemePref = "system" | "dark" | "light";
export type MotionPref = "system" | "reduce" | "full";
export type GlassMode = "native" | "css";

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
  viewMode: "map" | "list";
  overlay: Overlay;
  commandOpen: boolean;
  scan: "idle" | "scanning" | "done";
  toast: { key: number; text: string } | null;
  systemScheme: Scheme;
  theme: ThemePref;
  motion: MotionPref;
  glass: GlassMode;
  nativeGlass: boolean; // the backend reported native glass support
  showFps: boolean;
  blockCount: 2000 | 20000;
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
  finishScan: () => void;
  setViewMode: (mode: "map" | "list") => void;
  setOverlay: (o: Overlay) => void;
  setCommandOpen: (open: boolean) => void;
  notify: (text: string) => void;
  endFlight: (key: number) => void;
  setPrefs: (p: Partial<Pick<State, "theme" | "motion" | "glass" | "showFps" | "nativeGlass">>) => void;
  setBlockCount: (n: 2000 | 20000) => void;
};

let flightKey = 0;
let batchId = 0;

export const useApp = create<State & Actions>((set, get) => {
  const disk = mockDisk(2000);

  function log(action: string, target: string, size: number) {
    set((s) => ({ audit: [{ at: Date.now(), action, target, size }, ...s.audit].slice(0, 200) }));
  }

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
    viewMode: "map",
    overlay: null,
    commandOpen: false,
    scan: "idle",
    toast: null,
    systemScheme: systemScheme(),
    theme: "system",
    motion: "system",
    glass: "native",
    nativeGlass: false,
    showFps: false,
    blockCount: 2000,

    setInfo: (info) => set({ info }),
    setSystemScheme: (systemScheme) => set({ systemScheme }),

    setFocus: (id) => {
      const n = get().disk.nodes[id];
      if (!n || (n.children.length === 0 && n.kind !== "volume")) return;
      set({ focus: id });
    },
    zoomOut: () => {
      const { disk, focus } = get();
      const parent = disk.nodes[focus].parent;
      if (parent >= 0) set({ focus: parent });
    },

    setLens: (lens) => set({ lens, plan: null }),

    select: (id, open = true) => set({ selected: id, detailOpen: id !== null && open }),
    closeDetail: () => set({ detailOpen: false }),

    addToTray: (ids) => {
      const { disk } = get();
      const items = ids
        .map((id) => disk.nodes[id])
        .filter((n) => n && n.safety !== "keep" && n.kind !== "ghost" && n.parent >= 0);
      if (!items.length) return;
      const r = renderer();
      const flights: Flight[] = [];
      items.forEach((n, i) => {
        const rect = r?.screenRect(n.id);
        if (rect) flights.push({ key: ++flightKey, id: n.id, from: rect, color: nodeColor(n), delay: Math.min(i, 12) * 0.04 });
      });
      detach(disk, items.map((n) => n.id));
      const trayItems = items.map((n) => ({ id: n.id, name: n.name, size: n.size }));
      const batch: Batch = { id: ++batchId, at: Date.now(), items: trayItems, state: "inTray" };
      set((s) => ({
        tray: [...s.tray, ...trayItems],
        batches: [batch, ...s.batches],
        flights: [...s.flights, ...flights],
        diskVersion: s.diskVersion + 1,
        selected: s.selected !== null && items.some((n) => n.id === s.selected) ? null : s.selected,
        detailOpen: s.selected !== null && items.some((n) => n.id === s.selected) ? false : s.detailOpen,
        plan: remainingPlan(s.plan, items.map((n) => n.id)),
      }));
      r?.relayout(new Map(items.map((n) => [n.id, "quarantine" as RemovalMode])));
      for (const n of items) log(t("audit.quarantined"), n.path, n.size);
    },

    deleteNow: (ids) => {
      const { disk } = get();
      const items = ids.map((id) => disk.nodes[id]).filter((n) => n && n.safety === "safe" && n.parent >= 0);
      if (!items.length) return;
      detach(disk, items.map((n) => n.id));
      set((s) => ({
        diskVersion: s.diskVersion + 1,
        selected: null,
        detailOpen: false,
      }));
      renderer()?.relayout(new Map(items.map((n) => [n.id, "delete" as RemovalMode])));
      for (const n of items) log(t("audit.deleted"), n.path, n.size);
    },

    emptyTray: () => {
      const { tray } = get();
      if (!tray.length) return;
      const total = tray.reduce((s, i) => s + i.size, 0);
      set((s) => ({
        tray: [],
        batches: s.batches.map((b) => (b.state === "inTray" ? { ...b, state: "emptied" } : b)),
      }));
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
      renderer()?.relayout();
      log(t("audit.restored"), t("tray.items", { count: tray.length }), tray.reduce((s, i) => s + i.size, 0));
    },

    freeGhost: () => {
      const { disk } = get();
      const ghost = disk.nodes[disk.ghostId];
      if (ghost.size === 0) return;
      const size = ghost.size;
      zeroOut(disk, disk.ghostId);
      set((s) => ({ diskVersion: s.diskVersion + 1, selected: null, detailOpen: false }));
      renderer()?.relayout(new Map([[disk.ghostId, "delete" as RemovalMode]]));
      log(t("audit.snapshotsThinned"), "/", size);
    },

    planFree: (bytes) => {
      const r = renderer();
      const { disk } = get();
      const blocks = r?.view?.blocks ?? [];
      const ranked = blocks
        .map((b) => disk.nodes[b.id])
        .filter((n) => n.safety !== "keep" && n.kind !== "ghost")
        .map((n) => ({ n, score: n.size * unusedFactor(n.lastOpenedDays) * (n.safety === "safe" ? 1 : 0.7) }))
        .sort((a, b) => b.score - a.score);
      const ids: number[] = [];
      let total = 0;
      for (const { n } of ranked) {
        if (total >= bytes) break;
        ids.push(n.id);
        total += n.size;
      }
      set({ plan: { target: bytes, ids, total }, lens: "all", viewMode: get().viewMode });
    },
    clearPlan: () => set({ plan: null }),

    locate: (id) => {
      const { disk } = get();
      const n = disk.nodes[id];
      if (!n) return;
      const focus = n.children.length > 0 && n.parent >= 0 ? disk.nodes[n.parent].id : n.parent >= 0 ? n.parent : disk.root;
      set({ focus, selected: id, detailOpen: false });
      const pulse = () => renderer()?.pulse(id);
      // Pulse once the camera has arrived.
      setTimeout(pulse, 450);
      setTimeout(pulse, 1100);
    },

    startScan: () => {
      const r = renderer();
      if (!r || r.scanning) return;
      set({ scan: "scanning", focus: get().disk.root, plan: null });
      r.startScan(() => get().finishScan());
    },
    finishScan: () => set({ scan: "done" }),

    setViewMode: (viewMode) => set({ viewMode }),
    setOverlay: (overlay) => set({ overlay, commandOpen: false }),
    setCommandOpen: (commandOpen) => set({ commandOpen }),
    notify: (text) => set({ toast: { key: Date.now(), text } }),
    endFlight: (key) => set((s) => ({ flights: s.flights.filter((f) => f.key !== key) })),
    setPrefs: (p) => set(p),
    setBlockCount: (blockCount) => {
      const disk = mockDisk(blockCount);
      set({ blockCount, disk, focus: disk.root, selected: null, detailOpen: false, tray: [], plan: null, diskVersion: get().diskVersion + 1 });
      renderer()?.setDisk(disk);
    },
  };
});

/** Drops tray-bound candidates from the plan; the plan ends once nothing is left. */
function remainingPlan(plan: Plan | null, removed: number[]): Plan | null {
  if (!plan) return null;
  const ids = plan.ids.filter((id) => !removed.includes(id));
  return ids.length ? { ...plan, ids } : null;
}

/** ~2,000 visible blocks, or the 20,000-file stress disk. */
function mockDisk(count: 2000 | 20000): Disk {
  return count === 2000 ? generateDisk(3500) : generateDisk(20000, true);
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

// Resolved by the map module; set at startup so flights can match block colors.
let colorOf: (n: DiskNode) => string = () => "";
export function setNodeColorResolver(fn: (n: DiskNode) => string) {
  colorOf = fn;
}
function nodeColor(n: DiskNode): string {
  return colorOf(n);
}
