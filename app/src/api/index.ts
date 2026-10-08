// The one interface the UI uses for data and actions. Stage A binds it to mock data; Stage B
// swaps in Tauri commands backed by the Rust core, without touching components. Nothing outside
// src/api and src/mock may import from src/mock.
import type { AppInfo } from "../lib/backend";
import { mockApi } from "../mock/api";
import type { Disk, Insight, ScanProgress } from "./types";

export interface WrasseApi {
  /** Version, platform, file manager name and byte units for this machine. */
  appInfo(): Promise<AppInfo>;

  /** Scans the system volume and returns the full tree. Progress arrives through onScanProgress. */
  scan(): Promise<Disk>;
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

export const api: WrasseApi = mockApi;

export * from "./types";
