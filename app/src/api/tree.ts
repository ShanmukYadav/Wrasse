// The UI's copy of the tree after removals: detach (quarantined or deleted items leave their
// parent) and reattach (restored). Shared by the mock and the real API.
import { attached, LENS_IDS, type Disk } from "./types";

/** Adds (sign 1) or removes (sign -1) a node's bytes and lens totals from all its ancestors. */
function updateAncestors(disk: Disk, id: number, sign: 1 | -1) {
  const node = disk.nodes[id];
  for (let p = node.parent; p >= 0; p = disk.nodes[p].parent) {
    const n = disk.nodes[p];
    n.size += sign * node.size;
    for (const lens of LENS_IDS) {
      const bytes = node.matchBytes[lens];
      if (bytes) n.matchBytes[lens] = Math.max(0, (n.matchBytes[lens] ?? 0) + sign * bytes);
    }
    n.deep = n.children.reduce((bits, c) => bits | disk.nodes[c].deep, n.lenses);
  }
}

/** Removes nodes from their parents; nodes keep their parent pointer so they can be restored. */
export function detach(disk: Disk, ids: number[]) {
  for (const id of ids) {
    const n = disk.nodes[id];
    if (n.parent < 0 || !attached(disk, id)) continue;
    const parent = disk.nodes[n.parent];
    parent.children = parent.children.filter((c) => c !== id);
    updateAncestors(disk, id, -1);
  }
}

export function reattach(disk: Disk, ids: number[]) {
  for (const id of ids) {
    const n = disk.nodes[id];
    if (n.parent < 0 || attached(disk, id)) continue;
    const parent = disk.nodes[n.parent];
    parent.children.push(id);
    parent.children.sort((a, b) => disk.nodes[b].size - disk.nodes[a].size);
    updateAncestors(disk, id, 1);
  }
}

/** Empties a node in place (used for the snapshot ghost after "Free now"). */
export function zeroOut(disk: Disk, id: number) {
  const n = disk.nodes[id];
  updateAncestors(disk, id, -1);
  n.size = 0;
}

