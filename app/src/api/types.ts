// Data types shared by the UI and the api/ layer. Stage B's Tauri commands return the same shapes.
import type { Category, LensId, Safety } from "../design/tokens";

export const GB = 1e9;

export type NodeKind = "volume" | "dir" | "file" | "app" | "model" | "ghost";

export const LENS_BIT: Record<Exclude<LensId, "all">, number> = {
  junk: 1,
  apps: 2,
  aiModels: 4,
  dev: 8,
  largeOld: 16,
  grew: 32,
};

export type DiskNode = {
  id: number;
  name: string;
  path: string;
  size: number;
  parent: number; // -1 for the root
  children: number[];
  depth: number;
  category: Category;
  kind: NodeKind;
  lastOpenedDays: number;
  safety: Safety;
  why: string;
  lenses: number; // bitmask of LENS_BIT
  deep: number; // lenses of this node and all descendants
  /** Bytes under this node matching each lens ("grew": bytes grown). */
  matchBytes: Partial<Record<Exclude<LensId, "all">, number>>;
  grewBy: number;
  shade: 0 | 1 | 2;
  /** Set when this item is a byte-identical copy of another node. */
  duplicateOf?: number;
  /** App-managed libraries (Photos, Music): read-only, shown for size only. */
  readOnly?: boolean;
  /** Rebuilt automatically by its tool (node_modules, build products). */
  rebuildable?: boolean;
  /** An app (kind "app") whose process is running right now. */
  running?: boolean;
  /** The .app bundle inside an app: the item "Uninstall" targets. */
  appBundle?: boolean;
  /** For an app's leftovers: what kind of leftover it is (Caches, Logs, …). */
  leftover?: "caches" | "support" | "containers" | "logs" | "preferences" | "savedState";
  /** Extra label from the scanner: "In iCloud only", "Couldn't read", "Shares space with clones". */
  note?: string;
};

/** Totals from a real scan (absent in mock data). */
export type DiskSummary = {
  capacity: number;
  available: number;
  /** The sealed macOS volume. */
  osUsed: number;
  /** Bytes found by walking the data volume. */
  scanned: number;
  /** Used space no folder accounts for ("System data"). */
  unaccounted: number;
  home: number;
  files: number;
  dirs: number;
  unreadableDirs: number;
  datalessItems: number;
  hardlinksDeduped: number;
  cloneSharedBytes: number;
  cancelled: boolean;
};

export type Disk = {
  nodes: DiskNode[];
  root: number;
  capacity: number;
  ghostId: number;
  leafCount: number;
  summary?: DiskSummary;
};

export const LENS_IDS = Object.keys(LENS_BIT) as Exclude<LensId, "all">[];

export function lensMatches(node: DiskNode, lens: LensId): boolean {
  return lens === "all" || (node.deep & LENS_BIT[lens]) !== 0;
}

/** False once a node has been removed from its parent (quarantined or deleted); it keeps its parent pointer for restore. */
export function attached(disk: Disk, id: number): boolean {
  const n = disk.nodes[id];
  return n.parent < 0 || disk.nodes[n.parent].children.includes(id);
}

/** Progress while a scan runs. Stage B emits these at most 10 times a second. */
export type ScanProgress = {
  phase: "walking" | "done" | "cancelled";
  /** Files and folders visited so far. */
  items: number;
  /** Bytes on disk counted so far. */
  bytes: number;
  /** The folder being read right now, for the status line. */
  current: string;
};

export function ancestors(disk: Disk, id: number): number[] {
  const out: number[] = [];
  for (let n = disk.nodes[id]; n.parent >= 0; n = disk.nodes[n.parent]) out.unshift(n.parent);
  return out;
}

export function isAncestor(disk: Disk, ancestor: number, id: number): boolean {
  for (let n = disk.nodes[id]; n.parent >= 0; n = disk.nodes[n.parent]) if (n.parent === ancestor) return true;
  return false;
}

/** The lens that matches a category, so the lens bar follows the category you open. */
export function lensForFocus(disk: Disk, focus: number): LensId {
  let n = disk.nodes[focus];
  while (n.parent >= 0 && n.depth > 1) n = disk.nodes[n.parent];
  const byCategory: Partial<Record<DiskNode["category"], LensId>> = { aiModels: "aiModels", apps: "apps", developer: "dev", junk: "junk" };
  return (n.depth === 1 && byCategory[n.category]) || "all";
}

/** The top-level tile a category lens opens (Junk, Apps, AI Models, Dev), or null for filter lenses. */
export function categoryForLens(disk: Disk, lens: LensId): number | null {
  const byLens: Partial<Record<LensId, DiskNode["category"]>> = { aiModels: "aiModels", apps: "apps", dev: "developer", junk: "junk" };
  const cat = byLens[lens];
  if (!cat) return null;
  const n = disk.nodes.find((k) => k.depth === 1 && k.category === cat && k.size > 0);
  return n ? n.id : null;
}

/** Follows a chain of single-child folders (Users → shanmuk) to the first level worth showing. */
export function throughSingles(disk: Disk, id: number): number {
  let n = disk.nodes[id];
  while (n.children.length === 1 && disk.nodes[n.children[0]].children.length > 0) n = disk.nodes[n.children[0]];
  return n.id;
}

/** A disk with nothing scanned yet: the map is empty until the first scan returns. */
export function emptyDisk(): Disk {
  return {
    nodes: [
      {
        id: 0, name: "Macintosh HD", path: "/", size: 0, parent: -1, children: [], depth: 0, category: "other", kind: "volume",
        lastOpenedDays: 0, safety: "keep", why: "", lenses: 0, deep: 0, matchBytes: {}, grewBy: 0, shade: 0, duplicateOf: undefined,
      },
    ],
    root: 0,
    capacity: 0,
    ghostId: -1,
    leafCount: 0,
  };
}

/** An insight chip on a tile: amber = attention, green = safe, violet = AI, protected = slate. */
export type Insight = { tone: "amber" | "green" | "violet" | "protected"; text: string };
