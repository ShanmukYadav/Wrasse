// Canvas 2D renderer for the Reef Map.
//
// Each (layout, lens, theme) is rasterized once into an offscreen bitmap ("snapshot"). Steady
// frames, lens crossfades, camera zooms and the sonar sweep then composite one or two bitmaps,
// so frame cost stays flat as the block count grows. Only the short reflow after removing blocks
// draws blocks individually.
import { categoryShades, lensColors, map as M, palette, type LensId, type Scheme } from "../design/tokens";
import { t } from "../i18n";
import { formatBytes, type UnitSystem } from "../lib/format";
import { clamp01, easeInOut, easeOut, smoothstep } from "../motion/easing";
import { duration, mapMotion } from "../motion/tokens";
import { LENS_BIT, type Disk, type DiskNode } from "./data";
import { layoutFocus, lerpRect, type Item, type Layout, type Rect } from "./layout";

/** A lens, or the "free N GB" plan, which dims everything and lights up candidates. */
export type LensKey = LensId | "plan";
export type RemovalMode = "quarantine" | "delete";
export type Hole = Rect & { r: number };
export type FrameStats = { fps: number; worstMs: number; drawMs: number; frames: number };

type Transform = { sx: number; sy: number; tx: number; ty: number };

const IDENTITY: Transform = { sx: 1, sy: 1, tx: 0, ty: 0 };
const MAX_LABELS = 160;
const ROUND_MIN = 12; // px: smaller blocks are filled as plain rects (rounding is invisible there)
const SNAPSHOT_CACHE = 4;

export class MapRenderer {
  private ctx: CanvasRenderingContext2D;
  private dpr = 1;
  width = 0;
  height = 0;
  disk: Disk;
  view: Layout | null = null;
  scheme: Scheme = "dark";
  units: UnitSystem = "decimal";
  reduced = false;

  private layoutIds = new WeakMap<Layout, number>();
  private nextLayoutId = 1;
  private snapshots = new Map<string, HTMLCanvasElement>();
  private byArea = new WeakMap<Layout, Item[]>();
  private labelCache = new Map<string, string>();

  private lens = { from: "all" as LensKey, to: "all" as LensKey, start: -Infinity };
  private hover: number | null = null;
  private selected: number | null = null;
  private candidates: { ids: number[]; start: number } | null = null;
  private pulseState: { id: number; start: number } | null = null;
  private scan: { start: number; done: () => void } | null = null;
  private camera: { from: Layout; to: Layout; dir: "in" | "out"; rect: Rect; start: number; done: () => void } | null = null;
  private reflow: { from: Layout; to: Layout; start: number; modes: Map<number, RemovalMode> } | null = null;
  private holes: Hole[] = [];
  private raf = 0;

  private frameTimes: number[] = [];
  private drawTimes: number[] = [];
  private log: { dt: number[]; draw: number[] } = { dt: [], draw: [] };
  private rasterMs: number[] = [];
  private lastFrame = 0;
  stats: FrameStats = { fps: 0, worstMs: 0, drawMs: 0, frames: 0 };

  constructor(private canvas: HTMLCanvasElement, disk: Disk) {
    this.ctx = canvas.getContext("2d", { alpha: true })!;
    this.disk = disk;
  }

  // ---- configuration ----------------------------------------------------------------------

  resize(width: number, height: number, dpr: number) {
    this.width = width;
    this.height = height;
    this.dpr = dpr;
    this.canvas.width = Math.round(width * dpr);
    this.canvas.height = Math.round(height * dpr);
    const focus = this.view?.focus ?? this.disk.root;
    this.camera = null;
    this.reflow = null;
    this.snapshots.clear();
    this.view = layoutFocus(this.disk, focus, width, height);
    this.invalidate();
  }

  setDisk(disk: Disk) {
    this.disk = disk;
    this.camera = null;
    this.reflow = null;
    this.snapshots.clear();
    this.view = layoutFocus(disk, disk.root, this.width, this.height);
    this.invalidate();
  }

