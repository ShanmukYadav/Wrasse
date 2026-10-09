import type { WrasseApi } from "../api";
import { insights } from "../api/insights";
import type { Disk, RemoveOptions, ScanProgress, SkipReason } from "../api/types";
import { deletable, handPickOnly } from "../api/caution";
import { generateDisk } from "./disk";
import { detach, reattach, zeroOut } from "../api/tree";

const progressListeners = new Set<(p: ScanProgress) => void>();
const emit = (p: ScanProgress) => progressListeners.forEach((l) => l(p));

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
      version: "0.1.0",
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
    return { bytes: ok.reduce((s, id) => s + disk.nodes[id].size, 0), undoable: !opts.deleteNow, synced: [], skipped };
  },
  async remove(disk, ids, opts) {
    const { ok, skipped } = mockCheck(disk, ids, opts);
    detach(disk, ok);
    const bytes = ok.reduce((s, id) => s + disk.nodes[id].size, 0);
    return { disk, done: ok.map((id) => ({ id, entry: `mock/${id}` })), skipped, dryRun: false, bytes };
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
  async thinSnapshots(disk) {
    zeroOut(disk, disk.ghostId);
    return disk;
  },
  async quitApp(disk, appId) {
    disk.nodes[appId].running = false;
    return { disk, quit: true };
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
