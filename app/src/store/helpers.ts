import type { Disk, DiskNode, TrayItem } from "../api";
import type { DeletedEntry, Plan, Rect } from "./types";

/** The tile currently on screen that is, or contains, a node. */
export function visibleTileOf(disk: Disk, focus: number, id: number): number | null {
  for (let n = disk.nodes[id]; n; n = disk.nodes[n.parent]) {
    if (n.parent === focus) return n.id;
    if (n.parent < 0) return null;
  }
  return null;
}

export function tileRect(id: number): Rect | null {
  const el = document.querySelector<HTMLElement>(`[data-tile-id="${id}"]`);
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.top, w: r.width, h: r.height };
}

/** Drops tray-bound candidates from the plan; the plan ends once nothing is left. */
/** Drops candidates that are gone (all their parts removed). The plan stays, so the meter keeps
 *  showing what was freed and what's pending in the tray. */
export function remainingPlan(plan: Plan | null, removed: number[]): Plan | null {
  if (!plan) return null;
  const gone = new Set(removed);
  const ids = plan.ids.filter((id) => !gone.has(id) && !(plan.parts[id] ?? [id]).every((p) => gone.has(p)));
  return { ...plan, ids, off: plan.off.filter((id) => ids.includes(id)) };
}

/** Adds to the plan's "freed now" (or "in the Trash") total. */
export function countFreed(plan: Plan | null, freed: number, inTrash = 0): Plan | null {
  return plan ? { ...plan, freed: plan.freed + freed, inTrash: plan.inTrash + inTrash } : null;
}

export function usedBytes(disk: Disk): number {
  return disk.nodes[disk.root].size;
}

export function trayBytes(tray: TrayItem[]): number {
  return tray.reduce((s, i) => s + i.size, 0);
}

/** What a deleted item was, in the words History uses. */
export function whatItWas(n: DiskNode): DeletedEntry["what"] {
  if (n.kind === "app" || n.appBundle) return "app";
  if (n.leftover) return "appData";
  if (n.kind === "model" || n.category === "aiModels") return "model";
  if (n.kind === "ghost") return "snapshots";
  if (n.category === "developer") return "dev";
  if (n.rebuildable || n.category === "junk") return "cache";
  return n.children.length > 0 || n.kind === "dir" ? "folder" : "file";
}
