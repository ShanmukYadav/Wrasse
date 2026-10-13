import type { AppInfo } from "../api/env";
import type { DiskNode, Disk, FreeWhy, LoadedModel, PathRemoval, ScanProgress, ScannedFile, SpaceReport, TrayItem } from "../api";
import type { CategoryId, DiskFilters, FilterId, Scheme } from "../design/tokens";

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

export type State = {
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

export type Actions = {
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
