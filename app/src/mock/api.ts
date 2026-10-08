import type { WrasseApi } from "../api";
import { insights } from "../api/insights";
import type { ScanProgress } from "../api/types";
import { generateDisk } from "./disk";
import { detach, reattach, zeroOut } from "./tree";

const progressListeners = new Set<(p: ScanProgress) => void>();
const emit = (p: ScanProgress) => progressListeners.forEach((l) => l(p));

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
  // The mock edits the tree in place and hands it back; nothing on disk is touched.
  async quarantine(disk, ids) {
    detach(disk, ids);
    return disk;
  },
  async restore(disk, ids) {
    reattach(disk, ids);
    return disk;
  },
  async emptyTray() {},
  async deleteNow(disk, ids) {
    detach(disk, ids);
    return disk;
  },
  async thinSnapshots(disk) {
    zeroOut(disk, disk.ghostId);
    return disk;
  },
  async quitApp(disk, appId) {
    disk.nodes[appId].running = false;
    return disk;
  },
};
