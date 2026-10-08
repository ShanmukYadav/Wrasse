import { invoke } from "@tauri-apps/api/core";
import type { UnitSystem } from "./format";

export type AppInfo = {
  version: string;
  platform: string;
  fileManager: string;
  units: UnitSystem;
  dev: boolean;
  bench: boolean;
  /** True while cleanup actions are not implemented (Phase 1): the UI only shows. */
  readOnly: boolean;
  /** Dev test runs: skip onboarding and scan at launch (`WRASSE_AUTOSCAN=1`). */
  autoScan?: boolean;
};

/** What the window can render natively behind transparent regions of the webview. */
export type GlassSupport = { kind: "liquid" | "vibrancy" | "none" };

export type GlassRegion = { id: string; x: number; y: number; w: number; h: number; radius: number; anchor: "top" | "bottom" };

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// Outside Tauri (plain Vite, Playwright) the backend is mocked.
export async function getAppInfo(): Promise<AppInfo> {
  if (!inTauri) {
    return { version: "0.0.0", platform: "macOS", fileManager: "Finder", units: "decimal", dev: true, bench: false, readOnly: false };
  }
  return invoke<AppInfo>("app_info");
}

export async function getGlassSupport(): Promise<GlassSupport> {
  if (!inTauri) return { kind: "none" };
  return invoke<GlassSupport>("glass_support");
}

export async function setGlassRegions(regions: GlassRegion[]): Promise<void> {
  if (!inTauri) return;
  await invoke("glass_regions", { regions });
}

/** Dev only: prints a line in the terminal running `tauri dev` (used by the fps benchmark). */
export async function devLog(message: string): Promise<void> {
  if (!inTauri) {
    console.info(message);
    return;
  }
  await invoke("dev_log", { message });
}
