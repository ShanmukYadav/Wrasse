import { motion } from "motion/react";
import { BrandMark } from "../../design/brand/BrandMark";
import { reef } from "../../design/tokens";
import { useReducedMotion, useScheme } from "../../lib/prefs";
import { ease, mapMotion } from "../../motion/tokens";

// The scan: a calm sonar ring sweeping out from the center, the Wrasse mark at its heart and a
// few bubbles rising. Transform and opacity only; reduced motion keeps just a fade.
export function Sonar() {
  const scheme = useScheme();
  const reduced = useReducedMotion();
  if (reduced) return null;
  return (
    <div className="sonar" aria-hidden>
      {[0, 1].map((i) => (
        <motion.span
          key={i}
          className="sonar-ring"
          style={{ borderColor: reef.sonar[scheme] }}
          initial={{ scale: 0.05, opacity: 0.9 }}
          animate={{ scale: 1, opacity: 0 }}
          transition={{ duration: mapMotion.sonar, ease: ease.out, delay: i * 0.35 }}
        />
      ))}
      <motion.span
        className="sonar-mark"
        initial={{ opacity: 0, scale: 0.9 }}
        animate={{ opacity: [0, 1, 0], scale: [0.9, 1, 1.04] }}
        transition={{ duration: mapMotion.sonar, ease: ease.inOut, times: [0, 0.2, 1] }}
      >
        <BrandMark size={56} />
      </motion.span>
      {Array.from({ length: reef.bubbleCount }, (_, i) => {
        const a = (i * 137.5 * Math.PI) / 180;
        const r = 60 + ((i * 53) % 260);
        return (
          <motion.span
            key={i}
            className="sonar-bubble"
            style={{ left: `calc(50% + ${Math.cos(a) * r}px)`, top: `calc(50% + ${Math.sin(a) * r * 0.6}px)`, borderColor: reef.bubble[scheme] }}
            initial={{ y: 0, opacity: 0 }}
            animate={{ y: -40, opacity: [0, 0.8, 0] }}
            transition={{ duration: mapMotion.bubbleRise, ease: ease.out, delay: (r / 320) * mapMotion.sonar * 0.6 }}
          />
        );
      })}
    </div>
  );
}
