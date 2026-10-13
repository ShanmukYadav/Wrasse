import { create } from "zustand";
import { type DiskNode, type FreeWhy, type LoadedModel, type PathRemoval, type ScannedFile, type SpaceReport, type TrayItem, api, detach, mockMode, attached, cautionFor, deletable, categoryForLens, strongest, emptyDisk, lensForFocus, throughSingles, type Disk, type ScanProgress } from "../api";
import { systemScheme } from "../design/apply";
import { categories, type CategoryId, type DiskFilters, type FilterId, type Scheme } from "../design/tokens";
import { t } from "../i18n";
import type { AppInfo } from "../api/env";
import { bytesText } from "../lib/format";
import { mapMotion } from "../motion/tokens";
import { showAllScope } from "../ui/scope";

export type Overlay = "onboarding" | "junk" | "startup" | "settings" | "audit" | "history" | "design" | "largeOld" | null;
type ThemePref = "system" | "dark" | "light";
type MotionPref = "system" | "reduce" | "full";

export type Rect = { x: number; y: number; w: number; h: number };
type AuditEntry = { at: number; action: string; target: string; size: number };
type Batch = { id: number; at: number; items: TrayItem[]; state: "inTray" | "emptied" | "restored" };
/** A Free Up Space plan: ranked candidates; `off` are the ones the user unticked. */
/** One deleted item, for History: what it was, where, how big, and whether it can come back. */
export type DeletedEntry = {
  id: number;
  name: string;
  path: string;
  size: number;
  category: DiskNode["category"];
  /** What kind of thing it was, for the label: app, appData, model, dev, cache, folder, file, snapshots. */
  what: "app" | "appData" | "model" | "dev" | "cache" | "folder" | "file" | "snapshots";
  at: number;
  status: "inTray" | "restored" | "deleted";
  /** The tray entry, while it can be restored. */
  entry?: string;
};

export type ConfirmKind =
  /** OS folders, protected data, iCloud-synced items: needs "I understand". */
  | "caution"
  /** Delete for good, Empty tray, Empty Trash: "Not undoable". */
  | "deleteNow"
  | "emptyTray"
  | "emptyTrash"
  /** Thin local snapshots. Not undoable. */
  | "thinSnapshots"
  /** Homebrew cleanup, Docker prune, or unavailable simulators. Not undoable. */
  | "devDelegate";

/** A short message; with `action`, a button (e.g. Undo) and a longer stay. */
type Toast = { key: number; text: string; action?: { label: string; run: () => void } };

export type Plan = {
  target: number;
  /** Candidates, best first: the node shown for each (an app, a folder, a file). */
  ids: number[];
  /** What goes for each candidate (for an app: its bundle and the leftovers ticked by default). */
  parts: Record<number, number[]>;
  bytes: Record<number, number>;
  why: Record<number, FreeWhy>;
  total: number;
  off: number[];
  /** Missing bytes when the target can't be reached. */
  shortfall: number;
  /** Left out because used in the last 7 days. */
  heldRecent: number;
  /** Left out because their app is open. */
  heldRunning: number;
  /** When the plan was made: tray items since then count as "pending in tray". */
  startedAt: number;
  /** Freed for good since then (Delete now, emptied tray or Trash, Homebrew uninstalls). */
  freed: number;
  /** Moved to the Trash by Finder since then (frees once the Trash is emptied). */
  inTrash: number;
};
export type Flight = { key: number; id: number; from: Rect; color: string; delay: number };

type State = {
  info: AppInfo | null;
  disk: Disk;
  diskVersion: number;
  focus: number;
  /** The selected category. Large & Old and Grew live in `filters`. */
  lens: CategoryId;
  filters: DiskFilters;
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
  /** Live progress of a real scan (null in between). */
  scanProgress: ScanProgress | null;
  /** Full Disk Access (macOS); null until checked. */
  fullDiskAccess: boolean | null;
  toast: Toast | null;
  /** The tray panel (bottom right) is open. */
  trayOpen: boolean;
  /** Days a batch stays in the tray before it's emptied automatically. */
  keepDays: number;
  /** What the last move to the tray moved, for Undo and ⌘Z. */
  lastMove: TrayItem[];
  systemScheme: Scheme;
  theme: ThemePref;
  motion: MotionPref;
  /** The app whose uninstall sheet is open. */
  uninstall: number | null;
  /** The app Wrasse is asking to quit (waits up to 10 s). */
  quitting: number | null;
  /** Everything deleted this session, newest first (History). */
  deleted: DeletedEntry[];
  /** Tiles ticked for a bulk action. */
  checked: number[];
  /** A removal waiting for confirmation (caution, or "Not undoable"). */
  confirmDelete: { ids: number[]; kind: ConfirmKind; handPicked: boolean } | null;
  /** Large & Old files in Documents or Desktop, waiting on the full caution sheet. */
  fileCaution: { files: ScannedFile[]; run: () => void } | null;
  /** Dry-run: actions only report what they would do (dev builds start in it every launch). */
  dryRun: boolean;
  /** Expected versus actual free space, after a cleanup or when the user asks. */
  spaceReport: SpaceReport | null;
  /** Models Ollama or LM Studio currently have loaded. Empty until a lens or detail panel asks. */
  loadedModels: LoadedModel[];
};

