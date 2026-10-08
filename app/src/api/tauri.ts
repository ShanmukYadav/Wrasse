// The real backend: Tauri commands over the Rust core (app/src-tauri/src/scanning.rs).
// Phase 1 is read-only, so cleanup actions reject; the UI checks appInfo().readOnly first.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { WrasseApi } from ".";
import type { AppInfo } from "../lib/backend";
import { insights } from "./insights";
import type { Disk, ScanProgress } from "./types";

const readOnly = () => Promise.reject(new Error("Cleanup isn't available yet: this version only scans."));

export const tauriApi: WrasseApi = {
  appInfo: () => invoke<AppInfo>("app_info"),
  scan: () => invoke<Disk>("scan"),
  cachedScan: () => invoke<Disk | null>("cached_scan"),
  cancelScan: () => invoke("cancel_scan"),
  pauseScan: (paused) => invoke("pause_scan", { paused }),
  fullDiskAccess: () => invoke<boolean>("full_disk_access"),
  openFullDiskAccessSettings: () => invoke("open_full_disk_access_settings"),
  onScanProgress(listener) {
    let off: (() => void) | null = null;
    let stopped = false;
    void listen<ScanProgress>("scan-progress", (e) => listener(e.payload)).then((unlisten) => {
      if (stopped) unlisten();
      else off = unlisten;
    });
    return () => {
      stopped = true;
      off?.();
    };
  },
  insights,
  quarantine: readOnly,
  restore: readOnly,
  emptyTray: readOnly,
  deleteNow: readOnly,
  thinSnapshots: readOnly,
  quitApp: readOnly,
};
