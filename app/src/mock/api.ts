import type { WrasseApi } from "../api";
import { insights } from "../api/insights";
import type { Disk, RemoveOptions, ScanProgress, SkipReason, StartupItem } from "../api/types";
import { deletable, handPickOnly } from "../api/caution";
import { generateDisk } from "./disk";
import { planFree } from "./rank";
import { mockDuplicates, mockLarge, mockRemoval } from "./files";
import { detach, reattach, zeroOut } from "../api/tree";

const progressListeners = new Set<(p: ScanProgress) => void>();
const emit = (p: ScanProgress) => progressListeners.forEach((l) => l(p));

let mockKeepDays = 7;
let mockCheckUpdates = false;
const agent = (label: string, scope: StartupItem["scope"], app: string | null, enabled: boolean, signed: boolean | null): StartupItem => ({
  label,
  plist: `${scope === "user" ? "~/Library" : "/Library"}/${scope === "system" ? "LaunchDaemons" : "LaunchAgents"}/${label}.plist`,
  scope,
  program: app ? `/Applications/${app}.app/Contents/MacOS/helper` : "/usr/local/bin/helper",
  app,
  enabled,
  signed,
  canDisable: scope === "user",
});
let mockStartup: StartupItem[] = [
  agent("com.spotify.client.startuphelper", "user", "Spotify", true, true),
  agent("us.zoom.updater.login.check", "user", "zoom.us", true, true),
  agent("com.example.helper", "user", null, false, false),
  agent("com.google.keystone.agent", "allUsers", "Google Updater", true, true),
  agent("com.docker.vmnetd", "system", "Docker", true, true),
];
let mockLoginItem: boolean | null = false;
let mockNeverTouch: string[] = [];

/** The mock's stand-in for the core's checks: blocked items stay; Delete now only for Safe. */
function mockCheck(disk: Disk, ids: number[], opts: RemoveOptions) {
  const ok: number[] = [];
  const skipped: { id: number; reason: SkipReason }[] = [];
  for (const id of ids) {
    const n = disk.nodes[id];
    if (!n || !deletable(disk, id)) skipped.push({ id, reason: "protected" });
    else if (opts.deleteNow && n.safety !== "safe") skipped.push({ id, reason: "notSafeForDeleteNow" });
    else if (handPickOnly(n) && (!opts.handPicked || opts.deleteNow)) skipped.push({ id, reason: "handPickOnly" });
    else if (mockNeverTouch.some((p) => n.path.startsWith(p))) skipped.push({ id, reason: "excluded" });
    else ok.push(id);
  }
  return { ok, skipped };
}

const isWindows = typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");

