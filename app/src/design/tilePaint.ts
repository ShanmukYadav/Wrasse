import { useLayoutEffect, type RefObject } from "react";

// The D1 tile paint (a 160deg linear fill and a corner glow circle) as small images instead of
// CSS gradients. Same pixels: the fill is drawn with CSS's own gradient-line geometry onto a canvas
// with the tile's aspect ratio, and a linear gradient stays linear when that image is stretched
// evenly; the glow is a circle of the glow color drawn at 2x. Why: WebKit rasterizes every CSS
// gradient into its own surfaces each time a tile repaints, which cost about 60 MB of GPU memory
// in the first seconds after launch (measured with vmmap). A stretched image costs almost nothing.

const cache = new Map<string, string>();

/** Longest side of a fill image, in pixels. The gradient is smooth, so more adds nothing. */
const FILL_SIDE = 16;
/** Aspect-ratio rounding step for fill images. */
const ASPECT_STEP = 1.25;

/** A CSS `linear-gradient(angle, from 0%, to 100%)` for a `w`×`h` box, as a data URL. */
function fillImage(from: string, to: string, w: number, h: number, angleDeg = 160): string {
  // Aspect ratios are rounded to steps of 25%, so a screen of tiles shares a handful of images
  // (each one decoded once). On these fills the angle that rounding shifts changes no pixel by
  // more than one 8-bit step.
  const k = Math.max(-10, Math.min(10, Math.round(Math.log(Math.max(w, 1) / Math.max(h, 1)) / Math.log(ASPECT_STEP))));
  const aspect = ASPECT_STEP ** k;
  const pw = Math.max(2, Math.round(aspect >= 1 ? FILL_SIDE : FILL_SIDE * aspect));
  const ph = Math.max(2, Math.round(aspect >= 1 ? FILL_SIDE / aspect : FILL_SIDE));
  const key = `fill|${from}|${to}|${pw}x${ph}|${angleDeg}`;
  const hit = cache.get(key);
  if (hit) return hit;
  const canvas = document.createElement("canvas");
  canvas.width = pw;
  canvas.height = ph;
  const ctx = canvas.getContext("2d");
  if (!ctx) return "";
  // CSS: the gradient line passes through the centre at the given angle (0deg = up, clockwise),
  // and is long enough that the corners get the end colors.
  const a = (angleDeg * Math.PI) / 180;
  const dx = Math.sin(a);
  const dy = -Math.cos(a);
  const half = (Math.abs(pw * dx) + Math.abs(ph * dy)) / 2;
  const g = ctx.createLinearGradient(pw / 2 - dx * half, ph / 2 - dy * half, pw / 2 + dx * half, ph / 2 + dy * half);
  g.addColorStop(0, from);
  g.addColorStop(1, to);
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, pw, ph);
  const url = `url("${canvas.toDataURL("image/png")}")`;
  // Drop the canvas's pixels now rather than at the next garbage collection.
  canvas.width = 0;
  canvas.height = 0;
  cache.set(key, url);
  return url;
}

/** A filled circle of `color`, `size` CSS px across, drawn at 2x, as a data URL. */
function glowImage(color: string, size: number): string {
  const key = `glow|${color}|${size}`;
  const hit = cache.get(key);
  if (hit) return hit;
  const px = size * 2;
  const canvas = document.createElement("canvas");
  canvas.width = px;
  canvas.height = px;
  const ctx = canvas.getContext("2d");
  if (!ctx) return "";
  ctx.fillStyle = color;
  ctx.beginPath();
  ctx.arc(px / 2, px / 2, px / 2, 0, Math.PI * 2);
  ctx.fill();
  const url = `url("${canvas.toDataURL("image/png")}")`;
  canvas.width = 0;
  canvas.height = 0;
  cache.set(key, url);
  return url;
}

/** Puts the fill (and, with `glow`, the corner glow) on an element as `--fill-img`/`--glow-img`,
 *  before the first paint and again whenever the element changes size. */
export function useTilePaint(ref: RefObject<HTMLElement | null>, from: string, to: string, glow?: { color: string; size: number }) {
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    let last = "";
    const apply = () => {
      const w = el.offsetWidth;
      const h = el.offsetHeight;
      if (!w || !h) return;
      const fill = fillImage(from, to, w, h);
      if (fill !== last) {
        el.style.setProperty("--fill-img", fill);
        last = fill;
      }
    };
    apply();
    if (glow) el.style.setProperty("--glow-img", glowImage(glow.color, glow.size));
    const ro = new ResizeObserver(apply);
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref, from, to, glow?.color, glow?.size]);
}
