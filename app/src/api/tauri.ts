// The real backend: Tauri commands over the Rust core (app/src-tauri/src/scanning.rs and
// cleanup.rs). Every removal goes through the core's safety layer.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { WrasseApi } from ".";
import type { AppInfo } from "./env";
import { insights } from "./insights";
import { detach, detachedRoots, reattach } from "./tree";
import type { Disk, DiskNode, JunkMatch, JunkReport, RemoveOptions, ScanProgress, SkipReason, TrayItem, UninstallOutcome, UninstallRoute, FreePlan, BigFile, DuplicateGroup, PathPreview, PathRemoval, ScannedFile, LoadedModel, ModelMeta } from "./types";

type ItemRequest = { path: string; safety: "safe" | "yourCall"; ruleId?: string; ruleRoot?: string; handPicked: boolean; scanned?: { bytes: number; mtime: number } };
type CoreReport = { done: { path: string; bytes: number; entry: string | null }[]; skipped: { path: string; reason: SkipReason }[]; bytes: number; dryRun: boolean };
type CorePreview = { bytes: number; undoable: boolean; synced: string[]; skipped: { path: string; reason: SkipReason }[] };
type CoreUninstall = {
  route: UninstallRoute;
  bundleRemoved: boolean;
  bundleBytes: number;
  bundleEntry: string | null;
  bundleSkipped: SkipReason | null;
  leftovers: CoreReport;
  dryRun: boolean;
};
type CoreTrayEntry = { id: string; batch: string; name: string; path: string; bytes: number; at: number; ruleId: string | null };

/** A model removal. Paths are the ones the scan showed, so the report maps back to the node. */
function modelJob(n: DiskNode, opts: RemoveOptions) {
  const m: ModelMeta = n.model!;
  return {
    path: n.path,
    tool: m.tool,
    removal: m.removal,
    ...(m.ollamaName ? { ollamaName: m.ollamaName } : {}),
    ...(m.modelsDir ? { modelsDir: m.modelsDir } : {}),
    ...(m.weight ? { weight: m.weight } : {}),
    ...(m.partnerWeight ? { partnerWeight: m.partnerWeight } : {}),
    ...(m.dupeLevel ? { dupeLevel: m.dupeLevel } : {}),
    bytes: n.size,
    safety: n.safety === "safe" ? "safe" : "yourCall",
    handPicked: opts.handPicked,
    ...(m.cloneCopy ? { cloneCopy: true } : {}),
  };
}

function request(n: DiskNode, opts: RemoveOptions): ItemRequest {
  return {
    path: n.path,
    // "Keep" items only reach here after the caution sheet; they go to the tray as "your call".
    safety: n.safety === "safe" ? "safe" : "yourCall",
    ruleId: n.ruleId,
    ruleRoot: n.ruleRoot,
    handPicked: opts.handPicked,
  };
}

function pathRemoval(r: CoreReport): PathRemoval {
  return { done: r.done.map((d) => ({ path: d.path, bytes: d.bytes, entry: d.entry ?? undefined })), skipped: r.skipped, bytes: r.bytes, dryRun: r.dryRun };
}

function pathPreview(p: CorePreview): PathPreview {
  return { bytes: p.bytes, undoable: p.undoable, synced: p.synced, skipped: p.skipped };
}

/** Large & Old: each file carries the size and mtime the scan recorded. */
function scannedItems(files: ScannedFile[]): ItemRequest[] {
  return files.map((f) => ({ path: f.path, safety: "yourCall", handPicked: true, scanned: { bytes: f.bytes, mtime: f.mtime } }));
}