type Actions = {
  setInfo: (info: AppInfo) => void;
  setSystemScheme: (s: Scheme) => void;
  setFocus: (id: number) => void;
  zoomOut: () => void;
  setLens: (lens: CategoryId) => void;
  /** Turns one filter on or off. Stays on the current folder. */
  toggleFilter: (id: FilterId) => void;
  /** Turns one filter on or off without navigating. */
  setFilter: (id: FilterId, on: boolean) => void;
  /** Widens to the whole disk and shows the map. The active filters stay on. */
  showAll: () => void;
  select: (id: number | null, open?: boolean) => void;
  closeDetail: () => void;
  addToTray: (ids: number[], confirmed?: boolean, handPicked?: boolean) => void;
  toggleCheck: (id: number) => void;
  clearChecks: () => void;
  /** Delete (to the tray): goes straight through, or asks first for OS folders and protected data. */
  requestDelete: (ids: number[], handPicked?: boolean) => void;
  requestDeleteNow: (ids: number[]) => void;
  requestEmptyTray: () => void;
  setDryRun: (on: boolean) => void;
  confirmDeleteRequest: () => void;
  cancelDelete: () => void;
  /** Opens the full caution sheet for files in Documents or Desktop, then runs `run`. */
  openFileCaution: (files: ScannedFile[], run: () => void) => void;
  confirmFileCaution: () => void;
  cancelFileCaution: () => void;
  deleteNow: (ids: number[]) => void;
  emptyTray: () => void;
  restoreTray: () => void;
  /** Puts one item from the tray back. */
  restoreItem: (id: number) => void;
  /** Puts these tray items back (one item, a batch, anything). */
  restoreEntries: (items: TrayItem[]) => void;
  /** Undo (toast, ⌘Z, ⌘K): puts back what the last move to the tray moved. */
  undoLastMove: () => void;
  /** Reads which models are loaded. Does not unload or remove anything. */
  refreshLoaded: () => void;
  /** Asks Ollama to unload one model. */
  unloadModel: (name: string) => void;
  setTrayOpen: (open: boolean) => void;
  /** Moves files picked by path (Large & Old, Duplicates) to the tray; resolves to the paths moved. */
  moveFilesToTray: (run: () => Promise<PathRemoval>) => Promise<string[]>;
  freeGhost: () => void;
  /** Shows why free space is not what a cleanup was expected to leave. Read-only. */
  explainSpace: () => void;
  planFree: (bytes: number) => void;
  clearPlan: () => void;
  togglePlanItem: (id: number) => void;
  setPlanAll: (on: boolean) => void;
  locate: (id: number) => void;
  startScan: () => void;
  scanDisk: () => Promise<void>;
  cancelScan: () => void;
  checkFullDiskAccess: () => Promise<void>;
  finishOnboarding: () => void;
  setViewMode: (mode: "map" | "list") => void;
  setOverlay: (o: Overlay) => void;
  setCommandOpen: (open: boolean) => void;
  notify: (text: string, action?: Toast["action"]) => void;
  endFlight: (key: number) => void;
  setPrefs: (p: Partial<Pick<State, "theme" | "motion">>) => void;
  openUninstall: (appId: number) => void;
  closeUninstall: () => void;
  quitApp: (appId: number) => void;
  /** Uninstalls the app; `leftoverIds` are the ticked leftovers (they go to the tray). */
  confirmUninstall: (appId: number, leftoverIds: number[]) => void;
};

const ONBOARDED_KEY = "wrasse.onboarded";
/** How long after launch the automatic scan starts (see boot()). */
const LAUNCH_SCAN_DELAY_MS = 6000;
let flightKey = 0;
let batchId = 0;
let scanTimer = 0;

