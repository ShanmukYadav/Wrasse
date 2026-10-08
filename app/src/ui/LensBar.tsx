import { motion } from "motion/react";
import { useRef } from "react";
import { Icon } from "../design/icons/Icon";
import { useGlassRegion } from "../glass/regions";
import { t } from "../i18n";
import { spring } from "../motion/tokens";
import { useApp } from "../store/app";
import { LENSES } from "./lenses";

export function LensBar({ glassClass }: { glassClass: string }) {
  const ref = useRef<HTMLDivElement>(null);
  useGlassRegion("lens", ref, "bottom");
  const lens = useApp((s) => s.lens);
  const plan = useApp((s) => s.plan);
  const setLens = useApp((s) => s.setLens);

  return (
    <div ref={ref} className={`lens-bar ${glassClass}`} role="radiogroup" aria-label={t("lens.label")}>
      {LENSES.map(({ id, icon }) => {
        const active = !plan && lens === id;
        return (
          <button
            key={id}
            type="button"
            role="radio"
            aria-checked={active}
            className={`lens-pill focus-ring ${active ? "is-active" : ""}`}
            onClick={() => setLens(id)}
          >
            {active && <motion.span layoutId="lens-indicator" className="lens-indicator" transition={spring.soft} aria-hidden />}
            <span className="relative inline-flex"><Icon name={icon} size={16} style={id === "all" ? undefined : { color: `var(--lens-${id})` }} /></span>
            <span className="relative">{t(`lens.${id}`)}</span>
          </button>
        );
      })}
    </div>
  );
}