/** Maps the core's per-path results back to node ids. */
function byPath(disk: Disk, ids: number[]) {
  const m = new Map<string, number>();
  for (const id of ids) m.set(disk.nodes[id].path, id);
  return (path: string) => m.get(path) ?? -1;
}

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

  async preview(disk, ids, opts) {
    const idOf = byPath(disk, ids);
    const modelIds = ids.filter((id) => disk.nodes[id].model);
    const pathIds = ids.filter((id) => !disk.nodes[id].model);
    let bytes = 0;
    let undoable = true;
    const synced: number[] = [];
    const skipped: { id: number; reason: SkipReason }[] = [];
    if (pathIds.length) {
      const p = await invoke<CorePreview>("preview_removal", { items: pathIds.map((id) => request(disk.nodes[id], opts)), deleteNow: !!opts.deleteNow });
      bytes += p.bytes;
      undoable = undoable && p.undoable;
      synced.push(...p.synced.map(idOf));
      skipped.push(...p.skipped.map((s) => ({ id: idOf(s.path), reason: s.reason })));
    }
    if (modelIds.length) {
      const p = await invoke<CorePreview>("preview_models", { jobs: modelIds.map((id) => modelJob(disk.nodes[id], opts)) });
      bytes += p.bytes;
      undoable = undoable && p.undoable;
      skipped.push(...p.skipped.map((s) => ({ id: idOf(s.path), reason: s.reason })));
    }
    return { bytes, undoable, synced, skipped };
  },
  async remove(disk, ids, opts) {
    const idOf = byPath(disk, ids);
    const modelIds = ids.filter((id) => disk.nodes[id].model);
    const pathIds = ids.filter((id) => !disk.nodes[id].model);
    const reports: CoreReport[] = [];
    if (pathIds.length) reports.push(await invoke<CoreReport>("remove_items", { items: pathIds.map((id) => request(disk.nodes[id], opts)), deleteNow: !!opts.deleteNow }));
    if (modelIds.length) reports.push(await invoke<CoreReport>("remove_models", { jobs: modelIds.map((id) => modelJob(disk.nodes[id], opts)) }));
    const r: CoreReport = {
      done: reports.flatMap((x) => x.done),
      skipped: reports.flatMap((x) => x.skipped),
      bytes: reports.reduce((s, x) => s + x.bytes, 0),
      dryRun: reports.some((x) => x.dryRun),
    };
    const done = r.done.map((d) => ({ id: idOf(d.path), entry: d.entry ?? undefined })).filter((d) => d.id >= 0);
    if (!r.dryRun) detach(disk, done.map((d) => d.id));
    return { disk, done, skipped: r.skipped.map((s) => ({ id: idOf(s.path), reason: s.reason })), dryRun: r.dryRun, bytes: r.bytes };
  },
  async removeJunk(items: JunkMatch[]) {
    const reqs: ItemRequest[] = items.map((m) => ({ path: m.path, safety: m.safety, ruleId: m.ruleId, ruleRoot: m.root, handPicked: false }));
    const r = await invoke<CoreReport>("remove_items", { items: reqs, deleteNow: false });
    return { done: r.done.length, bytes: r.bytes, skipped: r.skipped.length, dryRun: r.dryRun };
  },
  async restore(disk, items: TrayItem[]) {
    const ids = items.map((i) => i.entry).filter((e): e is string => !!e);
    await invoke("restore_items", { ids });
    // Items still in this session's tree come back in place; others need a (quick) rescan.
    const known = items.filter((i) => i.id >= 0).map((i) => i.id);
    reattach(disk, known);
    return { disk, rescan: known.length < items.length };
  },
  async emptyTray(items) {
    const ids = items.map((i) => i.entry).filter((e): e is string => !!e);
    const status = await invoke<{ dryRun: boolean }>("cleanup_status");
    const bytes = await invoke<number>("purge_items", { ids });
    return { bytes, dryRun: status.dryRun };
  },
  async emptyTrash(disk, trashId) {
    const status = await invoke<{ dryRun: boolean }>("cleanup_status");
    const bytes = await invoke<number>("empty_trash");
    if (!status.dryRun) detach(disk, [trashId]);
    return { disk, done: [{ id: trashId }], skipped: [], dryRun: status.dryRun, bytes };
  },
  async cleanupStatus() {
    const s = await invoke<{ dryRun: boolean; dev: boolean; keepDays: number; tray: CoreTrayEntry[] }>("cleanup_status");
    return {
      dryRun: s.dryRun,
      dev: s.dev,
      keepDays: s.keepDays,
      tray: s.tray.map((e) => ({ id: -1, name: e.name, size: e.bytes, entry: e.id, path: e.path, batch: e.batch, at: e.at * 1000 })),
    };
  },
  setDryRun: (on) => invoke("set_dry_run", { on }),
  junk: () => invoke<JunkReport>("junk_report"),
  neverTouch: (path, on) => invoke<string[]>("never_touch", { path, on }),

  async thinSnapshots(disk) {
    const status = await invoke<{ dryRun: boolean }>("cleanup_status");
    const before = await invoke<import("./types").SpaceReport>("space_report", { before: null, freedNow: 0 });
    const bytes = await invoke<number>("run_delegate", { action: "thinSnapshots" });
    const report = await invoke<import("./types").SpaceReport>("space_report", { before: before.available, freedNow: status.dryRun ? 0 : bytes });
    const ghost = disk.nodes[disk.ghostId];
    if (!status.dryRun && ghost) {
      ghost.size = report.snapshotBytes;
      if (report.snapshotBytes === 0) {
        const parent = disk.nodes[ghost.parent];
        if (parent) parent.children = parent.children.filter((id) => id !== ghost.id);
        disk.ghostId = -1;
      }
    }
    return disk;
  },
  spaceReport: (before, freedNow) => invoke("space_report", { before, freedNow }),
  async delegateAction(action) {
    const status = await invoke<{ dryRun: boolean }>("cleanup_status");
    const bytes = await invoke<number>("run_delegate", { action });
    return { bytes, dryRun: status.dryRun };
  },
  async quitApp(disk, appId) {
    const app = disk.nodes[appId];
    const r = await invoke<{ quit: boolean }>("quit_app", { bundle: app.path });
    if (r.quit) app.running = false;
    return { disk, quit: r.quit };
  },
  async refreshRunning(disk) {
    const apps = disk.nodes.filter((n) => n && n.kind === "app" && n.app);
    if (apps.length === 0) return false;
    const running = new Set(await invoke<string[]>("running_apps", { bundles: apps.map((n) => n.path) }));
    let changed = false;
    for (const n of apps) {
      const now = running.has(n.path);
      if (!!n.running !== now) {
        n.running = now;
        changed = true;
      }
    }
    return changed;
  },
  largeFiles: (minBytes) => invoke<BigFile[]>("large_files", { minBytes }),
  async findDuplicates(onProgress) {
    const off = await listen<{ hashed: number; toHash: number }>("dupes-progress", (e) => onProgress(e.payload.hashed, e.payload.toHash));
    try {
      return await invoke<DuplicateGroup[]>("find_duplicates");
    } finally {
      off();
    }
  },
  cancelDuplicates: () => invoke("cancel_duplicates"),
  async removeDuplicates(keep, remove) {
    return pathRemoval(await invoke<CoreReport>("remove_duplicates", { keep, remove }));
  },
  async previewPaths(files) {
    return pathPreview(await invoke<CorePreview>("preview_removal", { items: scannedItems(files), deleteNow: false }));
  },
  async removePaths(files) {
    return pathRemoval(await invoke<CoreReport>("remove_items", { items: scannedItems(files), deleteNow: false }));
  },
  planFree: (disk, target) => invoke<FreePlan>("plan_free", { target, gone: detachedRoots(disk) }),
  async uninstall(disk, appId, leftoverIds): Promise<UninstallOutcome> {
    const app = disk.nodes[appId];
    const bundle = app.children.map((c) => disk.nodes[c]).find((c) => c.appBundle);
    const idOf = byPath(disk, leftoverIds);
    const r = await invoke<CoreUninstall>("uninstall_app", { bundle: app.path, leftovers: leftoverIds.map((id) => disk.nodes[id].path) });
    const leftovers = r.leftovers.done.map((d) => ({ id: idOf(d.path), entry: d.entry ?? undefined })).filter((d) => d.id >= 0);
    if (!r.dryRun) {
      detach(disk, leftovers.map((d) => d.id));
      if (r.bundleRemoved && bundle) detach(disk, [bundle.id]);
    }
    return {
      disk,
      route: r.route,
      bundleRemoved: r.bundleRemoved,
      bundleBytes: r.bundleBytes,
      bundleEntry: r.bundleEntry ?? undefined,
      bundleSkipped: r.bundleSkipped ?? undefined,
      leftovers,
      leftoverBytes: r.leftovers.bytes,
      skipped: r.leftovers.skipped.map((s) => ({ id: idOf(s.path), reason: s.reason })),
      dryRun: r.dryRun,
    };
  },
  loadedModels: () => invoke<LoadedModel[]>("loaded_models"),
  async unloadModel(name) {
    return { dryRun: await invoke<boolean>("unload_model", { name }) };
  },
};
