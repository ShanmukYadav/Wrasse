import { AnimatePresence, motion } from "motion/react";
import { Bytes } from "../components/Bytes";
import { Icon } from "../design/icons/Icon";
import { t } from "../i18n";
import { duration, ease, spring } from "../motion/tokens";
import { pressable } from "../motion/variants";
import { useApp } from "../store/app";

// Replaces the lens bar while blocks are ticked: how many, how big, Delete (to the tray) or Clear.
export function SelectionBar() {
  const checked = useApp((s) => s.checked);
  const disk = useApp((s) => s.disk);
  const { requestDelete, clearChecks } = useApp.getState();
  const total = checked.reduce((s, id) => s + (disk.nodes[id]?.size ?? 0), 0);
  return (
    <AnimatePresence>
      {checked.length > 0 && (
        <motion.div
          key="selection"
          className="lens-bar selection-bar"
          role="toolbar"
          aria-label={t("select.label")}
          // x keeps the bar centered: motion owns transform, so the CSS translateX(-50%) is restated here.
          initial={{ opacity: 0, y: 8, x: "-50%" }}
          animate={{ opacity: 1, y: 0, x: "-50%", transition: spring.soft }}
          exit={{ opacity: 0, y: 8, x: "-50%", transition: { duration: duration.fast, ease: ease.inOut } }}
        >
          <span className="selection-count">
            <span className="text-label text-strong">{t("select.count", { count: checked.length })}</span>
            <Bytes value={total} className="text-caption text-muted" />
          </span>
          <motion.button type="button" className="btn btn-danger focus-ring" onClick={() => requestDelete(checked)} {...pressable}>
            <Icon name="trash" size={16} /> {t("select.delete")}
          </motion.button>
          <button type="button" className="btn focus-ring" onClick={clearChecks}>
            {t("select.clear")}
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
