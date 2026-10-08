import { invoke } from "@tauri-apps/api/core";

export type AppInfo = {
  version: string;
  platform: string;
  fileManager: string;
  windowEffects: boolean;
  dev: boolean;
};

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// Outside Tauri (plain Vite, Playwright) the backend is mocked.
export async function getAppInfo(): Promise<AppInfo> {
  if (!inTauri) {
    return { version: "0.0.0", platform: "macOS", fileManager: "Finder", windowEffects: false, dev: true };
  }
  return invoke<AppInfo>("app_info");
}
