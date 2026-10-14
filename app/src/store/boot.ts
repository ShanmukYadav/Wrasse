import { api } from "../api";
import { LAUNCH_SCAN_DELAY_MS, ONBOARDED_KEY, useApp } from "./app";

/** Loads app info, shows the last scan, then onboards (first run) or scans: exactly once per page
 *  load, not again on StrictMode's double mount or when a hot update re-runs App's effects. */
export function boot() {
  const page = window as { __wrasseBooted?: boolean };
  if (page.__wrasseBooted) return;
  page.__wrasseBooted = true;
  const { setInfo, scanDisk, checkFullDiskAccess } = useApp.getState();
  void checkFullDiskAccess();
  void (async () => {
    const info = await api.appInfo();
    setInfo(info);
    // The tray survives restarts (it lives in the core); dry-run starts on in dev builds.
    const cleanup = await api.cleanupStatus().catch(() => null);
    if (cleanup) useApp.setState({ dryRun: cleanup.dryRun, tray: cleanup.tray, keepDays: cleanup.keepDays });
    const cached = await api.cachedScan().catch(() => null);
    if (cached) useApp.setState((s) => ({ disk: cached, focus: cached.root, scan: "done", diskVersion: s.diskVersion + 1 }));
    let onboarded = false;
    try {
      onboarded = localStorage.getItem(ONBOARDED_KEY) === "1";
    } catch {
      onboarded = false;
    }
    // The launch scan starts once the first paint has settled. WebKit keeps spare buffers for every
    // repaint in the first seconds, and a scan (sweep, then the new map) on top of them was what
    // pushed launch memory over its budget. The saved map is on screen meanwhile; Scan is instant.
    if (onboarded || info.autoScan) window.setTimeout(() => void scanDisk().then(() => info.scriptedUse && void scriptedUse()), LAUNCH_SCAN_DELAY_MS);
    else useApp.setState({ overlay: "onboarding" });
    watchRunningApps();
    api.onMenuScan(() => {
      if (useApp.getState().scan !== "scanning") useApp.getState().startScan();
    });
  })();
}

/** How often the running state of apps is re-read while the window is on screen. */
const RUNNING_POLL_MS = 5000;

/** Keeps "running" on apps true to life: re-reads it when the window comes back into view and
 *  every few seconds while it's visible, so opening or quitting an app elsewhere shows here. */
function watchRunningApps() {
  let busy = false;
  const refresh = () => {
    if (busy || document.visibilityState !== "visible") return;
    busy = true;
    const disk = useApp.getState().disk;
    void api
      .refreshRunning(disk)
      .then((changed) => {
        if (changed && useApp.getState().disk === disk) useApp.setState((s) => ({ diskVersion: s.diskVersion + 1 }));
      })
      .catch(() => undefined)
      .finally(() => {
        busy = false;
      });
  };
  window.addEventListener("focus", refresh);
  document.addEventListener("visibilitychange", refresh);
  window.setInterval(refresh, RUNNING_POLL_MS);
  refresh();
}

const pause = (ms: number) => new Promise((r) => window.setTimeout(r, ms));

/** "Idle after use" for the memory check (scripts/check-idle-memory.sh --after-use): what a person
 *  does in the first minute, through the same store actions the clicks call. The motion setting
 *  it changes is put back before it ends. */
async function scriptedUse() {
  const s = () => useApp.getState();
  await pause(3000);
  const { disk } = s();
  const big = disk.nodes[disk.root].children
    .map((id) => disk.nodes[id])
    .filter((n) => n.size > 0 && n.children.length > 0)
    .sort((a, b) => b.size - a.size);
  for (const n of big.slice(0, 2)) {
    s().setFocus(n.id);
    await pause(2000);
    s().setFocus(disk.root);
    await pause(1500);
  }
  s().setTrayOpen(true);
  await pause(2000);
  s().setTrayOpen(false);
  s().setOverlay("settings");
  await pause(1500);
  const motion = s().motion;
  s().setPrefs({ motion: motion === "reduce" ? "full" : "reduce" });
  await pause(1500);
  s().setPrefs({ motion });
  s().setOverlay(null);
}
