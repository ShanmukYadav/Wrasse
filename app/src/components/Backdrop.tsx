import { AnimatePresence, motion } from "motion/react";
import { sectionGradient } from "../design/tokens";
import { duration, ease } from "../motion/tokens";
import { screenById } from "../screens/registry";
import { useApp } from "../store/app";

// Full-window section gradient. The new layer fades in over the old one, which stays opaque
// until it is removed, so the crossfade never dips to the window color. Opacity only.
export function Backdrop() {
  const hue = useApp((s) => screenById(s.screen).hue);
  const scheme = useApp((s) => s.scheme);
  const key = `${hue}-${scheme}`;
  return (
    <div className="backdrop" aria-hidden>
      <AnimatePresence initial={false}>
        <motion.div
          key={key}
          className="backdrop-layer"
          style={{ background: sectionGradient(hue, scheme) }}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1, transition: { duration: duration.slow, ease: ease.inOut } }}
          // A no-op exit would finish instantly; this near-1 target keeps the old layer for the full crossfade.
          exit={{ opacity: 0.99, transition: { duration: duration.slow } }}
        />
      </AnimatePresence>
    </div>
  );
}
