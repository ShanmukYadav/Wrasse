import { invoke } from "@tauri-apps/api/core";
import type { UnitSystem } from "../lib/format";

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
  /** Memory checks only (`WRASSE_SCRIPTED_USE=1`): use the app a little after the scan. */
  scriptedUse?: string | null;
};

/** Running inside the Tauri app. Outside it (plain Vite, Playwright) the backend is mocked. */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Dev only: prints a line in the terminal running `tauri dev` (used by the fps benchmark). */
export async function devLog(message: string): Promise<void> {
  if (!inTauri) {
    console.info(message);
    return;
  }
  await invoke("dev_log", { message });
}
