// Holds the single MapRenderer so store actions can drive the canvas imperatively.
import type { MapRenderer } from "./renderer";

let current: MapRenderer | null = null;

export function setRenderer(r: MapRenderer | null) {
  current = r;
  if (import.meta.env.DEV) (window as unknown as { __wrasseRenderer: unknown }).__wrasseRenderer = r;
}

export function renderer(): MapRenderer | null {
  return current;
}