  setScheme(scheme: Scheme) {
    this.scheme = scheme;
    this.invalidate();
  }

  setUnits(units: UnitSystem) {
    this.units = units;
    this.invalidate();
  }

  setHoles(holes: Hole[]) {
    this.holes = holes;
    this.invalidate();
  }

  setLens(lens: LensKey) {
    if (lens === this.lens.to) return;
    const now = performance.now();
    this.lens = { from: this.lensAt(now) < 1 ? this.lens.from : this.lens.to, to: lens, start: now };
    this.invalidate();
  }

  setHover(id: number | null) {
    if (id === this.hover) return;
    this.hover = id;
    this.invalidate();
  }

  setSelected(id: number | null) {
    if (id === this.selected) return;
    this.selected = id;
    this.invalidate();
  }

  setCandidates(ids: number[] | null) {
    this.candidates = ids ? { ids, start: performance.now() } : null;
    this.invalidate();
  }

  pulse(id: number) {
    this.pulseState = { id, start: performance.now() };
    this.invalidate();
  }

  startScan(done: () => void) {
    this.scan = { start: performance.now(), done };
    this.invalidate();
  }

  get scanning() {
    return this.scan !== null;
  }

  get busy() {
    return this.camera !== null || this.reflow !== null;
  }

  // ---- transitions -----------------------------------------------------------------------

  /** Animates the camera to a new focus: zoom in, zoom out, or out-then-in via the common ancestor. */
  transitionTo(focus: number, done: () => void = () => {}) {
    const current = this.camera?.to ?? this.view;
    if (!current || focus === current.focus) return done();
    if (this.camera) {
      this.view = this.camera.to;
      this.camera = null;
    }
    const target = layoutFocus(this.disk, focus, this.width, this.height);
    if (this.isAncestor(current.focus, focus)) {
      this.startCamera(current, target, "in", done);
    } else if (this.isAncestor(focus, current.focus)) {
      this.startCamera(current, target, "out", done);
    } else {
      const lca = this.commonAncestor(current.focus, focus);
      const mid = layoutFocus(this.disk, lca, this.width, this.height);
      this.startCamera(current, mid, "out", () => this.startCamera(mid, target, "in", done));
    }
  }

  private startCamera(from: Layout, to: Layout, dir: "in" | "out", done: () => void) {
    const inner = dir === "in" ? to.focus : from.focus;
    const outer = dir === "in" ? from : to;
    const rect = this.rectOf(outer, inner) ?? { x: 0, y: 0, w: this.width, h: this.height };
    this.camera = { from, to, dir, rect, start: performance.now(), done };
    this.invalidate();
  }

  /** Re-lays out the current focus after the tree changed and animates blocks to their new rects. */
  relayout(modes: Map<number, RemovalMode> = new Map()) {
    if (!this.view) return;
    const from = this.camera?.to ?? this.view;
    const focus = this.disk.nodes[from.focus].size > 0 ? from.focus : this.disk.root;
    const to = layoutFocus(this.disk, focus, this.width, this.height);
    this.camera = null;
    this.view = to;
    this.reflow = { from, to, start: performance.now(), modes };
    this.invalidate();
  }

  // ---- queries -----------------------------------------------------------------------------

  hitTest(x: number, y: number): { id: number; group: boolean } | null {
    const v = this.view;
    if (!v || this.camera) return null;
    for (const g of v.groups) {
      if (x >= g.x && x <= g.x + g.w && y >= g.y && y <= g.y + M.headerHeight) return { id: g.id, group: true };
    }
    const b = v.index.hit(x, y);
    return b ? { id: b.id, group: false } : null;
  }

  /** Screen rect of a node in the current view, or of its nearest visible ancestor. */
  screenRect(id: number): Rect | null {
    return this.view ? this.rectOf(this.view, id) : null;
  }

