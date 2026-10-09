import { motion } from "motion/react";
import { useLayoutEffect, useRef, useState } from "react";
import { t } from "../i18n";
import { spring } from "../motion/tokens";
import { useApp } from "../store/app";
import { LENSES } from "./lenses";

// Floating lens bar: matching tiles stay in full color, the rest fade. One persistent navy pill
// slides to the active lens: it moves by transform and eases its width to the pill.
export function LensBar() {
  const lens = useApp((s) => s.lens);
  const plan = useApp((s) => s.plan);
  const setLens = useApp((s) => s.setLens);
  const pills = useRef<Record<string, HTMLButtonElement | null>>({});
  const [box, setBox] = useState<{ left: number; width: number } | null>(null);
  const active = plan ? null : lens;
  const selecting = useApp((s) => s.checked.length > 0);

  useLayoutEffect(() => {
    const measure = () => {
      const el = active ? pills.current[active] : null;
      setBox(el ? { left: el.offsetLeft, width: el.offsetWidth } : null);
    };
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [active]);

  if (selecting) return null; // the selection bar takes its place
  return (
    <div className="lens-bar" role="radiogroup" aria-label={t("lens.label")}>
      {box && (
        <motion.span
          className="lens-indicator"
          initial={false}
          animate={{ x: box.left, width: box.width }}
          transition={spring.soft}
          aria-hidden
        />
      )}
      {LENSES.map(({ id }) => {
        const on = active === id;
        return (
          <button
            key={id}
            ref={(el) => {
              pills.current[id] = el;
            }}
            type="button"
            role="radio"
            aria-checked={on}
            className={`lens-pill focus-ring ${on ? "is-active" : ""}`}
            onClick={() => setLens(id)}
          >
            <span className="relative">{t(`lens.${id}`)}</span>
          </button>
        );
      })}
    </div>
  );
}
