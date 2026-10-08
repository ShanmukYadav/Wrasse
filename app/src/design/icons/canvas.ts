// Draws the same icons on a Canvas 2D context, so map tiles match the DOM exactly.
import { icon as iconTokens } from "../tokens";
import { ICONS, isFillable, type IconElement, type IconName } from "./nodes";

type Prepared = { path: Path2D; fill: boolean; dash: number[] | null };
const cache = new Map<IconName, Prepared[]>();

function toPath([tag, a]: IconElement): Path2D {
  const p = new Path2D();
  const n = (k: string) => Number(a[k] ?? 0);
  switch (tag) {
    case "path":
      return new Path2D(String(a.d));
    case "rect":
      p.roundRect(n("x"), n("y"), n("width"), n("height"), n("rx"));
      return p;
    case "circle":
      p.arc(n("cx"), n("cy"), n("r"), 0, Math.PI * 2);
      return p;
    case "ellipse":
      p.ellipse(n("cx"), n("cy"), n("rx"), n("ry"), 0, 0, Math.PI * 2);
      return p;
    case "line":
      p.moveTo(n("x1"), n("y1"));
      p.lineTo(n("x2"), n("y2"));
      return p;
    case "polyline":
    case "polygon": {
      const pts = String(a.points).trim().split(/[\s,]+/).map(Number);
      for (let i = 0; i < pts.length; i += 2) (i ? p.lineTo : p.moveTo).call(p, pts[i], pts[i + 1]);
      if (tag === "polygon") p.closePath();
      return p;
    }
    default:
      return p;
  }
}

function prepared(name: IconName): Prepared[] {
  let p = cache.get(name);
  if (!p) {
    p = ICONS[name].map((el) => ({
      path: toPath(el),
      fill: isFillable(el),
      dash: el[1]["stroke-dasharray"] ? String(el[1]["stroke-dasharray"]).split(/\s+/).map(Number) : null,
    }));
    cache.set(name, p);
  }
  return p;
}

export function drawIcon(ctx: CanvasRenderingContext2D, name: IconName, x: number, y: number, size: number, color: string) {
  const s = size / iconTokens.grid;
  ctx.save();
  ctx.translate(x, y);
  ctx.scale(s, s);
  const alpha = ctx.globalAlpha;
  ctx.fillStyle = color;
  ctx.globalAlpha = alpha * iconTokens.fillOpacity;
  for (const p of prepared(name)) if (p.fill) ctx.fill(p.path);
  ctx.globalAlpha = alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = iconTokens.stroke;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  for (const p of prepared(name)) {
    ctx.setLineDash(p.dash ?? []);
    ctx.stroke(p.path);
  }
  ctx.restore();
}
