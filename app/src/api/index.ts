// The one interface the UI uses for data and actions. In the Tauri app it is backed by the Rust
// core (./tauri.ts); in the browser, and in the app when mock mode is on, by mock data. Nothing
// outside src/api and src/mock may import from src/mock.
import { inTauri, type AppInfo } from "../lib/backend";
import { mockApi } from "../mock/api";
import { tauriApi } from "./tauri";
import type { Disk, Insight, ScanProgress } from "./types";

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

  // Every action below takes the UI's current tree and resolves to the updated tree.
  /** Moves items to the quarantine tray (recoverable). */
  quarantine(disk: Disk, ids: number[]): Promise<Disk>;
  /** Puts quarantined items back where they were. */
  restore(disk: Disk, ids: number[]): Promise<Disk>;
  /** Permanently removes the quarantined items (the tray's "Empty now"). */
  emptyTray(ids: number[]): Promise<void>;
  /** Permanently deletes safe items right away, skipping the tray. */
  deleteNow(disk: Disk, ids: number[]): Promise<Disk>;
  /** Thins local Time Machine snapshots, freeing the "held by snapshots" space. */
  thinSnapshots(disk: Disk): Promise<Disk>;
  /** Asks a running app to quit (before uninstalling it). */
  quitApp(disk: Disk, appId: number): Promise<Disk>;
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

export const api: WrasseApi = mockMode() ? mockApi : tauriApi;

export * from "./types";
