// Keeps native glass views (drawn by Rust under the webview) aligned with the DOM controls they
// sit behind, and tells the map renderer to cut matching transparent holes.
import { useEffect, type RefObject } from "react";
import { setGlassRegions, type GlassRegion } from "../lib/backend";
import { renderer } from "../map/controller";
import { useApp } from "../store/app";

const elements = new Map<string, { el: HTMLElement; anchor: "top" | "bottom" }>();
let scheduled = 0;
let observer: ResizeObserver | null = null;
let lastSent = "";

export function nativeGlassActive(): boolean {
  const s = useApp.getState();
  return s.nativeGlass && s.glass === "native";
}

function collect(): GlassRegion[] {
  const out: GlassRegion[] = [];
  for (const [id, { el, anchor }] of elements) {
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) continue;
    const radius = parseFloat(getComputedStyle(el).borderTopLeftRadius) || 0;
    out.push({ id, x: r.left, y: r.top, w: r.width, h: r.height, radius, anchor });
  }
  return out;
}

export function syncGlass() {
  if (scheduled) return;
  scheduled = requestAnimationFrame(() => {
    scheduled = 0;
    const active = nativeGlassActive();
    const regions = active ? collect() : [];
    renderer()?.setHoles(regions.map((r) => ({ x: r.x, y: r.y, w: r.w, h: r.h, r: r.radius })));
    const key = JSON.stringify(regions);
    if (key !== lastSent) {
      lastSent = key;
      void setGlassRegions(regions);
    }
  });
}

/** Registers a persistent floating control as a native glass region. */
export function useGlassRegion(id: string, ref: RefObject<HTMLElement | null>, anchor: "top" | "bottom") {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    elements.set(id, { el, anchor });
    observer ??= new ResizeObserver(syncGlass);
    observer.observe(el);
    syncGlass();
    return () => {
      elements.delete(id);
      observer?.unobserve(el);
      syncGlass();
    };
  }, [id, ref, anchor]);
}

window.addEventListener("resize", syncGlass);
