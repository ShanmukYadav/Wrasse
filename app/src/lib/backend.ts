import { invoke } from "@tauri-apps/api/core";
import type { UnitSystem } from "./format";

export type AppInfo = {
  version: string;
  platform: string;
  fileManager: string;
  units: UnitSystem;
  dev: boolean;
};

export type VolumeSpace = { total: number; available: number };

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// Outside Tauri (plain Vite, Playwright) the backend is mocked.
export async function getAppInfo(): Promise<AppInfo> {
  if (!inTauri) {
    return { version: "0.0.0", platform: "macOS", fileManager: "Finder", units: "decimal", dev: true };
  }
  return invoke<AppInfo>("app_info");
}

export async function getSystemVolume(): Promise<VolumeSpace | null> {
  if (!inTauri) return { total: 494_384_795_648, available: 396_264_345_600 };
  return invoke<VolumeSpace | null>("system_volume");
}
