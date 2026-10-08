import { CaretRight, ListBullets, SquaresFour } from "@phosphor-icons/react";
import { AnimatePresence, motion } from "motion/react";
import { t } from "../i18n";
import { duration, ease, spring } from "../motion/tokens";
import { ancestors } from "../api/types";
import { useApp } from "../store/app";

export function TopLeft() {
  const viewMode = useApp((s) => s.viewMode);
  const setViewMode = useApp((s) => s.setViewMode);
  const focus = useApp((s) => s.focus);
  const disk = useApp((s) => s.disk);
  const setFocus = useApp((s) => s.setFocus);
  const trail = [...ancestors(disk, focus), focus];

  return (
    <div className="top-left">
      <div className="view-toggle glass-css" role="radiogroup" aria-label={t("view.label")}>
        {(["map", "list"] as const).map((mode) => (
          <button
            key={mode}
            type="button"
            role="radio"
            aria-checked={viewMode === mode}
            aria-label={t(`view.${mode}`)}
            title={t(`view.${mode}`)}
            className="toggle-btn focus-ring"
            onClick={() => setViewMode(mode)}
          >
            {viewMode === mode && <motion.span layoutId="view-toggle" className="toggle-indicator" transition={spring.soft} aria-hidden />}
            {mode === "map" ? <SquaresFour size={16} weight="duotone" className="relative" /> : <ListBullets size={16} weight="bold" className="relative" />}
          </button>
        ))}
      </div>
      <nav className="breadcrumb glass-css" aria-label={t("breadcrumb.label")}>
        <AnimatePresence initial={false} mode="popLayout">
          {trail.map((id, i) => (
            <motion.span
              key={id}
              layout
              className="flex items-center gap-1"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1, transition: { duration: duration.base, ease: ease.out } }}
              exit={{ opacity: 0, transition: { duration: duration.fast, ease: ease.inOut } }}
            >
              {i > 0 && <CaretRight size={11} weight="bold" className="text-faint" aria-hidden />}
              <button
                type="button"
                className={`crumb focus-ring ${i === trail.length - 1 ? "is-current" : ""}`}
                aria-current={i === trail.length - 1 ? "location" : undefined}
                onClick={() => setFocus(id)}
              >
                {disk.nodes[id].name}
              </button>
            </motion.span>
          ))}
        </AnimatePresence>
      </nav>
    </div>
  );
}
