import { motion } from "motion/react";
import type { DiskNode } from "../../api";
import { t } from "../../i18n";
import { formatBytes, type UnitSystem } from "../../lib/format";
import { spring } from "../../motion/tokens";
import type { Cell } from "./gridLayout";

// Space held by snapshots: dashed, quiet, with a "Free now" action.
export function GhostTile({ node, cell, units, onSelect, onFree }: { node: DiskNode; cell: Cell; units: UnitSystem; onSelect: () => void; onFree: () => void }) {
  const f = formatBytes(node.size, units);
  return (
    <motion.div
      layout
      data-tile-id={node.id}
      className="ghost-tile"
      style={{ gridColumn: `${cell.col} / span ${cell.colSpan}`, gridRow: `${cell.row} / span ${cell.rowSpan}` }}
      transition={spring.soft}
    >
      <button type="button" className="ghost-text focus-ring" onClick={onSelect}>
        <span className="ghost-title">{t("home.ghostHeld", { size: `${f.value} ${f.unit}` })}</span>
        <span className="ghost-hint">{t("home.ghostBy")}</span>
      </button>
      <button type="button" className="link focus-ring" onClick={onFree}>
        {t("home.ghostFree")}
      </button>
    </motion.div>
  );
}