/** The tile currently on screen that is, or contains, a node. */
function visibleTileOf(disk: Disk, focus: number, id: number): number | null {
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

  /** Phase 1 only scans: cleanup actions explain that instead of pretending. */
  function readOnly(): boolean {
    if (!get().info?.readOnly) return false;
    get().notify(t("readOnly.toast"));
    return true;
  }

  const PARTLY_DONE = /(?:Emptying the Trash|Moving it to the Trash) stopped after 2 minutes and may be partly done\./;
  const failed = (e: unknown) => {
    const raw = e instanceof Error ? e.message : typeof e === "string" ? e : String(e);
    // A Finder action stopped at the 2-minute limit is an error. Show that sentence on its own,
    // never a success toast.
    get().notify(raw.match(PARTLY_DONE)?.[0] ?? raw);
    return null;
  };
  const fmtBytes = (b: number) => bytesText(b, get().info?.units ?? "decimal");
  function reportSkipped(skipped: number | { reason: string }[]) {
    const count = typeof skipped === "number" ? skipped : skipped.length;
    if (!count) return;
    const sinceScan = typeof skipped !== "number" && skipped.every((s) => s.reason === "changedSinceScan");
    get().notify(t(sinceScan ? "delete.changedSinceScanToast" : "delete.skippedToast", { count }));
  }

  /** Delete (to the tray): the core decides; only what it actually moved flies to the tray. */
  async function moveToTray(ids: number[], confirmed: boolean, handPicked: boolean) {
    const { disk, focus } = get();
    const before = await spaceAvailable();
    // Keep items only move after the user confirmed the caution sheet; blocked ones never.
    const items = ids.map((id) => disk.nodes[id]).filter((n) => n && deletable(disk, n.id) && (confirmed || n.safety !== "keep"));
    if (!items.length) return;
    const result = await api.remove(disk, items.map((n) => n.id), { handPicked }).catch(failed);
    if (!result) return;
    if (result.dryRun) {
      set({ selected: null, detailOpen: false });
      await publishSpace(before, 0);
      const released = result.done.filter((d) => !d.entry && items.some((n) => n.id === d.id && n.model?.removal === "ollama"));
      const key = released.length && released.length === result.done.length ? "dryRun.deleteToast" : "dryRun.trayToast";
      return get().notify(t(key, { count: result.done.length, size: fmtBytes(result.bytes) }), { label: t("dryRun.turnOff"), run: () => get().setOverlay("settings") });
    }
    const entries = new Map(result.done.map((d) => [d.id, d.entry]));
    const moved = items.filter((n) => entries.has(n.id));
    // Ollama's own removal has no tray entry and can't be undone. Quarantine does.
    const trayMoved = moved.filter((n) => entries.get(n.id));
    const released = moved.filter((n) => !entries.get(n.id));
    // One flight per tile involved (many files can share a tile).
    const flights: Flight[] = [];
    const seen = new Set<number>();
    for (const n of trayMoved) {
      const tile = visibleTileOf(disk, focus, n.id);
      if (tile === null || seen.has(tile) || flights.length >= 12) continue;
      seen.add(tile);
      const rect = tileRect(tile);
      if (!rect) continue;
      const whole = tile === n.id;
      const from = whole ? rect : { x: rect.x + rect.w / 2 - 22, y: rect.y + rect.h / 2 - 22, w: 44, h: 44 };
      flights.push({ key: ++flightKey, id: n.id, from, color: categories[scheme()][disk.nodes[tile].category].accent, delay: flights.length * 0.05 });
    }
    const at = Date.now();
    const batch = `session-${batchId + 1}`;
    const trayItems: TrayItem[] = trayMoved.map((n) => ({ id: n.id, name: n.name, size: n.size, entry: entries.get(n.id), path: n.path, batch, at }));
    const releasedBytes = released.reduce((s, n) => s + n.size, 0);
    await publishSpace(before, releasedBytes);
    set((s) => ({
      disk: result.disk,
      diskVersion: s.diskVersion + 1,
      tray: [...s.tray, ...trayItems],
      batches: trayItems.length ? [{ id: ++batchId, at: Date.now(), items: trayItems, state: "inTray" as const }, ...s.batches] : s.batches,
      flights: [...s.flights, ...flights],
      selected: null,
      detailOpen: false,
      checked: s.checked.filter((c) => !moved.some((n) => n.id === c)),
      plan: countFreed(remainingPlan(s.plan, moved.map((n) => n.id)), releasedBytes),
      lastMove: trayItems.length ? trayItems : s.lastMove,
    }));
    for (const n of trayMoved) log(t("audit.quarantined"), n.path, n.size);
    for (const n of released) log(t("audit.deleted"), n.path, n.size);
    record(trayMoved, "inTray", entries);
    record(released, "deleted");
    if (trayItems.length) {
      const size = fmtBytes(trayItems.reduce((s, i) => s + i.size, 0));
      get().notify(t("tray.movedToast", { count: trayItems.length, size }), { label: t("tray.undo"), run: () => get().undoLastMove() });
      void syncTray();
    } else if (released.length) {
      get().notify(t("model.removed", { count: released.length, size: fmtBytes(releasedBytes) }));
    }
    reportSkipped(result.skipped);
    ascendIfEmpty();
  }

  async function restoreItems(items: TrayItem[]) {
    if (!items.length) return;
    const result = await api.restore(get().disk, items).catch(failed);
    if (!result) return;
    const ids = new Set(items.map((i) => i.entry ?? String(i.id)));
    set((s) => ({
      disk: result.disk,
      diskVersion: s.diskVersion + 1,
      tray: s.tray.filter((i) => !ids.has(i.entry ?? String(i.id))),
      lastMove: s.lastMove.filter((i) => !ids.has(i.entry ?? String(i.id))),
      batches: s.tray.length === items.length ? s.batches.map((b) => (b.state === "inTray" ? { ...b, state: "restored" } : b)) : s.batches,
      deleted: s.deleted.map((d) => (d.status === "inTray" && items.some((i) => i.id === d.id || (i.entry && i.entry === d.entry)) ? { ...d, status: "restored" } : d)),
    }));
    log(t("audit.restored"), t("tray.items", { count: items.length }), items.reduce((s, i) => s + i.size, 0));
    // Items from before a relaunch aren't in this tree: a quick (incremental) rescan shows them.
    if (result.rescan) void get().scanDisk();
  }

  /** Uninstall: the bundle goes by its route; ticked leftovers go to the tray. */
  async function uninstallApp(appId: number, leftoverIds: number[]) {
    const { disk, focus } = get();
    const app = disk.nodes[appId];
    if (!app) return;
    const bundle = app.children.map((c) => disk.nodes[c]).find((c) => c.appBundle);
    const tile = visibleTileOf(disk, focus, appId);
    const rect = tile === null ? null : tileRect(tile);
    const r = await api.uninstall(disk, appId, leftoverIds).catch(failed);
    if (!r) return;
    if (r.dryRun) {
      return get().notify(t("dryRun.uninstallToast", { name: app.name, route: t(`uninstall.routeShort.${r.route.kind}`) }), {
        label: t("dryRun.turnOff"),
        run: () => get().setOverlay("settings"),
      });
    }
    const at = Date.now();
    const batch = `session-${batchId + 1}`;
    const entries = new Map<number, string | undefined>(r.leftovers.map((l) => [l.id, l.entry]));
    const parts: DiskNode[] = r.leftovers.map((l) => disk.nodes[l.id]);
    if (r.bundleRemoved && bundle) {
      parts.unshift(bundle);
      if (r.bundleEntry) entries.set(bundle.id, r.bundleEntry);
    }
    const toTray = parts.filter((n) => entries.get(n.id));
    const trayItems: TrayItem[] = toTray.map((n) => ({ id: n.id, name: n === bundle ? n.name : `${app.name} · ${n.name}`, size: n.size, entry: entries.get(n.id), path: n.path, batch, at }));
    const flights: Flight[] =
      rect && tile !== null && parts.length
        ? [{ key: ++flightKey, id: appId, from: rect, color: categories[scheme()][disk.nodes[tile].category].accent, delay: 0 }]
        : [];
    set((s) => ({
      disk: r.disk,
      diskVersion: s.diskVersion + 1,
      tray: [...s.tray, ...trayItems],
      batches: trayItems.length ? [{ id: ++batchId, at, items: trayItems, state: "inTray" as const }, ...s.batches] : s.batches,
      flights: [...s.flights, ...flights],
      selected: null,
      detailOpen: false,
      lastMove: trayItems.length ? trayItems : s.lastMove,
      plan: countFreed(
        remainingPlan(s.plan, [appId, ...parts.map((n) => n.id)]),
        r.bundleRemoved && r.route.kind === "homebrew" ? r.bundleBytes : 0,
        r.bundleRemoved && r.route.kind === "finderTrash" ? r.bundleBytes : 0,
      ),
    }));
    record(toTray, "inTray", entries);
    if (r.bundleRemoved && bundle && !r.bundleEntry) record([bundle], "deleted");
    log(t("audit.uninstalled"), app.path, r.bundleBytes + r.leftoverBytes);
    const size = fmtBytes(r.bundleBytes + r.leftoverBytes);
    if (!r.bundleRemoved) {
      get().notify(t("uninstall.bundleStayed", { name: app.name, why: t(`skip.${r.bundleSkipped ?? "failed"}`) }));
    } else if (r.route.kind === "tray") {
      get().notify(t("uninstall.toastTray", { name: app.name, size }), { label: t("tray.undo"), run: () => get().undoLastMove() });
    } else if (r.route.kind === "homebrew") {
      get().notify(trayItems.length ? t("uninstall.toastBrewData", { name: app.name }) : t("uninstall.toastBrew", { name: app.name }));
    } else {
      get().notify(trayItems.length ? t("uninstall.toastFinderData", { name: app.name }) : t("uninstall.toastFinder", { name: app.name }));
    }
    reportSkipped(r.skipped);
    void syncTray();
    ascendIfEmpty();
  }

  /** Takes the core's batch ids and times for the tray (the UI's own are provisional). */
  async function syncTray() {
    if (mockMode()) return;
    const status = await api.cleanupStatus().catch(() => null);
    if (!status) return;
    set((s) => {
      const nodeOf = new Map(s.tray.filter((i) => i.entry).map((i) => [i.entry, i.id]));
      const lastMove = new Set(s.lastMove.map((i) => i.entry));
      const tray = status.tray.map((i) => ({ ...i, id: nodeOf.get(i.entry) ?? i.id }));
      return { tray, keepDays: status.keepDays, lastMove: tray.filter((i) => lastMove.has(i.entry)) };
    });
  }

  async function spaceAvailable(): Promise<number | null> {
    const report = await api.spaceReport(null, 0).catch(() => null);
    return report ? report.available : null;
  }

  async function publishSpace(before: number | null, freedNow: number) {
    if (before === null) return;
    const spaceReport = await api.spaceReport(before, freedNow).catch(() => null);
    if (spaceReport) set({ spaceReport });
  }

  async function runDelegated(id: number) {
    const node = get().disk.nodes[id];
    const action = node?.delegateAction;
    if (!node || !action) return;
    const before = await spaceAvailable();
    const result = await api.delegateAction(action).catch(failed);
    if (!result) return;
    await publishSpace(before, result.dryRun ? 0 : result.bytes);
    if (result.dryRun) return get().notify(t("dryRun.delegateToast", { name: node.name }));
    if (action === "thinSnapshots") {
      const report = get().spaceReport;
      set((s) => {
        const disk = s.disk;
        const ghost = disk.ghostId >= 0 ? disk.nodes[disk.ghostId] : undefined;
        if (ghost && report) {
          ghost.size = report.snapshotBytes;
          if (report.snapshotBytes === 0) {
            const parent = ghost.parent >= 0 ? disk.nodes[ghost.parent] : undefined;
            if (parent) parent.children = parent.children.filter((c) => c !== ghost.id);
            disk.ghostId = -1;
          }
        }
        return { disk, diskVersion: s.diskVersion + 1, selected: null, detailOpen: false };
      });
      log(t("audit.snapshotsThinned"), node.path, result.bytes);
    }
    get().notify(t("space.done", { name: node.name }));
    ascendIfEmpty();
  }

  async function emptyTrash(id: number) {
    const { disk } = get();
    const n = disk.nodes[id];
    const before = await spaceAvailable();
    const result = await api.emptyTrash(disk, id).catch(failed);
    if (!result) return;
    await publishSpace(before, result.dryRun ? 0 : result.bytes);
    if (result.dryRun) return get().notify(t("dryRun.trashToast", { size: fmtBytes(result.bytes) }));
    // What Finder moved to the Trash for this plan is freed now too.
    set((s) => ({ disk: result.disk, diskVersion: s.diskVersion + 1, selected: null, detailOpen: false, plan: s.plan ? { ...s.plan, freed: s.plan.freed + result.bytes, inTrash: 0 } : null }));
    if (n) {
      log(t("audit.deleted"), n.path, result.bytes);
      record([n], "deleted");
    }
    get().notify(t("trash.emptiedToast", { size: fmtBytes(result.bytes) }));
    ascendIfEmpty();
  }

  /** Adds deleted items to History. */
  function record(items: DiskNode[], status: DeletedEntry["status"], trayEntries?: Map<number, string | undefined>) {
    const at = Date.now();
    const entries = items.map((n) => ({ id: n.id, name: n.name, path: n.path, size: n.size, category: n.category, what: whatItWas(n), at, status, entry: trayEntries?.get(n.id) }));
    set((s) => ({ deleted: [...entries, ...s.deleted].slice(0, 500) }));
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
    filters: { largeOld: false, grew: false },
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
    scanProgress: null,
    fullDiskAccess: null,
    toast: null,
    trayOpen: false,
    keepDays: 7,
    lastMove: [],
    systemScheme: systemScheme(),
    theme: "light", // Day reef first; night comes after the light theme is approved
    motion: "system",
    uninstall: null,
    quitting: null,
    checked: [],
    confirmDelete: null,
    fileCaution: null,
    deleted: [],
    dryRun: false,
    spaceReport: null,
    loadedModels: [],

    setInfo: (info) => set({ info }),
    setSystemScheme: (systemScheme) => set({ systemScheme }),

    setFocus: (id) => {
      const n = get().disk.nodes[id];
      if (!n || (n.children.length === 0 && n.kind !== "volume")) return;
      const focus = throughSingles(get().disk, id);
      // The lens bar follows the category you open (AI Models → AI Models, Files → All).
      set({ focus, lens: lensForFocus(get().disk, focus), plan: null, selected: null, detailOpen: false, checked: [] });
    },
    zoomOut: () => {
      const { disk, focus, viewMode } = get();
      // Back from the list view returns to the tiles of the same folder.
      if (viewMode === "list") return set({ viewMode: "map", selected: null, detailOpen: false });
      // Skip back over single-child folders too, so going up always changes the view.
      let parent = disk.nodes[focus].parent;
      while (parent >= 0 && disk.nodes[parent].children.length === 1 && disk.nodes[parent].parent >= 0) parent = disk.nodes[parent].parent;
      if (parent >= 0) set({ focus: parent, lens: lensForFocus(disk, parent), plan: null, selected: null, detailOpen: false, checked: [] });
    },

    setLens: (lens) =>
      // "All" also takes you back home. Filters stay on.
      set((s) => {
        const target = lens === "all" ? s.disk.root : categoryForLens(s.disk, lens);
        if (target === null) return { lens, plan: null };
        return { lens, plan: null, focus: target, viewMode: "map", selected: null, detailOpen: false, checked: [] };
      }),
    toggleFilter: (id) => set((s) => ({ filters: { ...s.filters, [id]: !s.filters[id] }, plan: null })),
    setFilter: (id, on) => set((s) => ({ filters: { ...s.filters, [id]: on }, plan: null })),
    showAll: () =>
      set((s) => {
        // The empty-state link only widens the folder. Grew (and Large & Old) stay on.
        const next = showAllScope(s.filters);
        return {
          lens: next.category,
          filters: next.filters,
          focus: s.disk.root,
          viewMode: "map",
          plan: null,
          selected: null,
          detailOpen: false,
          checked: [],
        };
      }),

    select: (id, open = true) => set({ selected: id, detailOpen: id !== null && open }),
    closeDetail: () => set({ detailOpen: false, selected: null }),

    addToTray: (ids, confirmed = false, handPicked = true) => void moveToTray(ids, confirmed, handPicked),

    toggleCheck: (id) => set((s) => ({ checked: s.checked.includes(id) ? s.checked.filter((c) => c !== id) : [...s.checked, id] })),
    clearChecks: () => set({ checked: [] }),
    requestDelete: (ids, handPicked = true) => {
      const { disk } = get();
      // The OS does these. They are not undoable, so always confirm, and don't send them to the tray.
      // A mixed selection keeps the files on the normal path; the tool action is confirmed on its own.
      const delegated = ids.filter((id) => disk.nodes[id]?.delegate);
      const files = ids.filter((id) => !disk.nodes[id]?.delegate);
      if (delegated.length > 0 && files.length === 0) {
        const id = delegated[0];
        const action = disk.nodes[id].delegateAction ?? "emptyTrash";
        const kind: ConfirmKind = action === "thinSnapshots" ? "thinSnapshots" : action === "emptyTrash" ? "emptyTrash" : "devDelegate";
        return set({ confirmDelete: { ids: [id], kind, handPicked }, detailOpen: false });
      }
      const ok = files.filter((id) => deletable(disk, id));
      if (!ok.length) return get().notify(ids.some((id) => cautionFor(disk, id).kind === "category") ? t("delete.categoryToast") : t("delete.blockedToast"));
      const ollama = ok.some((id) => disk.nodes[id].model?.removal === "ollama");
      // Ollama may remove the model itself. That has to be confirmed, because it can't be undone.
      if (strongest(disk, ok) === "none" && !ollama) return get().addToTray(ok, false, handPicked);
      set({ confirmDelete: { ids: ok, kind: "caution", handPicked }, detailOpen: false });
    },
    requestDeleteNow: (ids) => set({ confirmDelete: { ids, kind: "deleteNow", handPicked: true }, detailOpen: false }),
    requestEmptyTray: () => {
      if (get().tray.length) set({ confirmDelete: { ids: [], kind: "emptyTray", handPicked: true } });
    },
    confirmDeleteRequest: () => {
      const c = get().confirmDelete;
      set({ confirmDelete: null });
      if (!c) return;
      if (c.kind === "caution") get().addToTray(c.ids, true, c.handPicked);
      else if (c.kind === "deleteNow") get().deleteNow(c.ids);
      else if (c.kind === "emptyTray") get().emptyTray();
      else if (c.kind === "emptyTrash") void emptyTrash(c.ids[0]);
      else if (c.kind === "thinSnapshots" || c.kind === "devDelegate") void runDelegated(c.ids[0]);
    },
    cancelDelete: () => set({ confirmDelete: null }),
    openFileCaution: (files, run) => set({ fileCaution: { files, run }, confirmDelete: null }),
    confirmFileCaution: () => {
      const run = get().fileCaution?.run;
      set({ fileCaution: null });
      run?.();
    },
    cancelFileCaution: () => set({ fileCaution: null }),
    setDryRun: (on) => {
      set({ dryRun: on });
      void api.setDryRun(on);
    },

    deleteNow: (ids) => {
      void (async () => {
        const { disk } = get();
        const items = ids.map((id) => disk.nodes[id]).filter((n) => n && n.safety === "safe" && deletable(disk, n.id));
        if (!items.length) return get().notify(t("delete.notSafeToast"));
        const before = await spaceAvailable();
        const result = await api.remove(disk, items.map((n) => n.id), { handPicked: true, deleteNow: true }).catch(failed);
        if (!result) return;
        await publishSpace(before, result.dryRun ? 0 : result.bytes);
        if (result.dryRun) return get().notify(t("dryRun.deleteToast", { count: result.done.length, size: fmtBytes(result.bytes) }));
        const done = new Set(result.done.map((d) => d.id));
        const gone = items.filter((n) => done.has(n.id));
        // Fade and collapse (no flight: nothing goes to the tray).
        set({ deleting: gone.map((n) => n.id), selected: null, detailOpen: false });
        setTimeout(() => {
          set((s) => ({ disk: result.disk, deleting: [], diskVersion: s.diskVersion + 1, plan: countFreed(remainingPlan(s.plan, gone.map((n) => n.id)), result.bytes) }));
          ascendIfEmpty();
        }, 260);
        for (const n of gone) log(t("audit.deleted"), n.path, n.size);
        record(gone, "deleted");
        reportSkipped(result.skipped);
      })();
    },

    emptyTray: () => {
      void (async () => {
        const { tray } = get();
        if (!tray.length) return;
        const before = await spaceAvailable();
        const result = await api.emptyTray(tray).catch(failed);
        if (!result) return;
        await publishSpace(before, result.dryRun ? 0 : result.bytes);
        if (result.dryRun) return get().notify(t("dryRun.emptyTrayToast", { size: fmtBytes(result.bytes) }));
        set((s) => ({
          tray: [],
          lastMove: [],
          plan: countFreed(s.plan, result.bytes),
          batches: s.batches.map((b) => (b.state === "inTray" ? { ...b, state: "emptied" } : b)),
          deleted: s.deleted.map((d) => (d.status === "inTray" ? { ...d, status: "deleted" } : d)),
        }));
        log(t("audit.emptied"), t("tray.items", { count: tray.length }), result.bytes);
        get().notify(t("tray.emptiedToast"));
      })();
    },

    restoreTray: () => void restoreItems(get().tray),
    restoreEntries: (items) => void restoreItems(items),
    refreshLoaded: () => {
      void api.loadedModels().then((loadedModels) => set({ loadedModels })).catch(() => {});
    },
    unloadModel: (name) => {
      void (async () => {
        const result = await api.unloadModel(name).catch(failed);
        if (!result) return;
        if (result.dryRun) return get().notify(t("dryRun.unloadToast", { name }));
        get().notify(t("model.unloaded", { name }));
        const loadedModels = await api.loadedModels().catch(() => get().loadedModels);
        set({ loadedModels });
      })();
    },
    undoLastMove: () => {
      const keys = new Set(get().tray.map((i) => i.entry ?? String(i.id)));
      const items = get().lastMove.filter((i) => keys.has(i.entry ?? String(i.id)));
      if (!items.length) return get().notify(t("tray.nothingToUndo"));
      set({ lastMove: [] });
      void restoreItems(items).then(() => get().notify(t("tray.undoneToast", { count: items.length })));
    },
    setTrayOpen: (open) => set({ trayOpen: open }),
    moveFilesToTray: async (run) => {
      if (readOnly()) return [];
      const before = await spaceAvailable();
      const r = await run().catch(failed);
      if (!r) return [];
      await publishSpace(before, 0);
      if (r.dryRun) {
        get().notify(t("dryRun.trayToast", { count: r.done.length, size: fmtBytes(r.bytes) }), { label: t("dryRun.turnOff"), run: () => get().setOverlay("settings") });
        return [];
      }
      const { disk } = get();
      const byPath = new Map(disk.nodes.filter((n) => n.parent >= 0).map((n) => [n.path, n.id]));
      const at = Date.now();
      const batch = `session-${batchId + 1}`;
      const items: TrayItem[] = r.done.map((d) => ({ id: byPath.get(d.path) ?? -1, name: d.path.split("/").pop() ?? d.path, size: d.bytes, entry: d.entry, path: d.path, batch, at }));
      const known = items.filter((i) => i.id >= 0).map((i) => i.id);
      detach(disk, known);
      set((s) => ({
        diskVersion: s.diskVersion + 1,
        tray: [...s.tray, ...items],
        batches: items.length ? [{ id: ++batchId, at, items, state: "inTray" as const }, ...s.batches] : s.batches,
        lastMove: items.length ? items : s.lastMove,
        plan: remainingPlan(s.plan, known),
        deleted: [
          ...items.map((i) => ({ id: i.id, name: i.name, path: i.path ?? "", size: i.size, category: "files" as const, what: "file" as const, at, status: "inTray" as const, entry: i.entry })),
          ...s.deleted,
        ].slice(0, 500),
      }));
      for (const i of items) log(t("audit.quarantined"), i.path ?? i.name, i.size);
      if (items.length) {
        get().notify(t("tray.movedToast", { count: items.length, size: fmtBytes(r.bytes) }), { label: t("tray.undo"), run: () => get().undoLastMove() });
        void syncTray();
      }
      reportSkipped(r.skipped);
      return r.done.map((d) => d.path);
    },
    restoreItem: (id) => {
      const item = get().tray.find((i) => i.id === id || i.entry === String(id));
      if (item) void restoreItems([item]);
    },

    freeGhost: () => {
      if (readOnly()) return;
      const { disk } = get();
      const ghost = disk.nodes[disk.ghostId];
      if (!ghost || ghost.size === 0) return;
      set({ confirmDelete: { ids: [ghost.id], kind: "thinSnapshots", handPicked: true }, detailOpen: false });
    },
    explainSpace: () => {
      void api.spaceReport(null, 0).then((spaceReport) => set({ spaceReport })).catch((e) => failed(e));
    },

    planFree: (bytes) => {
      void (async () => {
        const r = await api.planFree(get().disk, bytes).catch(failed);
        if (!r) return;
        set({
          plan: {
            target: bytes,
            ids: r.items.map((c) => c.id),
            parts: Object.fromEntries(r.items.map((c) => [c.id, c.parts])),
            bytes: Object.fromEntries(r.items.map((c) => [c.id, c.bytes])),
            why: Object.fromEntries(r.items.map((c) => [c.id, c.why])),
            total: r.total,
            off: r.items.filter((c) => c.defaultOff).map((c) => c.id),
            shortfall: r.shortfall,
            heldRecent: r.heldRecent,
            heldRunning: r.heldRunning,
            startedAt: Date.now(),
            freed: 0,
            inTrash: 0,
          },
          lens: "all",
        });
      })();
    },
    clearPlan: () => set({ plan: null }),
    togglePlanItem: (id) =>
      set((s) => (s.plan ? { plan: { ...s.plan, off: s.plan.off.includes(id) ? s.plan.off.filter((o) => o !== id) : [...s.plan.off, id] } } : {})),
    setPlanAll: (on) => set((s) => (s.plan ? { plan: { ...s.plan, off: on ? [] : [...s.plan.ids] } } : {})),

    locate: (id) => {
      const { disk } = get();
      const n = disk.nodes[id];
      if (!n || n.parent < 0) return;
      set({ focus: n.parent, lens: lensForFocus(disk, n.parent), selected: id, detailOpen: false });
      setTimeout(() => set({ pulse: { id, key: Date.now() } }), 450);
    },

    // The Scan button and command: a real scan of the volume (or a mock one).
    startScan: () => void get().scanDisk(),
    scanDisk: async () => {
      if (get().scan === "scanning") return;
      clearTimeout(scanTimer);
      // The sonar runs while the scan does, then the tiles settle on the result.
      set((s) => ({ scan: "scanning", scanProgress: null, focus: s.disk.root, lens: "all", plan: null, selected: null, detailOpen: false, viewMode: "map" }));
      const off = api.onScanProgress((p) => set({ scanProgress: p }));
      try {
        const disk = await api.scan();
        // The mock regenerates its tree, so its tray resets; the real tray lives in the core.
        set((s) => ({ disk, focus: disk.root, tray: mockMode() ? [] : s.tray, plan: null, diskVersion: s.diskVersion + 1, scanKey: s.scanKey + 1 }));
        if (disk.summary?.cancelled) get().notify(t("scan.cancelledToast"));
      } catch {
        get().notify(t("scan.failedToast"));
      } finally {
        off();
      }
      scanTimer = window.setTimeout(() => set({ scan: "done", scanProgress: null }), mapMotion.sonar * 1000);
    },
    cancelScan: () => void api.cancelScan(),
    checkFullDiskAccess: async () => set({ fullDiskAccess: await api.fullDiskAccess() }),
    finishOnboarding: () => {
      try {
        localStorage.setItem(ONBOARDED_KEY, "1");
      } catch {
        // Private mode: onboarding shows again next launch, which is harmless.
      }
      set({ overlay: null });
      void get().scanDisk();
    },

    setViewMode: (viewMode) => set({ viewMode }),
    setOverlay: (overlay) => set({ overlay, commandOpen: false }),
    setCommandOpen: (commandOpen) => set({ commandOpen }),
    notify: (text, action) => set({ toast: { key: Date.now(), text, action } }),
    endFlight: (key) => set((s) => ({ flights: s.flights.filter((f) => f.key !== key) })),
    setPrefs: (p) => set(p),

    openUninstall: (appId) => set({ uninstall: appId, detailOpen: false }),
    closeUninstall: () => set({ uninstall: null }),
    quitApp: (appId) => {
      if (readOnly() || get().quitting !== null) return;
      const name = get().disk.nodes[appId].name;
      log(t("audit.quit"), name, 0);
      set({ quitting: appId });
      void api
        .quitApp(get().disk, appId)
        .then(({ disk, quit }) => {
          set((s) => ({ disk, diskVersion: s.diskVersion + 1, quitting: null }));
          if (!quit) get().notify(t("uninstall.stillOpen", { name }));
        })
        .catch((e) => {
          set({ quitting: null });
          failed(e);
        });
    },
    confirmUninstall: (appId, leftoverIds) => {
      if (readOnly()) return set({ uninstall: null });
      set({ uninstall: null });
      void uninstallApp(appId, leftoverIds);
    },
  };
});

