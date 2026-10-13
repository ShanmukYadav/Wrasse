import { useEffect, useRef } from "react";

// The sonar's rings and bubbles, drawn on one canvas the size of the scan area. Same look as the
// D1 sweep:
// - each ring grows from 8vmax to 160vmax across the sonar's duration (cubic-bezier 0.22, 1, 0.36,
//   1), its 2px border fading from the sonar ink to clear, the second ring 0.35 s behind the first;
// - each bubble (6px, 1px border) rises 40px while it fades in to 0.8 and out again.
// Growing a DOM ring by width repainted a 160vmax area (and the tiles under it) every frame, and
// every animated bubble was a composited layer whose surface WebKit kept after the scan: a canvas
// keeps one backing store of the scan area instead, dropped when the scan ends.

/** CSS `cubic-bezier(x1, y1, x2, y2)` as a function of progress 0–1. */
function bezier(x1: number, y1: number, x2: number, y2: number) {
  const cx = 3 * x1;
  const bx = 3 * (x2 - x1) - cx;
  const ax = 1 - cx - bx;
  const cy = 3 * y1;
  const by = 3 * (y2 - y1) - cy;
  const ay = 1 - cy - by;
  const xAt = (t: number) => ((ax * t + bx) * t + cx) * t;
  const yAt = (t: number) => ((ay * t + by) * t + cy) * t;
  const dxAt = (t: number) => (3 * ax * t + 2 * bx) * t + cx;
  return (x: number) => {
    let t = x;
    for (let i = 0; i < 8; i++) {
      const d = xAt(t) - x;
      if (Math.abs(d) < 1e-5) break;
      const slope = dxAt(t);
      if (Math.abs(slope) < 1e-6) break;
      t -= d / slope;
    }
    return yAt(Math.min(1, Math.max(0, t)));
  };
}

const ease = bezier(0.22, 1, 0.36, 1);
const DELAYS = [0, 0.35];

/** One bubble: its top-left offset from the centre (CSS px) and its animation delay (s). */
export type Bubble = { dx: number; dy: number; delay: number };

function rgba(color: string): [string, string, string, number] {
  const m = color.match(/rgba?\(([^)]+)\)/);
  const p = m ? m[1].split(",").map((x) => x.trim()) : ["15", "118", "110", "0.45"];
  return [p[0], p[1], p[2], p[3] === undefined ? 1 : Number(p[3])];
}

export function SonarRings({
  duration,
  ink,
  paused,
  bubbles,
  bubbleInk,
  bubbleDuration,
}: {
  duration: number;
  ink: string;
  paused: boolean;
  bubbles: Bubble[];
  bubbleInk: string;
  bubbleDuration: number;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  useEffect(() => {
    const el = canvas.current;
    const ctx = el?.getContext("2d");
    if (!el || !ctx) return;
    let frame = 0;
    let last = performance.now();
    let elapsed = 0;
    const size = () => {
      const dpr = window.devicePixelRatio || 1;
      const r = el.getBoundingClientRect();
      el.width = Math.max(1, Math.round(r.width * dpr));
      el.height = Math.max(1, Math.round(r.height * dpr));
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    size();
    const observer = new ResizeObserver(size);
    observer.observe(el);
    // The inks are rgba(r, g, b, a): borders fade by scaling that alpha.
    const [r, g, b, alpha] = rgba(ink);
    const [br, bg, bb, balpha] = rgba(bubbleInk);
    const draw = (now: number) => {
      if (!pausedRef.current) elapsed += (now - last) / 1000;
      last = now;
      const w = el.clientWidth;
      const h = el.clientHeight;
      const vmax = Math.max(window.innerWidth, window.innerHeight) / 100;
      ctx.clearRect(0, 0, w, h);
      ctx.lineWidth = 2;
      for (const delay of DELAYS) {
        const t = elapsed - delay;
        if (t < 0) continue;
        const p = ease((t % duration) / duration);
        const diameter = 8 * vmax + (160 - 8) * vmax * p;
        // The DOM ring's 2px border sits inside its box: the stroke's centre is 1px in.
        const radius = diameter / 2 - 1;
        ctx.strokeStyle = `rgba(${r}, ${g}, ${b}, ${alpha * (1 - p)})`;
        ctx.beginPath();
        ctx.arc(w / 2, h / 2, Math.max(0, radius), 0, Math.PI * 2);
        ctx.stroke();
      }
      // Bubbles: translateY(0 → -40px) over the whole cycle; opacity 0 → 0.8 → 0, each half eased
      // on its own (CSS applies the timing function per keyframe segment).
      ctx.lineWidth = 1;
      for (const bub of bubbles) {
        const t = elapsed - bub.delay;
        // Before its delay a bubble holds its first frame (opacity 0): nothing to draw.
        if (t < 0) continue;
        const p = (t % bubbleDuration) / bubbleDuration;
        const rise = -40 * ease(p);
        const opacity = p < 0.5 ? 0.8 * ease(p / 0.5) : 0.8 * (1 - ease((p - 0.5) / 0.5));
        if (opacity <= 0.001) continue;
        ctx.strokeStyle = `rgba(${br}, ${bg}, ${bb}, ${balpha * opacity})`;
        ctx.beginPath();
        ctx.arc(w / 2 + bub.dx + 3, h / 2 + bub.dy + 3 + rise, 2.5, 0, Math.PI * 2);
        ctx.stroke();
      }
      frame = requestAnimationFrame(draw);
    };
    frame = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      // Let WebKit drop the backing store right away.
      el.width = 0;
      el.height = 0;
    };
  }, [duration, ink, bubbles, bubbleInk, bubbleDuration]);

  return <canvas ref={canvas} className="sonar-rings" aria-hidden />;
}