export const mockApi: WrasseApi = {
  async appInfo() {
    return {
      version: "0.1.0-beta.1",
      platform: isWindows ? "Windows" : "macOS",
      fileManager: isWindows ? "File Explorer" : "Finder",
      units: isWindows ? "binary" : "decimal",
      dev: true,
      bench: false,
      readOnly: false,
    };
  },
  async scan() {
    emit({ phase: "walking", items: 0, bytes: 0, current: "/" });
    const disk = generateDisk();
    emit({ phase: "done", items: disk.nodes.length, bytes: disk.nodes[disk.root].size, current: "" });
    return disk;
  },
  async cachedScan() {
    return null;
  },
  async cancelScan() {},
  async pauseScan() {},
  async fullDiskAccess() {
    return true;
  },
  async openFullDiskAccessSettings() {},
  onScanProgress(listener) {
    progressListeners.add(listener);
    return () => progressListeners.delete(listener);
  },
  insights,
  // The mock edits the tree in place and hands it back; nothing on disk is touched. It applies
  // the same rules the UI shows (blocked items, Delete now only for Safe).
  async preview(disk, ids, opts) {
    const { ok, skipped } = mockCheck(disk, ids, opts);
    const ollama = ok.some((id) => disk.nodes[id].model?.removal === "ollama");
    return { bytes: ok.reduce((s, id) => s + disk.nodes[id].size, 0), undoable: !opts.deleteNow && !ollama, synced: [], skipped };
  },
  async remove(disk, ids, opts) {
    const { ok, skipped } = mockCheck(disk, ids, opts);
    detach(disk, ok);
    const bytes = ok.reduce((s, id) => s + disk.nodes[id].size, 0);
    return {
      disk,
      done: ok.map((id) => ({ id, entry: disk.nodes[id].model?.removal === "ollama" ? undefined : `mock/${id}` })),
      skipped,
      dryRun: false,
      bytes,
    };
  },
  async removeJunk(items) {
    return { done: items.length, bytes: items.reduce((s, i) => s + i.bytes, 0), skipped: 0, dryRun: false };
  },
  async restore(disk, items) {
    reattach(disk, items.map((i) => i.id).filter((id) => id >= 0));
    return { disk, rescan: false };
  },
  async emptyTray(items) {
    return { bytes: items.reduce((s, i) => s + i.size, 0), dryRun: false };
  },
  async emptyTrash(disk, trashId) {
    const bytes = disk.nodes[trashId]?.size ?? 0;
    detach(disk, [trashId]);
    return { disk, done: [{ id: trashId }], skipped: [], dryRun: false, bytes };
  },
  async cleanupStatus() {
    return { dryRun: false, dev: true, tray: [], keepDays: 7 };
  },
  async setDryRun() {},
  async junk() {
    const d = generateDisk();
    const matches = d.nodes
      .filter((n) => n.category === "junk" && n.depth === 2 && n.size > 0)
      .map((n) => ({
        ruleId: n.name === "Trash" ? "trash" : "app-caches",
        ruleName: n.name,
        category: "app-cache",
        path: n.path,
        root: n.path,
        safety: "safe" as const,
        explanation: n.why,
        regenerates: true,
        custom: false,
        bytes: n.size,
        delegate: n.name === "Trash" ? "emptyTrash" : null,
      }));
    return { matches, held: [], errors: [] };
  },
  async neverTouch(path, on) {
    mockNeverTouch = on ? [...new Set([...mockNeverTouch, path])] : mockNeverTouch.filter((p) => p !== path);
    return mockNeverTouch;
  },
  async neverTouchList() {
    return mockNeverTouch;
  },
  async setCheckUpdates(on) {
    mockCheckUpdates = on;
    return on;
  },
  onUpdateAvailable() {
    return () => {};
  },
  async installUpdate() {},
  async settings() {
    return { checkUpdates: mockCheckUpdates, updatesAvailable: true, keepDays: mockKeepDays, loginItem: mockLoginItem, rulesFolder: "~/Library/Application Support/Wrasse/rules/custom", dataFolder: "~/Library/Application Support/Wrasse" };
  },
  async setKeepDays(days) {
    mockKeepDays = Math.min(30, Math.max(1, Math.round(days)));
    return mockKeepDays;
  },
  async setLoginItem(on) {
    mockLoginItem = on;
    return mockLoginItem;
  },
  async setResting() {},
  onMenuScan() {
    return () => {};
  },
  async reveal() {},
  async quickLook() {},
  async openRulesFolder() {},
  async ownDataPreview() {
    return { folder: "~/Library/Application Support/Wrasse", bytes: 9.4e6, trayBytes: 0, trayItems: 0, dryRun: true };
  },
  async exportTable(table, format, date) {
    return `~/Downloads/${table.title} ${date}.${format === "markdown" ? "md" : format}`;
  },
  async startupItems() {
    return mockStartup;
  },
  async disableStartupItem(plist) {
    const item = mockStartup.find((i) => i.plist === plist);
    if (!item?.canDisable) throw new Error("only your own startup items can be turned off");
    mockStartup = mockStartup.filter((i) => i.plist !== plist);
    return { done: [{ path: plist, bytes: 1200, entry: `mock-${Date.now()}` }], skipped: [], bytes: 1200, dryRun: false };
  },
  async removeOwnData() {
    return { folder: "~/Library/Application Support/Wrasse", bytes: 9.4e6, trayBytes: 0, trayItems: 0, dryRun: false };
  },
  async thinSnapshots(disk) {
    zeroOut(disk, disk.ghostId);
    return disk;
  },
  async spaceReport(before, freedNow) {
    const available = 120e9;
    const expected = before === null ? undefined : before + freedNow;
    return {
      available,
      importantAvailable: available + 6.1e9,
      purgeable: 6.1e9,
      snapshots: ["com.apple.TimeMachine.2026-10-11-120000.local"],
      snapshotBytes: 6.1e9,
      trashBytes: 1.2e9,
      trayBytes: 0.4e9,
      expectedAvailable: expected,
      gap: expected === undefined ? 0 : Math.max(0, expected - available),
    };
  },
  async delegateAction() {
    return { bytes: 0, dryRun: true };
  },
  async quitApp(disk, appId) {
    disk.nodes[appId].running = false;
    return { disk, quit: true };
  },
  async refreshRunning() {
    return false;
  },
  async largeFiles(minBytes) {
    return mockLarge(minBytes);
  },
  async findDuplicates(onProgress) {
    for (let i = 1; i <= 4; i++) {
      await new Promise((r) => setTimeout(r, 200));
      onProgress(i * 5e9, 2e10);
    }
    return mockDuplicates();
  },
  async cancelDuplicates() {},
  async removeDuplicates(_keep, remove) {
    return mockRemoval(remove);
  },
  async previewPaths(files) {
    const synced = files.filter((f) => /\/(Documents|Desktop|Library\/Mobile Documents|Library\/CloudStorage)\//.test(f.path)).map((f) => f.path);
    return { bytes: files.reduce((s, f) => s + f.bytes, 0), undoable: true, synced, skipped: [] };
  },
  async removePaths(files) {
    return mockRemoval(files.map((f) => f.path));
  },
  async planFree(disk, target) {
    return planFree(disk, target);
  },
  async loadedModels() {
    return [];
  },
  async unloadModel() {
    return { dryRun: true };
  },
  async uninstall(disk, appId, leftoverIds) {
    const app = disk.nodes[appId];
    const route = app.app?.route ?? { kind: "tray" as const };
    const bundle = app.children.map((c) => disk.nodes[c]).find((c) => c.appBundle);
    const bundleBytes = bundle?.size ?? 0;
    const leftoverBytes = leftoverIds.reduce((s, id) => s + disk.nodes[id].size, 0);
    detach(disk, [...(bundle ? [bundle.id] : []), ...leftoverIds]);
    return {
      disk,
      route,
      bundleRemoved: true,
      bundleBytes,
      bundleEntry: route.kind === "tray" && bundle ? `mock/${bundle.id}` : undefined,
      leftovers: leftoverIds.map((id) => ({ id, entry: `mock/${id}` })),
      leftoverBytes,
      skipped: [],
      dryRun: false,
    };
  },
};
