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
};

export type Disk = {
  nodes: DiskNode[];
  root: number;
  capacity: number;
  ghostId: number;
  leafCount: number;
};

export const LENS_IDS = Object.keys(LENS_BIT) as Exclude<LensId, "all">[];

export function lensMatches(node: DiskNode, lens: LensId): boolean {
  return lens === "all" || (node.deep & LENS_BIT[lens]) !== 0;
}

export function ancestors(disk: Disk, id: number): number[] {
  const out: number[] = [];
  for (let n = disk.nodes[id]; n.parent >= 0; n = disk.nodes[n.parent]) out.unshift(n.parent);
  return out;
}

export function isAncestor(disk: Disk, ancestor: number, id: number): boolean {
  for (let n = disk.nodes[id]; n.parent >= 0; n = disk.nodes[n.parent]) if (n.parent === ancestor) return true;
  return false;
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
