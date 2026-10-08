import { motion } from "motion/react";
import { t } from "../i18n";
import { spring } from "../motion/tokens";
import { useApp } from "../store/app";
import { LENSES } from "./lenses";

// Floating lens bar: matching tiles stay in full color, the rest fade.
export function LensBar() {
  const lens = useApp((s) => s.lens);
  const plan = useApp((s) => s.plan);
  const setLens = useApp((s) => s.setLens);
  return (
    <div className="lens-bar" role="radiogroup" aria-label={t("lens.label")}>
      {LENSES.map(({ id }) => {
        const active = !plan && lens === id;
        return (
          <button key={id} type="button" role="radio" aria-checked={active} className={`lens-pill focus-ring ${active ? "is-active" : ""}`} onClick={() => setLens(id)}>
            {active && <motion.span layoutId="lens-indicator" className="lens-indicator" transition={spring.soft} aria-hidden />}
            <span className="relative">{t(`lens.${id}`)}</span>
          </button>
        );
      })}
    </div>
  );
}
