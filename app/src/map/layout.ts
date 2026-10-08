// Map layout: the children of the focus folder as squarified tiles (one level at a time), plus a
// uniform-grid spatial index for hit-testing.
import type { Disk } from "../api/types";
import { map as mapTokens } from "../design/tokens";

export type Rect = { x: number; y: number; w: number; h: number };

export type Item = Rect & {
  id: number;
  /** Always true for tiles; kept so the renderer can share code paths. */
  block: boolean;
  group: boolean;
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

const MIN_TILE = 2; // px: smaller tiles are not drawn

export function layoutFocus(disk: Disk, focus: number, width: number, height: number): Layout {
  const { inset, tileGap } = mapTokens;
  const area: Rect = {
    x: inset.left,
    y: inset.top,
    w: Math.max(0, width - inset.left - inset.right),
    h: Math.max(0, height - inset.top - inset.bottom),
  };
  const kids = disk.nodes[focus].children.filter((c) => disk.nodes[c].size > 0);
  const rects = squarify(kids.map((c) => disk.nodes[c].size), {
    x: area.x - tileGap / 2,
    y: area.y - tileGap / 2,
    w: area.w + tileGap,
    h: area.h + tileGap,
  });
  const blocks: Item[] = [];
  const byId = new Map<number, Item>();
  kids.forEach((id, i) => {
    const r = rects[i];
    const w = r.w - tileGap;
    const h = r.h - tileGap;
    if (w < MIN_TILE || h < MIN_TILE) return;
    const item: Item = { id, x: r.x + tileGap / 2, y: r.y + tileGap / 2, w, h, block: true, group: false, rel: 1 };
    blocks.push(item);
    byId.set(id, item);
  });
  return { focus, width, height, items: blocks, byId, blocks, groups: [], index: new SpatialIndex(blocks, width, height) };
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

/** Uniform grid over the layout; each cell lists the tiles overlapping it. */
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
