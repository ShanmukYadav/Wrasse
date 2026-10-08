// Places tiles on the D1 grid (12 × 8 cells): a squarified treemap over the grid whose edges are
// snapped to whole cells, so tiles are "sized roughly by size" and always align to the grid.

export type Rect = { x: number; y: number; w: number; h: number };
export type Cell = { id: number; col: number; row: number; colSpan: number; rowSpan: number };

/** Lays out `values` (sorted descending) inside `rect`, returning one rect per value in order. */
export function squarify(values: number[], rect: Rect): Rect[] {
  const out: Rect[] = new Array(values.length);
  const total = values.reduce((s, v) => s + v, 0);
  if (total <= 0 || rect.w <= 0 || rect.h <= 0) return values.map(() => ({ x: rect.x, y: rect.y, w: 0, h: 0 }));
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
      const next = worst(areas, start, end + 1, rowSum + areas[end], side);
      if (next > best) break;
      best = next;
      rowSum += areas[end];
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
    max = Math.max(max, areas[i]);
    min = Math.min(min, areas[i]);
  }
  const s2 = side * side;
  const sum2 = sum * sum;
  return Math.max((s2 * max) / sum2, sum2 / (s2 * min));
}

/** Snaps a squarified layout to whole grid cells. Tiles with no full cell are dropped. */
export function gridLayout(items: { id: number; size: number }[], cols: number, rows: number, emphasis = 1, minCells = 2): Cell[] {
  const sorted = [...items].sort((a, b) => b.size - a.size);
  const total = sorted.reduce((s, i) => s + Math.pow(i.size, emphasis), 0);
  // Give small tiles a floor so every tile keeps at least `minCells` cells.
  const floor = (total * minCells) / (cols * rows);
  const weights = sorted.map((i) => Math.max(Math.pow(i.size, emphasis), floor));
  const rects = squarify(weights, { x: 0, y: 0, w: cols, h: rows });
  const cells: Cell[] = [];
  sorted.forEach((item, i) => {
    const r = rects[i];
    const x0 = Math.round(r.x);
    const y0 = Math.round(r.y);
    const x1 = Math.round(r.x + r.w);
    const y1 = Math.round(r.y + r.h);
    if (x1 - x0 < 1 || y1 - y0 < 1) return;
    cells.push({ id: item.id, col: x0 + 1, row: y0 + 1, colSpan: x1 - x0, rowSpan: y1 - y0 });
  });
  return cells;
}
