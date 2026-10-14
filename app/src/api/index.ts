// The one interface the UI uses for data and actions. In the Tauri app it is backed by the Rust
// core (./tauri.ts); in the browser, and in the app when mock mode is on, by mock data. Nothing
// outside src/api and src/mock may import from src/mock.
import { inTauri, type AppInfo } from "./env";
import { tauriApi } from "./tauri";
import type { OwnData, SettingsView, StartupItem, CleanupStatus, Disk, Insight, JunkMatch, JunkReport, RemoveOptions, RemoveOutcome, RemovePreview, ScanProgress, TrayItem, UninstallOutcome, FreePlan, BigFile, DuplicateGroup, PathPreview, PathRemoval, ScannedFile, LoadedModel, SpaceReport } from "./types";

export interface WrasseApi {
  /** Version, platform, file manager name and byte units for this machine. */
  appInfo(): Promise<AppInfo>;

  /** Scans the system volume and returns the full tree. Progress arrives through onScanProgress. */
  scan(): Promise<Disk>;
  /** The last finished scan, saved on disk, to show at launch while a new scan runs. */
  cachedScan(): Promise<Disk | null>;
  /** Stops the running scan; scan() then resolves with what was read so far. */
  cancelScan(): Promise<void>;
  /** Pauses or resumes the running scan. */
  pauseScan(paused: boolean): Promise<void>;
  /** Whether macOS Full Disk Access is granted (always true where it doesn't exist). */
  fullDiskAccess(): Promise<boolean>;
  /** Opens System Settings at Privacy & Security › Full Disk Access. */
  openFullDiskAccessSettings(): Promise<void>;
  /** Subscribes to scan progress; returns the unsubscribe function. */
  onScanProgress(listener: (p: ScanProgress) => void): () => void;

  /** Insight chips for a tile (attention, safe, AI, protected). */
  insights(disk: Disk, id: number): Insight[];