  /** The visible block nearest to `from` in a direction, for arrow-key navigation. */
  neighbor(from: number | null, dir: "left" | "right" | "up" | "down"): number | null {
    const v = this.view;
    if (!v || v.blocks.length === 0) return null;
    const start = from !== null ? this.rectOf(v, from) : null;
    if (!start) return this.sortedByArea(v)[0].id;
    const cx = start.x + start.w / 2;
    const cy = start.y + start.h / 2;
    let best: Item | null = null;
    let bestScore = Infinity;
    for (const b of v.blocks) {
      if (b.id === from) continue;
      const dx = b.x + b.w / 2 - cx;
      const dy = b.y + b.h / 2 - cy;
      const along = dir === "left" ? -dx : dir === "right" ? dx : dir === "up" ? -dy : dy;
      const across = dir === "left" || dir === "right" ? Math.abs(dy) : Math.abs(dx);
      if (along <= 1) continue;
      const score = along + across * 2;
      if (score < bestScore) {
        bestScore = score;
        best = b;
      }
    }
    return best?.id ?? null;
  }

  /** The child of the current focus that contains `id` (the block a double-click drills into). */
  childOfFocusContaining(id: number): number | null {
    const v = this.view;
    if (!v) return null;
    let n: DiskNode = this.disk.nodes[id];
    while (n.parent >= 0 && n.parent !== v.focus) n = this.disk.nodes[n.parent];
    return n.parent === v.focus ? n.id : null;
  }

  private rectOf(layout: Layout, id: number): Rect | null {
    for (let n: DiskNode | undefined = this.disk.nodes[id]; n; n = n.parent >= 0 ? this.disk.nodes[n.parent] : undefined) {
      const item = layout.byId.get(n.id);
      if (item) return item;
    }
    return null;
  }

  private isAncestor(ancestor: number, id: number): boolean {
    for (let n = this.disk.nodes[id]; n.parent >= 0; n = this.disk.nodes[n.parent]) if (n.parent === ancestor) return true;
    return false;
  }

  private commonAncestor(a: number, b: number): number {
    const seen = new Set<number>();
    for (let n: number = a; n >= 0; n = this.disk.nodes[n].parent) seen.add(n);
    for (let n: number = b; n >= 0; n = this.disk.nodes[n].parent) if (seen.has(n)) return n;
    return this.disk.root;
  }

  private sortedByArea(layout: Layout): Item[] {
    let s = this.byArea.get(layout);
    if (!s) {
      s = [...layout.blocks].sort((a, b) => b.w * b.h - a.w * a.h);
      this.byArea.set(layout, s);
    }
    return s;
  }

  // ---- colors ------------------------------------------------------------------------------

  private colorFor(n: DiskNode, lens: LensKey): [string, number] {
    const base = categoryShades[this.scheme][n.category][n.shade];
    if (lens === "all") return [base, 1];
    if (lens === "plan") return [base, M.dimmedAlpha];
    return (n.deep & LENS_BIT[lens]) !== 0 ? [lensColors[this.scheme][lens], 1] : [base, M.dimmedAlpha];
  }

  private lensAt(now: number): number {
    return this.reduced ? 1 : easeInOut(clamp01((now - this.lens.start) / (duration.base * 1000)));
  }

  // ---- snapshots -----------------------------------------------------------------------------

  private snapshot(layout: Layout, lens: LensKey): HTMLCanvasElement {
    let id = this.layoutIds.get(layout);
    if (!id) this.layoutIds.set(layout, (id = this.nextLayoutId++));
    const key = `${id}|${lens}|${this.scheme}|${this.units}`;
    let snap = this.snapshots.get(key);
    if (snap) {
      this.snapshots.delete(key);
      this.snapshots.set(key, snap); // most recently used last
      return snap;
    }
    if (this.snapshots.size >= SNAPSHOT_CACHE) {
      const [oldestKey, oldest] = this.snapshots.entries().next().value as [string, HTMLCanvasElement];
      this.snapshots.delete(oldestKey);
      snap = oldest; // reuse the canvas
    } else {
      snap = document.createElement("canvas");
    }
    snap.width = this.canvas.width;
    snap.height = this.canvas.height;
    const c = snap.getContext("2d")!;
    c.clearRect(0, 0, snap.width, snap.height);
    const t0 = performance.now();
    this.paint(c, layout, lens);
    this.rasterMs.push(performance.now() - t0);
    this.snapshots.set(key, snap);
    return snap;
  }

