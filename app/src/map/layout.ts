// Squarified treemap layout (Bruls, Huizing & van Wijk) of a focus node, laid out to full depth
// so any descendant has a rect, plus a uniform-grid spatial index for hit-testing.
import { map as mapTokens } from "../design/tokens";
import type { Disk } from "./data";

export type Rect = { x: number; y: number; w: number; h: number };

export type Item = Rect & {
  id: number;
  /** Drawn as a block: a leaf, or a directory too small to subdivide. */
  block: boolean;
  /** A direct child of the focus drawn as a labeled group frame. */
  group: boolean;
  /** Depth relative to the focus. */
  rel: number;
};

export type Layout = {
  focus: number;
  width: number;
  height: number;
  items: Item[];
  byId: Map<number, Item>;
  blocks: Item[];
  groups: Item[];
  index: SpatialIndex;
};

const MIN_SPLIT = 6; // px: smaller directories are drawn as one block
const MIN_ITEM = 1; // px: smaller children are not drawn
const GROUP_MIN_W = 96;
const GROUP_MIN_H = 64;

export function layoutFocus(disk: Disk, focus: number, width: number, height: number): Layout {
  const items: Item[] = [];
  const blocks: Item[] = [];
  const groups: Item[] = [];
  const byId = new Map<number, Item>();
  const { gap, headerHeight } = mapTokens;

  function place(id: number, x: number, y: number, w: number, h: number, rel: number) {
    const node = disk.nodes[id];
    const item: Item = { id, x, y, w, h, block: false, group: false, rel };
    items.push(item);
    byId.set(id, item);
    const kids = node.children.filter((c) => disk.nodes[c].size > 0);
    if (rel > 0 && (kids.length === 0 || w < MIN_SPLIT || h < MIN_SPLIT)) {
      item.block = true;
      blocks.push(item);
      return;
    }
    if (kids.length === 0) return;
    let inner: Rect = { x, y, w, h };
    if (rel === 1 && w >= GROUP_MIN_W && h >= GROUP_MIN_H) {
      item.group = true;
      groups.push(item);
      inner = { x: x + gap, y: y + headerHeight, w: w - gap * 2, h: h - headerHeight - gap };
    }
    const rects = squarify(kids.map((c) => disk.nodes[c].size), inner);
    for (let i = 0; i < kids.length; i++) {
      const r = rects[i];
      const cw = r.w - gap;
      const ch = r.h - gap;
      if (cw < MIN_ITEM || ch < MIN_ITEM) continue;
      place(kids[i], r.x + gap / 2, r.y + gap / 2, cw, ch, rel + 1);
    }
  }

  place(focus, 0, 0, width, height, 0);
  return { focus, width, height, items, byId, blocks, groups, index: new SpatialIndex(blocks, width, height) };
}

/** Lays out `values` (sorted descending) inside `rect`, returning one rect per value in order. */
export function squarify(values: number[], rect: Rect): Rect[] {
  const out: Rect[] = new Array(values.length);
  const total = values.reduce((s, v) => s + v, 0);
  if (total <= 0 || rect.w <= 0 || rect.h <= 0) {
    for (let i = 0; i < values.length; i++) out[i] = { x: rect.x, y: rect.y, w: 0, h: 0 };
    return out;
  }
  const scale = (rect.w * rect.h) / total;
  const areas = values.map((v) => v * scale);
  let { x, y, w, h } = rect;
  let start = 0;

  while (start < areas.length) {
    const side = Math.min(w, h);
    let end = start + 1;
    let rowSum = areas[start];
    let best = worst(areas, start, end, rowSum, side);
    while (end < areas.length) {
      const nextSum = rowSum + areas[end];
      const next = worst(areas, start, end + 1, nextSum, side);
      if (next > best) break;
      best = next;
      rowSum = nextSum;
      end++;
    }
    // Lay the row along the shorter side.
    if (w >= h) {
      const rowW = rowSum / h;
      let cy = y;
      for (let i = start; i < end; i++) {
        const ih = areas[i] / rowW;
        out[i] = { x, y: cy, w: rowW, h: ih };
        cy += ih;
      }
      x += rowW;
      w -= rowW;
    } else {
      const rowH = rowSum / w;
      let cx = x;
      for (let i = start; i < end; i++) {
        const iw = areas[i] / rowH;
        out[i] = { x: cx, y, w: iw, h: rowH };
        cx += iw;
      }
      y += rowH;
      h -= rowH;
    }
    start = end;
  }
  return out;
}

function worst(areas: number[], start: number, end: number, sum: number, side: number): number {
  let max = 0;
  let min = Infinity;
  for (let i = start; i < end; i++) {
    if (areas[i] > max) max = areas[i];
    if (areas[i] < min) min = areas[i];
  }
  const s2 = side * side;
  const sum2 = sum * sum;
  return Math.max((s2 * max) / sum2, sum2 / (s2 * min));
}

/** Uniform grid over the layout; each cell lists the blocks overlapping it. */
export class SpatialIndex {
  private cells: Item[][];
  private cols: number;
  private rows: number;
  private size = 48;

  constructor(blocks: Item[], width: number, height: number) {
    this.cols = Math.max(1, Math.ceil(width / this.size));
    this.rows = Math.max(1, Math.ceil(height / this.size));
    this.cells = Array.from({ length: this.cols * this.rows }, () => []);
    for (const b of blocks) {
      const c0 = Math.max(0, Math.floor(b.x / this.size));
      const c1 = Math.min(this.cols - 1, Math.floor((b.x + b.w) / this.size));
      const r0 = Math.max(0, Math.floor(b.y / this.size));
      const r1 = Math.min(this.rows - 1, Math.floor((b.y + b.h) / this.size));
      for (let r = r0; r <= r1; r++) for (let c = c0; c <= c1; c++) this.cells[r * this.cols + c].push(b);
    }
  }

  hit(x: number, y: number): Item | null {
    const c = Math.floor(x / this.size);
    const r = Math.floor(y / this.size);
    if (c < 0 || r < 0 || c >= this.cols || r >= this.rows) return null;
    for (const b of this.cells[r * this.cols + c]) {
      if (x >= b.x && x <= b.x + b.w && y >= b.y && y <= b.y + b.h) return b;
    }
    return null;
  }
}

export function lerpRect(a: Rect, b: Rect, t: number): Rect {
  return { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, w: a.w + (b.w - a.w) * t, h: a.h + (b.h - a.h) * t };
}