// Hot updates to this file keep the current session (disk, focus, tray) instead of resetting
// to an empty store: the new store takes the previous one's data, with the new actions.
if (import.meta.hot) {
  const page = window as { __wrasseStore?: typeof useApp };
  const prev = page.__wrasseStore;
  if (prev && prev !== useApp) {
    const data = Object.fromEntries(Object.entries(prev.getState()).filter(([, v]) => typeof v !== "function"));
    useApp.setState(data as Partial<State>);
  }
  page.__wrasseStore = useApp;
}

/** Loads app info, shows the last scan, then onboards (first run) or scans: exactly once per page
 *  load, not again on StrictMode's double mount or when a hot update re-runs App's effects. */
export function boot() {
  const page = window as { __wrasseBooted?: boolean };
  if (page.__wrasseBooted) return;
  page.__wrasseBooted = true;
  const { setInfo, scanDisk, checkFullDiskAccess } = useApp.getState();
  void checkFullDiskAccess();
  void (async () => {
    const info = await api.appInfo();
    setInfo(info);
    // The tray survives restarts (it lives in the core); dry-run starts on in dev builds.
    const cleanup = await api.cleanupStatus().catch(() => null);
    if (cleanup) useApp.setState({ dryRun: cleanup.dryRun, tray: cleanup.tray, keepDays: cleanup.keepDays });
    const cached = await api.cachedScan().catch(() => null);
    if (cached) useApp.setState((s) => ({ disk: cached, focus: cached.root, scan: "done", diskVersion: s.diskVersion + 1 }));
    let onboarded = false;
    try {
      onboarded = localStorage.getItem(ONBOARDED_KEY) === "1";
    } catch {
      onboarded = false;
    }
    // The launch scan starts once the first paint has settled. WebKit keeps spare buffers for every
    // repaint in the first seconds, and a scan (sweep, then the new map) on top of them was what
    // pushed launch memory over its budget. The saved map is on screen meanwhile; Scan is instant.
    if (onboarded || info.autoScan) window.setTimeout(() => void scanDisk(), LAUNCH_SCAN_DELAY_MS);
    else useApp.setState({ overlay: "onboarding" });
    watchRunningApps();
  })();
}