  /** Full-quality draw of a layout in screen space: frames, blocks, outlines, labels, ghost. */
  private paint(c: CanvasRenderingContext2D, layout: Layout, lens: LensKey) {
    const pal = M[this.scheme];
    c.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);

    const frames = new Path2D();
    for (const g of layout.groups) frames.roundRect(g.x, g.y, g.w, g.h, M.radius + 2);
    c.fillStyle = pal.groupFrame;
    c.fill(frames);

    // Batch blocks by fill: rounded paths for larger blocks, plain rects for tiny ones.
    const batches = new Map<string, { color: string; alpha: number; path: Path2D; rects: Item[] }>();
    const outlines = new Path2D();
    for (const b of layout.blocks) {
      const n = this.disk.nodes[b.id];
      if (n.kind === "ghost") continue;
      const [color, alpha] = this.colorFor(n, lens);
      const key = `${color}|${alpha}`;
      let batch = batches.get(key);
      if (!batch) batches.set(key, (batch = { color, alpha, path: new Path2D(), rects: [] }));
      if (b.w >= ROUND_MIN && b.h >= ROUND_MIN) {
        const r = Math.min(M.radius, b.w / 2, b.h / 2);
        batch.path.roundRect(b.x, b.y, b.w, b.h, r);
        if (alpha === 1) outlines.roundRect(b.x + 0.5, b.y + 0.5, b.w - 1, b.h - 1, r);
      } else batch.rects.push(b);
    }
    for (const batch of batches.values()) {
      c.globalAlpha = batch.alpha;
      c.fillStyle = batch.color;
      c.fill(batch.path);
      for (const r of batch.rects) c.fillRect(r.x, r.y, r.w, r.h);
    }
    c.globalAlpha = 1;
    c.strokeStyle = pal.blockStroke;
    c.lineWidth = 1;
    c.stroke(outlines);