  // Removal goes through the core's safety layer (Phase 2). Results say what was done and what
  // was skipped and why; in dry-run nothing changes and `dryRun` is true.
  /** What removing these would do (bytes, iCloud-synced items, skips), touching nothing. */
  preview(disk: Disk, ids: number[], opts: RemoveOptions): Promise<RemovePreview>;
  /** Moves items to the tray, or with `deleteNow` deletes Safe items for good. */
  remove(disk: Disk, ids: number[], opts: RemoveOptions): Promise<RemoveOutcome>;
  /** Removes junk-rule items by path (System Junk). The tree is refreshed by a rescan after. */
  removeJunk(items: JunkMatch[]): Promise<{ done: number; bytes: number; skipped: number; dryRun: boolean }>;
  /** Puts tray items back. `rescan` means the tree should be refreshed with a scan. */
  restore(disk: Disk, items: TrayItem[]): Promise<{ disk: Disk; rescan: boolean }>;
  /** Deletes tray items for good (the tray's "Empty now"). Not undoable. */
  emptyTray(items: TrayItem[]): Promise<{ bytes: number; dryRun: boolean }>;
  /** Empties the Trash through the OS. Not undoable. */
  emptyTrash(disk: Disk, trashId: number): Promise<RemoveOutcome>;
  /** Dry-run state and what's in the tray (it survives restarts). */
  cleanupStatus(): Promise<CleanupStatus>;
  setDryRun(on: boolean): Promise<void>;
  /** What the junk rules find right now. */
  junk(): Promise<JunkReport>;
  /** Adds or removes a "Never touch" path; resolves to the whole list. */
  neverTouch(path: string, on: boolean): Promise<string[]>;
  /** The whole "Never touch" list. */
  neverTouchList(): Promise<string[]>;
  /** Settings the core keeps, the login item, the custom rules and data folders. */
  settings(): Promise<SettingsView>;
  /** How many days batches stay in the tray (1–30); resolves to the saved value. */
  setKeepDays(days: number): Promise<number>;
  /** Turns Wrasse's login item on or off; resolves to what macOS now reports. */
  setLoginItem(on: boolean): Promise<boolean | null>;
  /** Shows a file or folder selected in the file manager. Read-only. */
  reveal(path: string): Promise<void>;
  /** Quick Look preview of a file (Space). Read-only. */
  quickLook(path: string): Promise<void>;
  /** Creates the custom rules folder if needed and shows it. */
  openRulesFolder(): Promise<void>;
  /** What "Remove all Wrasse data" would remove. Read-only. */
  ownDataPreview(): Promise<OwnData>;
  /** Removes Wrasse's own data, the tray included. Not undoable. */
  removeOwnData(): Promise<OwnData>;
  /** What starts at login or with the Mac. Read-only. */
  startupItems(): Promise<StartupItem[]>;
  /** Turns off one of your own startup items: unloads it, then its plist goes to the tray. */
  disableStartupItem(plist: string): Promise<PathRemoval>;
  /** Thins local Time Machine snapshots, freeing the "held by snapshots" space. */
  thinSnapshots(disk: Disk): Promise<Disk>;
  /** Free space, purgeable space, snapshots, Trash and the tray. Read-only. */
  spaceReport(before: number | null, freedNow: number): Promise<SpaceReport>;
  /** Runs one delegated cleanup. Dev builds are in dry run, so this reports without doing it. */
  delegateAction(action: string): Promise<{ bytes: number; dryRun: boolean }>;
  /** Asks a running app to quit (before uninstalling it); waits up to 10 s. */
  quitApp(disk: Disk, appId: number): Promise<{ disk: Disk; quit: boolean }>;
  /** Re-reads which apps are running and updates `running` on their nodes in place.
   *  Resolves true when any changed. Read-only. */
  refreshRunning(disk: Disk): Promise<boolean>;
  /** Free Up Space: what to remove to free `target` bytes, best first. `gone` are nodes
   *  already removed this session. Read-only. */
  planFree(disk: Disk, target: number): Promise<FreePlan>;
  /** The biggest files in the home folder of at least `minBytes`, with last use. */
  largeFiles(minBytes: number): Promise<BigFile[]>;
  /** Byte-identical files of 10 MB or more (size, then partial hash, then full hash). */
  findDuplicates(onProgress: (hashed: number, toHash: number) => void): Promise<DuplicateGroup[]>;
  cancelDuplicates(): Promise<void>;
  /** Moves the chosen copies to the tray, keeping `keep`, after checking they're still identical. */
  removeDuplicates(keep: string, remove: string[]): Promise<PathRemoval>;
  /** What moving these paths would do (Large & Old's caution sheet). */
  previewPaths(files: ScannedFile[]): Promise<PathPreview>;
  /** Moves files picked by hand to the tray (by path), re-checking each against the scan. */
  removePaths(files: ScannedFile[]): Promise<PathRemoval>;
  /** Uninstalls an app: the bundle goes by its route (tray, Homebrew, Finder Trash), the chosen
   *  leftovers go to the tray. The core looks the app up again and checks it isn't running. */
  uninstall(disk: Disk, appId: number, leftoverIds: number[]): Promise<UninstallOutcome>;
  /** Models Ollama reports as loaded, plus LM Studio's process memory. Read-only. */
  loadedModels(): Promise<LoadedModel[]>;
  /** Asks Ollama to unload a model. Does not delete it. */
  unloadModel(name: string): Promise<{ dryRun: boolean }>;
}

/** Dev toggle: in the app, `wrasse.mock = "1"` in localStorage switches to mock data for UI work. */
export const MOCK_KEY = "wrasse.mock";
export function mockMode(): boolean {
  if (!inTauri) return true;
  try {
    return localStorage.getItem(MOCK_KEY) === "1";
  } catch {
    return false;
  }
}

// The mock (fake disk, apps and files) is a separate chunk, loaded only in mock mode, so the app
// doesn't parse it at every launch.
export const api: WrasseApi = mockMode() ? (await import("../mock/api")).mockApi : tauriApi;

export * from "./types";
export * from "./caution";
export * from "./tree";
