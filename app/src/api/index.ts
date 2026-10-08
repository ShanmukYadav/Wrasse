// The one interface the UI uses for data and actions. Stage A binds it to mock data; Stage B
// swaps in Tauri commands backed by the Rust core, without touching components.
import type { AppInfo } from "../lib/backend";
import { mockApi } from "../mock/api";
import type { Disk, Insight } from "./types";

export interface WrasseApi {
  appInfo(): Promise<AppInfo>;
  /** Scans the system volume and returns the full tree. */
  scan(): Promise<Disk>;
  /** Insight chips for a tile (attention, safe, AI, protected). */
  insights(disk: Disk, id: number): Insight[];
}

export const api: WrasseApi = mockApi;

export * from "./types";