    this.paintLabels(c, layout, lens);
    this.paintGhost(c, layout, 1);
  }

  private paintLabels(c: CanvasRenderingContext2D, layout: Layout, lens: LensKey) {
    const pal = M[this.scheme];
    c.textBaseline = "alphabetic";
    c.font = M.headerFont;
    c.fillStyle = pal.header;
    for (const g of layout.groups) {
      if (g.w < 60) continue;
      const n = this.disk.nodes[g.id];
      c.fillText(this.fit(c, `h${g.id}`, `${n.name}  ·  ${this.bytes(n.size)}`, g.w - 12), g.x + 6, g.y + 13);
    }
    let drawn = 0;
    for (const b of this.sortedByArea(layout)) {
      if (b.w < 64 || b.h < 30 || ++drawn > MAX_LABELS) break;
      const n = this.disk.nodes[b.id];
      if (n.kind === "ghost") continue;
      const alpha = this.colorFor(n, lens)[1];
      // Lens-colored blocks are bright, so their labels switch to the dark on-accent color.
      const onLens = lens !== "all" && lens !== "plan" && alpha === 1;
      c.globalAlpha = alpha;
      c.font = M.labelFont;
      c.fillStyle = onLens ? palette[this.scheme].onAccent : pal.label;
      c.fillText(this.fit(c, `n${b.id}`, n.name, b.w - 14), b.x + 7, b.y + 17);
      if (b.h >= 44) {
        c.font = M.sizeFont;
        c.globalAlpha = onLens ? 0.7 : alpha;
        c.fillStyle = onLens ? palette[this.scheme].onAccent : pal.labelMuted;
        c.fillText(this.bytes(n.size), b.x + 7, b.y + 32);
      }
    }
    c.globalAlpha = 1;
  }

  private paintGhost(c: CanvasRenderingContext2D, layout: Layout, alpha: number) {
    const item = layout.byId.get(this.disk.ghostId);
    if (!item || alpha <= 0) return;
    const pal = M[this.scheme];
    c.globalAlpha = alpha;
    c.fillStyle = pal.ghostFill;
    c.strokeStyle = pal.ghostStroke;
    c.lineWidth = 1.5;
    c.setLineDash([...M.ghostDash]);
    c.beginPath();
    c.roundRect(item.x + 1, item.y + 1, item.w - 2, item.h - 2, M.radius);
    c.fill();
    c.stroke();
    c.setLineDash([]);
    if (item.w > 120 && item.h > 40) {
      c.font = M.labelFont;
      c.fillStyle = pal.ghostLabel;
      const label = t("map.ghost", { size: this.bytes(this.disk.nodes[this.disk.ghostId].size) });
      c.fillText(this.fit(c, "ghost", label, item.w - 16), item.x + 8, item.y + 20);
      c.font = M.sizeFont;
      c.fillStyle = pal.selection;
      c.fillText(t("map.ghostAction"), item.x + 8, item.y + 36);
    }
    c.globalAlpha = 1;
  }

  private fit(c: CanvasRenderingContext2D, key: string, text: string, maxW: number): string {
    const k = `${key}:${text}:${Math.floor(maxW / 8)}`;
    const cached = this.labelCache.get(k);
    if (cached !== undefined) return cached;
    let out = text;
    if (c.measureText(out).width > maxW) {
      let lo = 0;
      let hi = text.length;
      while (lo < hi) {
        const mid = (lo + hi + 1) >> 1;
        if (c.measureText(text.slice(0, mid) + "…").width <= maxW) lo = mid;
        else hi = mid - 1;
      }
      out = lo > 0 ? text.slice(0, lo) + "…" : "";
    }
    if (this.labelCache.size > 20000) this.labelCache.clear();
    this.labelCache.set(k, out);
    return out;
  }

  private bytes(n: number): string {
    const f = formatBytes(n, this.units);
    return `${f.value} ${f.unit}`;
  }

  // ---- frame loop ----------------------------------------------------------------------------

  invalidate() {
    if (!this.raf) this.raf = requestAnimationFrame(this.frame);
  }

  private frame = (now: number) => {
    this.raf = 0;
    const t0 = performance.now();
    const animating = this.draw(now);
    const t1 = performance.now();
    if (animating) {
      if (this.lastFrame) this.record(now - this.lastFrame, t1 - t0);
      this.lastFrame = now;
      this.raf = requestAnimationFrame(this.frame);
    } else {
      this.lastFrame = 0;
    }
  };

  /** Returns and clears every frame recorded since the last call (for the benchmark). */
  takeFrames(): { dt: number[]; draw: number[]; raster: number[] } {
    const out = { ...this.log, raster: this.rasterMs };
    this.log = { dt: [], draw: [] };
    this.rasterMs = [];
    return out;
  }

  private record(dt: number, drawMs: number) {
    this.log.dt.push(dt);
    this.log.draw.push(drawMs);
    this.frameTimes.push(dt);
    this.drawTimes.push(drawMs);
    if (this.frameTimes.length > 120) {
      this.frameTimes.shift();
      this.drawTimes.shift();
    }
    const avg = this.frameTimes.reduce((s, v) => s + v, 0) / this.frameTimes.length;
    this.stats = {
      fps: 1000 / avg,
      worstMs: Math.max(...this.frameTimes),
      drawMs: this.drawTimes.reduce((s, v) => s + v, 0) / this.drawTimes.length,
      frames: this.stats.frames + 1,
    };
  }

  /** Draws one frame; returns whether another frame is needed. */
  private draw(now: number): boolean {
    const ctx = this.ctx;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalAlpha = 1;
    ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    ctx.fillStyle = M[this.scheme].background;
    ctx.fillRect(0, 0, this.width, this.height);
    if (!this.view) return false;

    const lensT = this.lensAt(now);
    let animating = lensT < 1;

    if (this.camera) animating = this.drawCamera(now, lensT) || animating;
    else if (this.reflow) animating = this.drawReflow(now) || animating;
    else if (this.scan) animating = this.drawScan(now, lensT) || animating;
    else this.composite(this.view, IDENTITY, 1, lensT);

    if (!this.camera && !this.scan) {
      animating = this.drawCandidates(now) || animating;
      this.drawHoverAndSelection();
      animating = this.drawPulse(now) || animating;
    }

    // Cut transparent holes where native glass sits under the webview.
    if (this.holes.length) {
      ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
      ctx.globalAlpha = 1;
      ctx.globalCompositeOperation = "destination-out";
      ctx.beginPath();
      for (const h of this.holes) ctx.roundRect(h.x, h.y, h.w, h.h, h.r);
      ctx.fill();
      ctx.globalCompositeOperation = "source-over";
    }
    return animating;
  }

  /** Draws a layout's snapshot(s) under a transform, crossfading between lenses. */
  private composite(layout: Layout, tr: Transform, alpha: number, lensT: number) {
    const ctx = this.ctx;
    ctx.setTransform(this.dpr * tr.sx, 0, 0, this.dpr * tr.sy, this.dpr * tr.tx, this.dpr * tr.ty);
    ctx.imageSmoothingQuality = "medium";
    if (lensT < 1 && this.lens.from !== this.lens.to) {
      ctx.globalAlpha = alpha * (1 - lensT);
      ctx.drawImage(this.snapshot(layout, this.lens.from), 0, 0, this.width, this.height);
      ctx.globalAlpha = alpha * lensT;
    } else {
      ctx.globalAlpha = alpha;
    }
    ctx.drawImage(this.snapshot(layout, this.lens.to), 0, 0, this.width, this.height);
    ctx.globalAlpha = 1;
  }

  private drawCamera(now: number, lensT: number): boolean {
    const cam = this.camera!;
    const p = clamp01((now - cam.start) / (duration.slow * 1000));
    const e = easeOut(p);
    const W = this.width;
    const H = this.height;

    if (this.reduced) {
      // Reduced motion: no camera movement, just a crossfade.
      this.composite(cam.from, IDENTITY, 1 - e, lensT);
      this.composite(cam.to, IDENTITY, e, lensT);
    } else {
      // Camera rect in the outer layout's space: full view ↔ inner rect, zooming geometrically.
      const r = cam.rect;
      const z = cam.dir === "in" ? e : 1 - e;
      const cw = W * Math.pow(r.w / W, z);
      const ch = H * Math.pow(r.h / H, z);
      const k = W - r.w > 0.5 ? (W - cw) / (W - r.w) : z;
      const cx = W / 2 + (r.x + r.w / 2 - W / 2) * k;
      const cy = H / 2 + (r.y + r.h / 2 - H / 2) * k;
      const outer: Transform = { sx: W / cw, sy: H / ch, tx: -(cx - cw / 2) * (W / cw), ty: -(cy - ch / 2) * (H / ch) };
      const inner: Transform = { sx: (r.w / W) * outer.sx, sy: (r.h / H) * outer.sy, tx: r.x * outer.sx + outer.tx, ty: r.y * outer.sy + outer.ty };
      const outerLayout = cam.dir === "in" ? cam.from : cam.to;
      const innerLayout = cam.dir === "in" ? cam.to : cam.from;
      this.composite(outerLayout, outer, 1 - smoothstep(0.45, 1, z), lensT);
      const ctx = this.ctx;
      ctx.save();
      ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
      ctx.beginPath();
      ctx.rect(inner.tx, inner.ty, W * inner.sx, H * inner.sy);
      ctx.clip();
      this.composite(innerLayout, inner, smoothstep(0.05, 0.6, z), lensT);
      ctx.restore();
    }

    if (p >= 1) {
      this.view = cam.to;
      this.camera = null;
      cam.done();
    }
    return true;
  }

  private drawReflow(now: number): boolean {
    const rf = this.reflow!;
    const p = clamp01((now - rf.start) / (duration.slow * 1000));
    const e = this.reduced ? 1 : easeOut(p);
    const ctx = this.ctx;
    const pal = M[this.scheme];
    const lens = this.lens.to;
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    ctx.globalAlpha = 1;
    ctx.fillStyle = pal.groupFrame;
    for (const g of rf.to.groups) {
      const from = rf.from.byId.get(g.id);
      const r = from ? lerpRect(from, g, e) : g;
      ctx.beginPath();
      ctx.roundRect(r.x, r.y, r.w, r.h, M.radius + 2);
      ctx.fill();
    }
    for (const b of rf.to.blocks) {
      const n = this.disk.nodes[b.id];
      if (n.kind === "ghost") continue;
      const from = rf.from.byId.get(b.id);
      const r = from ? lerpRect(from, b, e) : b;
      const [color, a] = this.colorFor(n, lens);
      ctx.globalAlpha = a * (from ? 1 : e);
      ctx.fillStyle = color;
      this.block(r);
    }
    // Permanently deleted blocks fade and collapse in place; quarantined ones already flew away.
    for (const b of rf.from.blocks) {
      if (rf.to.byId.has(b.id) || this.removalMode(rf.modes, b.id) !== "delete") continue;
      const n = this.disk.nodes[b.id];
      if (n.kind === "ghost") continue;
      const s = 1 - e * 0.85;
      const [color, a] = this.colorFor(n, lens);
      ctx.globalAlpha = a * (1 - e);
      ctx.fillStyle = color;
      this.block({ x: b.x + (b.w * (1 - s)) / 2, y: b.y + (b.h * (1 - s)) / 2, w: b.w * s, h: b.h * s });
    }
    ctx.globalAlpha = 1;
    if (rf.to.byId.has(this.disk.ghostId)) this.paintGhost(ctx, rf.to, 1);
    else this.paintGhost(ctx, rf.from, 1 - e);
    if (p >= 1) this.reflow = null;
    return true;
  }

  private block(r: Rect) {
    if (r.w >= ROUND_MIN && r.h >= ROUND_MIN) {
      this.ctx.beginPath();
      this.ctx.roundRect(r.x, r.y, r.w, r.h, Math.min(M.radius, r.w / 2, r.h / 2));
      this.ctx.fill();
    } else {
      this.ctx.fillRect(r.x, r.y, r.w, r.h);
    }
  }

  private removalMode(modes: Map<number, RemovalMode>, id: number): RemovalMode | undefined {
    for (let n: DiskNode | undefined = this.disk.nodes[id]; n; n = n.parent >= 0 ? this.disk.nodes[n.parent] : undefined) {
      const m = modes.get(n.id);
      if (m) return m;
    }
    return undefined;
  }

  private drawScan(now: number, lensT: number): boolean {
    const scan = this.scan!;
    const view = this.view!;
    const p = clamp01((now - scan.start) / (mapMotion.sonar * 1000));
    const ctx = this.ctx;
    const pal = M[this.scheme];

    if (this.reduced) {
      // Reduced motion: the sonar becomes a fade.
      this.composite(view, IDENTITY, easeInOut(p), lensT);
    } else {
      const cx = this.width / 2;
      const cy = this.height / 2;
      const feather = mapMotion.sonarFeather;
      const R = easeOut(p) * (Math.hypot(cx, cy) + feather);
      ctx.save();
      ctx.beginPath();
      ctx.arc(cx, cy, Math.max(0, R - feather), 0, Math.PI * 2);
      ctx.clip();
      this.composite(view, IDENTITY, 1, lensT);
      ctx.restore();
      // Blocks in the sonar band fade and scale in as the sweep passes them.
      ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
      const r2min = Math.max(0, R - feather) ** 2;
      const r2max = R * R;
      for (const b of view.blocks) {
        const dx = b.x + b.w / 2 - cx;
        const dy = b.y + b.h / 2 - cy;
        const d2 = dx * dx + dy * dy;
        if (d2 < r2min || d2 >= r2max) continue;
        const n = this.disk.nodes[b.id];
        if (n.kind === "ghost") continue;
        const q = (R - Math.sqrt(d2)) / feather;
        const s = 0.6 + 0.4 * q;
        const [color, a] = this.colorFor(n, this.lens.to);
        ctx.globalAlpha = a * q;
        ctx.fillStyle = color;
        this.block({ x: b.x + (b.w * (1 - s)) / 2, y: b.y + (b.h * (1 - s)) / 2, w: b.w * s, h: b.h * s });
      }
      ctx.strokeStyle = pal.sonar;
      ctx.globalAlpha = (1 - p) * 0.9;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(cx, cy, R, 0, Math.PI * 2);
      ctx.stroke();
      ctx.globalAlpha = (1 - p) * 0.15;
      ctx.lineWidth = 22;
      ctx.beginPath();
      ctx.arc(cx, cy, Math.max(0, R - 11), 0, Math.PI * 2);
      ctx.stroke();
      ctx.globalAlpha = 1;
    }
    if (p >= 1) {
      this.scan = null;
      scan.done();
      this.invalidate();
      return false;
    }
    return true;
  }

  private drawCandidates(now: number): boolean {
    const cands = this.candidates;
    if (!cands || !this.view) return false;
    const ctx = this.ctx;
    const pal = M[this.scheme];
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    let pending = false;
    cands.ids.forEach((id, i) => {
      const r = this.view!.byId.get(id);
      if (!r) return;
      const appear = this.reduced ? 1 : clamp01((now - cands.start - i * mapMotion.candidateStagger * 1000) / (duration.base * 1000));
      if (appear < 1) pending = true;
      if (appear <= 0) return;
      ctx.globalAlpha = 0.9 * appear;
      ctx.fillStyle = pal.candidate;
      this.block(r);
      if (r.w > 26 && r.h > 20) {
        ctx.globalAlpha = appear;
        ctx.font = M.labelFont;
        ctx.fillStyle = palette[this.scheme].onAccent;
        ctx.fillText(String(i + 1), r.x + 6, r.y + 16);
      }
    });
    ctx.globalAlpha = 1;
    return pending;
  }

  private drawHoverAndSelection() {
    const v = this.view!;
    const ctx = this.ctx;
    const pal = M[this.scheme];
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    ctx.globalAlpha = 1;
    if (this.hover !== null) {
      const r = v.byId.get(this.hover);
      if (r) {
        ctx.fillStyle = pal.blockHover;
        this.block(r);
      }
    }
    if (this.selected !== null) {
      const r = this.rectOf(v, this.selected);
      if (r) {
        ctx.strokeStyle = pal.selection;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.roundRect(r.x + 1, r.y + 1, r.w - 2, r.h - 2, Math.min(M.radius, r.w / 2, r.h / 2));
        ctx.stroke();
      }
    }
  }

  private drawPulse(now: number): boolean {
    const pulse = this.pulseState;
    if (!pulse || !this.view) return false;
    const elapsed = (now - pulse.start) / 1000;
    if (elapsed > mapMotion.pulse) {
      this.pulseState = null;
      return false;
    }
    const r = this.rectOf(this.view, pulse.id);
    if (!r) return false;
    const ctx = this.ctx;
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    ctx.strokeStyle = M[this.scheme].selection;
    for (let k = 0; k < 3; k++) {
      const q = (elapsed - k * 0.3) / (mapMotion.pulse - 0.6);
      if (q <= 0 || q >= 1) continue;
      const grow = (this.reduced ? 0 : 26) * easeOut(q);
      ctx.globalAlpha = 1 - q;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.roundRect(r.x - grow, r.y - grow, r.w + grow * 2, r.h + grow * 2, M.radius + grow);
      ctx.stroke();
    }
    ctx.globalAlpha = 1;
    return true;
  }

  destroy() {
    if (this.raf) cancelAnimationFrame(this.raf);
  }
}
