// Draws the Wrasse mark on a Canvas (sonar scan center), in the icon family's duotone style.
import { icon as iconTokens } from "../tokens";
import { MARK } from "./mark";

let paths: { body: Path2D; tail: Path2D; stripe: Path2D } | null = null;

export function drawMark(ctx: CanvasRenderingContext2D, cx: number, cy: number, size: number, color: string) {
  paths ??= { body: new Path2D(MARK.body), tail: new Path2D(MARK.tail), stripe: new Path2D(MARK.stripe) };
  const s = size / 24;
  const alpha = ctx.globalAlpha;
  ctx.save();
  ctx.translate(cx - size / 2, cy - size / 2);
  ctx.scale(s, s);
  ctx.fillStyle = color;
  ctx.globalAlpha = alpha * iconTokens.fillOpacity;
  ctx.fill(paths.body);
  ctx.fill(paths.tail);
  ctx.globalAlpha = alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = iconTokens.stroke;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  ctx.stroke(paths.body);
  ctx.stroke(paths.tail);
  ctx.stroke(paths.stripe);
  ctx.beginPath();
  ctx.arc(MARK.eye.cx, MARK.eye.cy, MARK.eye.r, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}
