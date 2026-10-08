import { motion } from "motion/react";
import { useRef } from "react";
import { lensColors } from "../design/tokens";
import { useGlassRegion } from "../glass/regions";
import { t } from "../i18n";
import { useScheme } from "../lib/prefs";
import { spring } from "../motion/tokens";
import { useApp } from "../store/app";
import { LENSES } from "./lenses";

export function LensBar({ glassClass }: { glassClass: string }) {
  const ref = useRef<HTMLDivElement>(null);
  useGlassRegion("lens", ref, "bottom");
  const lens = useApp((s) => s.lens);
  const plan = useApp((s) => s.plan);
  const setLens = useApp((s) => s.setLens);
  const scheme = useScheme();

  return (
    <div ref={ref} className={`lens-bar ${glassClass}`} role="radiogroup" aria-label={t("lens.label")}>
      {LENSES.map(({ id, icon: Icon }) => {
        const active = !plan && lens === id;
        const color = id === "all" ? undefined : lensColors[scheme][id];
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
            <Icon weight="duotone" size={16} className="relative" style={color ? { color } : undefined} aria-hidden />
            <span className="relative">{t(`lens.${id}`)}</span>
          </button>
        );
      })}
    </div>
  );
}
