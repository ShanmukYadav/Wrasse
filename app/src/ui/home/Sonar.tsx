import type { CSSProperties } from "react";
import { BrandMark } from "../../design/brand/BrandMark";
import { reef } from "../../design/tokens";
import { useReducedMotion, useScheme } from "../../lib/prefs";
import { mapMotion } from "../../motion/tokens";

// The scan: a calm sonar ring sweeping out from the center, the Wrasse mark at its heart and a
// few bubbles rising. Plain CSS animations that play once: Home mounts this only while a scan is
// running (keyed by scan), so re-renders, hovers and lens changes never restart it, and it is
// gone the moment the scan ends. Reduced motion keeps only a fade of the mark.
export function Sonar() {
  const scheme = useScheme();
  const reduced = useReducedMotion();
  const vars = { "--sonar-dur": `${mapMotion.sonar}s`, "--bubble-dur": `${mapMotion.bubbleRise}s` } as CSSProperties;
  if (reduced) {
    return (
      <div className="sonar" style={vars} aria-hidden>
        <span className="sonar-mark is-fade">
          <BrandMark size={56} />
        </span>
      </div>
    );
  }
  return (
    <div className="sonar" style={vars} aria-hidden>
      {[0, 1].map((i) => (
        <span key={i} className="sonar-ring" style={{ borderColor: reef.sonar[scheme], animationDelay: `${i * 0.35}s` }} />
      ))}
      <span className="sonar-mark">
        <BrandMark size={56} />
      </span>
      {Array.from({ length: reef.bubbleCount }, (_, i) => {
        const a = (i * 137.5 * Math.PI) / 180;
        const r = 60 + ((i * 53) % 260);
        return (
          <span
            key={i}
            className="sonar-bubble"
            style={{
              left: `calc(50% + ${Math.cos(a) * r}px)`,
              top: `calc(50% + ${Math.sin(a) * r * 0.6}px)`,
              borderColor: reef.bubble[scheme],
              animationDelay: `${(r / 320) * mapMotion.sonar * 0.6}s`,
            }}
          />
        );
      })}
    </div>
  );
}
