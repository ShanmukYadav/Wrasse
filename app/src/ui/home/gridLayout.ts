// Places tiles on the D1 grid (12 × 8 cells), sized roughly by size and always on whole cells.

export type Cell = { id: number; col: number; row: number; colSpan: number; rowSpan: number };

/** Tile weights: size^emphasis, the biggest capped at `maxShare` of the total, and every tile
 *  raised to at least `minCells` cells' worth (less when there are too many tiles to allow it). */
function tileWeights(sizes: number[], cols: number, rows: number, emphasis = 1, minCells = 2, maxShare = 1): number[] {
  const weights = sizes.map((s) => Math.pow(Math.max(0, s), emphasis));
  const n = weights.length;
  // With few tiles the cap must still leave the biggest clearly biggest (2 tiles: up to 75%).
  const share = Math.max(maxShare, 1.5 / Math.max(1, n));
  // With many tiles the minimum shrinks so the floor doesn't flatten every size difference.
  const cells = Math.max(2, Math.min(minCells, Math.floor(((cols * rows) / Math.max(1, n)) * 0.6)));
  for (let pass = 0; pass < 3; pass++) {
    const total = weights.reduce((s, w) => s + w, 0);
    if (total <= 0) break;
    // Cap: a weight above `share` of the total shrinks so it is exactly `share` of the new total.
    for (let i = 0; i < n; i++) {
      const rest = total - weights[i];
      if (n > 1 && weights[i] > share * total) weights[i] = (share / (1 - share)) * rest;
    }
    // Floor: no tile below its minimum share of the grid.
    const capped = weights.reduce((s, w) => s + w, 0);
    const floor = (capped * cells) / (cols * rows);
    for (let i = 0; i < weights.length; i++) weights[i] = Math.max(weights[i], floor);
  }
  return weights;
}

type Box = { col: number; row: number; w: number; h: number };
type Plan = { cost: number; cells: Cell[] };

/** One grid cell is about this much wider than tall on screen (12 × 8 cells in a wide window). */
const CELL_ASPECT = 1.3;

/** Places tiles on whole grid cells by cutting the grid in two, again and again, each cut on a
 *  whole cell. Every way of cutting the size-sorted list (biggest first, top-left) is tried and the
 *  layout whose tiles are closest to their fair area and closest to square wins, with tiles at
 *  least three columns wide where possible so names fit. Memoized, so it's cheap for 12 tiles. */
export function gridLayout(items: { id: number; size: number }[], cols: number, rows: number, emphasis = 1, minCells = 2, maxShare = 1): Cell[] {
  const sorted = [...items].sort((a, b) => b.size - a.size);
  if (!sorted.length) return [];
  const weights = tileWeights(
    sorted.map((i) => i.size),
    cols,
    rows,
    emphasis,
    minCells,
    maxShare,
  );
  const prefix = [0];
  for (const w of weights) prefix.push(prefix[prefix.length - 1] + w);
  const total = prefix[prefix.length - 1];
  const fair = (k: number) => (weights[k] / total) * cols * rows;
  const memo = new Map<string, Plan>();

  const tile = (k: number, box: Box): Plan => {
    const r = (box.w * CELL_ASPECT) / box.h;
    const aspect = Math.max(r, 1 / r);
    const area = box.w * box.h;
    const misfit = Math.abs(area - fair(k)) / fair(k);
    const narrow = box.w < 3 && cols >= 6 ? 1.5 : 0;
    // Big tiles' shapes matter more: they're what the eye reads first.
    const weight = 0.5 + fair(k) / ((cols * rows) / sorted.length);
    return { cost: weight * (2 * (aspect - 1) + 5 * misfit + narrow), cells: [{ id: sorted[k].id, col: box.col, row: box.row, colSpan: box.w, rowSpan: box.h }] };
  };

  const best = (i: number, j: number, box: Box): Plan => {
    if (j - i === 1) return tile(i, box);
    if (box.w * box.h < j - i) return { cost: Infinity, cells: [] };
    const key = `${i},${j},${box.col},${box.row},${box.w},${box.h}`;
    const hit = memo.get(key);
    if (hit) return hit;
    let plan: Plan = { cost: Infinity, cells: [] };
    for (let k = i + 1; k < j; k++) {
      const share = (prefix[k] - prefix[i]) / (prefix[j] - prefix[i]);
      for (const vertical of [true, false]) {
        const side = vertical ? box.w : box.h;
        if (side < 2) continue;
        const ideal = side * share;
        // The two whole-cell cuts around the ideal one.
        for (const cut of new Set([Math.floor(ideal), Math.ceil(ideal)])) {
          if (cut < 1 || cut > side - 1) continue;
          const a: Box = vertical ? { ...box, w: cut } : { ...box, h: cut };
          const b: Box = vertical ? { ...box, col: box.col + cut, w: box.w - cut } : { ...box, row: box.row + cut, h: box.h - cut };
          const pa = best(i, k, a);
          if (pa.cost >= plan.cost) continue;
          const pb = best(k, j, b);
          const cost = pa.cost + pb.cost;
          if (cost < plan.cost) plan = { cost, cells: [...pa.cells, ...pb.cells] };
        }
      }
    }
    memo.set(key, plan);
    return plan;
  };

  return best(0, sorted.length, { col: 1, row: 1, w: cols, h: rows }).cells;
}
