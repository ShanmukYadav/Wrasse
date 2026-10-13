import { useEffect, useRef } from "react";

// The sonar's rings and bubbles, drawn as SVG circles over the scan area. Same look as the D1
// sweep:
// - each ring grows from 8vmax to 160vmax across the sonar's duration (cubic-bezier 0.22, 1, 0.36,
//   1), its 2px border fading from the sonar ink to clear, the second ring 0.35 s behind the first;
// - each bubble (6px, 1px border) rises 40px while it fades in to 0.8 and out again.
// Memory (measured with vmmap): the old DOM rings sat in an overflow clip that made the whole scan
// area a layer of its own, and every animated bubble was a composited layer; a canvas instead was
// a window-sized surface buffered three times on the GPU (~55 MB). These circles are attributes
// repainted in place: they draw into the page's own layer, with no surface of their own.

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
  const svg = useRef<SVGSVGElement>(null);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  useEffect(() => {
    const el = svg.current;
    if (!el) return;
    const rings = Array.from(el.querySelectorAll<SVGCircleElement>("circle.ring"));
    const dots = Array.from(el.querySelectorAll<SVGCircleElement>("circle.bubble"));
    const [r, g, b, alpha] = rgba(ink);
    const [br, bg, bb, balpha] = rgba(bubbleInk);
    let frame = 0;
    let last = performance.now();
    let elapsed = 0;
    const draw = (now: number) => {
      if (!pausedRef.current) elapsed += (now - last) / 1000;
      last = now;
      const w = el.clientWidth;
      const h = el.clientHeight;
      const vmax = Math.max(window.innerWidth, window.innerHeight) / 100;
      DELAYS.forEach((delay, i) => {
        const ring = rings[i];
        const t = elapsed - delay;
        if (!ring) return;
        if (t < 0) {
          ring.setAttribute("stroke", "transparent");
          return;
        }
        const p = ease((t % duration) / duration);
        const diameter = 8 * vmax + (160 - 8) * vmax * p;
        // The DOM ring's 2px border sat inside its box: the stroke's centre is 1px in.
        ring.setAttribute("cx", String(w / 2));
        ring.setAttribute("cy", String(h / 2));
        ring.setAttribute("r", String(Math.max(0, diameter / 2 - 1)));
        ring.setAttribute("stroke", `rgba(${r}, ${g}, ${b}, ${alpha * (1 - p)})`);
      });
      // Bubbles: translateY(0 → -40px) over the whole cycle; opacity 0 → 0.8 → 0, each half eased
      // on its own (CSS applies the timing function per keyframe segment).
      bubbles.forEach((bub, i) => {
        const dot = dots[i];
        if (!dot) return;
        const t = elapsed - bub.delay;
        // Before its delay a bubble holds its first frame (opacity 0).
        const p = t < 0 ? 0 : (t % bubbleDuration) / bubbleDuration;
        const opacity = t < 0 ? 0 : p < 0.5 ? 0.8 * ease(p / 0.5) : 0.8 * (1 - ease((p - 0.5) / 0.5));
        dot.setAttribute("cx", String(w / 2 + bub.dx + 3));
        dot.setAttribute("cy", String(h / 2 + bub.dy + 3 - 40 * ease(p)));
        dot.setAttribute("stroke", `rgba(${br}, ${bg}, ${bb}, ${balpha * opacity})`);
      });
      frame = requestAnimationFrame(draw);
    };
    frame = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(frame);
  }, [duration, ink, bubbles, bubbleInk, bubbleDuration]);

  return (
    <svg ref={svg} className="sonar-rings" aria-hidden>
      {DELAYS.map((d) => (
        <circle key={d} className="ring" fill="none" stroke="transparent" strokeWidth={2} />
      ))}
      {bubbles.map((_, i) => (
        <circle key={i} className="bubble" r={2.5} fill="none" stroke="transparent" strokeWidth={1} />
      ))}
    </svg>
  );
}
