// Mock tree mutations for the prototype: detach (quarantine/delete) and reattach (restore).
import type { Disk } from "./data";

function updateAncestors(disk: Disk, id: number, delta: number) {
  for (let p = disk.nodes[id].parent; p >= 0; p = disk.nodes[p].parent) {
    const n = disk.nodes[p];
    n.size += delta;
    n.deep = n.children.reduce((bits, c) => bits | disk.nodes[c].deep, n.lenses);
  }
}

export function attached(disk: Disk, id: number): boolean {
  const n = disk.nodes[id];
  return n.parent < 0 || disk.nodes[n.parent].children.includes(id);
}

/** Removes nodes from their parents; nodes keep their parent pointer so they can be restored. */
export function detach(disk: Disk, ids: number[]) {
  for (const id of ids) {
    const n = disk.nodes[id];
    if (n.parent < 0 || !attached(disk, id)) continue;
    const parent = disk.nodes[n.parent];
    parent.children = parent.children.filter((c) => c !== id);
    updateAncestors(disk, id, -n.size);
  }
}

export function reattach(disk: Disk, ids: number[]) {
  for (const id of ids) {
    const n = disk.nodes[id];
    if (n.parent < 0 || attached(disk, id)) continue;
    const parent = disk.nodes[n.parent];
    parent.children.push(id);
    parent.children.sort((a, b) => disk.nodes[b].size - disk.nodes[a].size);
    updateAncestors(disk, id, n.size);
  }
}

/** Empties a node in place (used for the snapshot ghost after "Free now"). */
export function zeroOut(disk: Disk, id: number) {
  const n = disk.nodes[id];
  updateAncestors(disk, id, -n.size);
  n.size = 0;
}

/** Leaves under a node, for the list view. */
export function leavesUnder(disk: Disk, id: number): number[] {
  const out: number[] = [];
  const stack = [id];
  while (stack.length) {
    const n = disk.nodes[stack.pop()!];
    if (n.children.length === 0) {
      if (n.id !== id && n.size > 0) out.push(n.id);
    } else stack.push(...n.children);
  }
  return out;
}
