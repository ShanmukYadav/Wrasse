import type { WrasseApi } from "../api";
import { GB, type Disk, type DiskNode, type Insight, type ScanProgress } from "../api/types";
import { formatBytes } from "../lib/format";
import { t } from "../i18n";
import { generateDisk } from "./disk";
import { detach, reattach, zeroOut } from "./tree";

const progressListeners = new Set<(p: ScanProgress) => void>();
const emit = (p: ScanProgress) => progressListeners.forEach((l) => l(p));

const isWindows = typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");
// Chips round to whole units at 10 and above ("31 GB rebuildable", "8.4 GB old videos").
const fmt = (bytes: number) => {
  const f = formatBytes(bytes, isWindows ? "binary" : "decimal");
  const n = Number(f.value);
  return `${n >= 10 ? Math.round(n) : f.value} ${f.unit}`;
};

function walk(disk: Disk, id: number, visit: (n: DiskNode) => void) {
  const stack = [id];
  while (stack.length) {
    const n = disk.nodes[stack.pop()!];
    visit(n);
    stack.push(...n.children);
  }
}

/** Mock insight chips; Stage B computes these in the Rust core. */
function insights(disk: Disk, id: number): Insight[] {
  const n = disk.nodes[id];
  const out: Insight[] = [];
  let dupCount = 0;
  let dupBytes = 0;
  let oldVideoBytes = 0;
  let unusedApps = 0;
  let rebuildable = 0;
  walk(disk, id, (m) => {
    if (m.rebuildable) rebuildable += m.size;
    if (m.duplicateOf !== undefined) {
      dupCount++;
      dupBytes += m.size;
    }
    if (m.children.length === 0 && /\.(mov|mp4)$/i.test(m.name) && m.lastOpenedDays > 365) oldVideoBytes += m.size;
    if (m.kind === "app" && m.lastOpenedDays > 180) unusedApps++;
  });
  if (n.kind === "ghost") return out;
  if (n.duplicateOf !== undefined) return [{ tone: "amber", text: t("detail.duplicateLabel") }];
  // Apps: running state and long-unused, on the app tile and on its bundle.
  const app = n.kind === "app" ? n : n.appBundle ? disk.nodes[n.parent] : null;
  if (app) {
    const chips: Insight[] = [];
    if (app.running) chips.push({ tone: "protected", text: t("insight.running") });
    if (app.lastOpenedDays > 180) chips.push({ tone: "amber", text: t("insight.unusedApp") });
    return chips;
  }
  if (n.leftover) return n.safety === "safe" ? [{ tone: "green", text: t("insight.safeToClear") }] : [];
  if (n.category === "system" || (n.safety === "keep" && n.depth > 1)) return [{ tone: "protected", text: t("insight.protected") }];
  if (n.category === "junk") return [{ tone: "green", text: t("insight.allSafe") }];
  if (n.category === "aiModels" && dupCount > 0) out.push({ tone: "violet", text: t("insight.modelDuplicates", { count: dupCount, size: fmt(dupBytes) }) });
  if (oldVideoBytes >= 1 * GB) out.push({ tone: "amber", text: t("insight.oldVideos", { size: fmt(oldVideoBytes) }) });
  if (n.category !== "aiModels" && dupCount > 0) out.push({ tone: "amber", text: t("insight.duplicates", { count: dupCount }) });
  if (unusedApps > 0) out.push({ tone: "amber", text: t("insight.unusedApps", { count: unusedApps }) });
  if (n.category === "developer" && rebuildable > 0) out.push({ tone: "green", text: t("insight.rebuildable", { size: fmt(rebuildable) }) });
  return out.slice(0, 2);
}

export const mockApi: WrasseApi = {
  async appInfo() {
    return {
      version: "0.1.0",
      platform: isWindows ? "Windows" : "macOS",
      fileManager: isWindows ? "File Explorer" : "Finder",
      units: isWindows ? "binary" : "decimal",
      dev: true,
      bench: false,
    };
  },
  async scan() {
    emit({ phase: "walking", items: 0, bytes: 0, current: "/" });
    const disk = generateDisk();
    emit({ phase: "done", items: disk.nodes.length, bytes: disk.nodes[disk.root].size, current: "" });
    return disk;
  },
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
