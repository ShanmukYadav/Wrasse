import { motion } from "motion/react";
import { useEffect, useState } from "react";
import { d1 } from "../design/tokens";
import { ease, mapMotion } from "../motion/tokens";
import { useApp, type Flight } from "../store/app";

// Quarantined blocks lift, shrink and fly into the tray icon.
export function Flyers() {
  const flights = useApp((s) => s.flights);
  return (
    <div className="flyers" aria-hidden>
      {flights.map((f) => (
        <Flyer key={f.key} flight={f} />
      ))}
    </div>
  );
}

function Flyer({ flight }: { flight: Flight }) {
  const endFlight = useApp((s) => s.endFlight);
  const [target, setTarget] = useState<{ x: number; y: number } | null>(null);
  useEffect(() => {
    const el = document.getElementById("tray-target");
    const r = el?.getBoundingClientRect();
    setTarget(r ? { x: r.left + r.width / 2, y: r.top + r.height / 2 } : { x: window.innerWidth - 60, y: window.innerHeight - 40 });
  }, []);
  if (!target) return null;
  const { from } = flight;
  const w = Math.max(8, from.w);
  const h = Math.max(8, from.h);
  const dx = target.x - (from.x + w / 2);
  const dy = target.y - (from.y + h / 2);
  const end = 14 / Math.max(w, h);
  return (
    <motion.div
      className="flyer"
      style={{ left: from.x, top: from.y, width: w, height: h, background: flight.color, borderRadius: d1.tile.radius }}
      initial={{ x: 0, y: 0, scale: 1, opacity: 1 }}
      animate={{
        x: [0, 0, dx],
        y: [0, -8, dy],
        scale: [1, 1.04, end],
        opacity: [1, 1, 0.2],
      }}
      transition={{ duration: mapMotion.flight, times: [0, 0.18, 1], ease: ease.inOut, delay: flight.delay }}
      onAnimationComplete={() => endFlight(flight.key)}
    />
  );
}
