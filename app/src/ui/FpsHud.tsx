import { useEffect, useRef } from "react";
import { renderer } from "../map/controller";
import { useApp } from "../store/app";

// Live frame stats from the map renderer (toggle in Settings).
export function FpsHud() {
  const show = useApp((s) => s.showFps);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!show) return;
    const id = setInterval(() => {
      const r = renderer();
      if (!r || !ref.current) return;
      const s = r.stats;
      ref.current.textContent = `${s.fps.toFixed(0)} fps · worst ${s.worstMs.toFixed(1)} ms · draw ${s.drawMs.toFixed(1)} ms · ${r.view?.blocks.length ?? 0} blocks`;
    }, 250);
    return () => clearInterval(id);
  }, [show]);
  if (!show) return null;
  return <div ref={ref} className="fps-hud tabular" aria-hidden />;
}