/** How often the running state of apps is re-read while the window is on screen. */
const RUNNING_POLL_MS = 5000;

/** Keeps "running" on apps true to life: re-reads it when the window comes back into view and
 *  every few seconds while it's visible, so opening or quitting an app elsewhere shows here. */
function watchRunningApps() {
  let busy = false;
  const refresh = () => {
    if (busy || document.visibilityState !== "visible") return;
    busy = true;
    const disk = useApp.getState().disk;
    void api
      .refreshRunning(disk)
      .then((changed) => {
        if (changed && useApp.getState().disk === disk) useApp.setState((s) => ({ diskVersion: s.diskVersion + 1 }));
      })
      .catch(() => undefined)
      .finally(() => {
        busy = false;
      });
  };
  window.addEventListener("focus", refresh);
  document.addEventListener("visibilitychange", refresh);
  window.setInterval(refresh, RUNNING_POLL_MS);
  refresh();
}

/** Drops tray-bound candidates from the plan; the plan ends once nothing is left. */
/** Drops candidates that are gone (all their parts removed). The plan stays, so the meter keeps
 *  showing what was freed and what's pending in the tray. */
function remainingPlan(plan: Plan | null, removed: number[]): Plan | null {
  if (!plan) return null;
  const gone = new Set(removed);
  const ids = plan.ids.filter((id) => !gone.has(id) && !(plan.parts[id] ?? [id]).every((p) => gone.has(p)));
  return { ...plan, ids, off: plan.off.filter((id) => ids.includes(id)) };
}

/** Adds to the plan's "freed now" (or "in the Trash") total. */
function countFreed(plan: Plan | null, freed: number, inTrash = 0): Plan | null {
  return plan ? { ...plan, freed: plan.freed + freed, inTrash: plan.inTrash + inTrash } : null;
}

export function usedBytes(disk: Disk): number {
  return disk.nodes[disk.root].size;
}

export function trayBytes(tray: TrayItem[]): number {
  return tray.reduce((s, i) => s + i.size, 0);
}

/** What a deleted item was, in the words History uses. */
function whatItWas(n: DiskNode): DeletedEntry["what"] {
  if (n.kind === "app" || n.appBundle) return "app";
  if (n.leftover) return "appData";
  if (n.kind === "model" || n.category === "aiModels") return "model";
  if (n.kind === "ghost") return "snapshots";
  if (n.category === "developer") return "dev";
  if (n.rebuildable || n.category === "junk") return "cache";
  return n.children.length > 0 || n.kind === "dir" ? "folder" : "file";
}
