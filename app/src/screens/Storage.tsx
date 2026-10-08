import { motion } from "motion/react";
import { Bytes } from "../components/Bytes";
import { Glass } from "../components/Glass";
import { SectionIcon } from "../components/SectionIcon";
import { t } from "../i18n";
import { spring } from "../motion/tokens";
import { list, listItem } from "../motion/variants";
import { useApp } from "../store/app";
import { screenById } from "./registry";

// Treemap-shaped placeholder tiles: [column, row, colSpan, rowSpan] on a 6×4 grid.
const placeholderTiles = [
  [1, 1, 3, 4],
  [4, 1, 3, 2],
  [4, 3, 2, 2],
  [6, 3, 1, 1],
  [6, 4, 1, 1],
] as const;

export function Storage() {
  const def = screenById("storage");
  const volume = useApp((s) => s.volume);
  const used = volume ? volume.total - volume.available : 0;
  const fraction = volume && volume.total > 0 ? used / volume.total : 0;

  return (
    <motion.div variants={list} initial="initial" animate="animate" className="screen-pad flex h-full flex-col gap-6">
      <motion.header variants={listItem} className="flex items-center gap-4">
        <SectionIcon def={def} size="lg" />
        <h1 className="text-display">{t(def.title)}</h1>
      </motion.header>

      <Glass variants={listItem} className="flex flex-col gap-5 p-7">
        <div className="flex items-end justify-between gap-6">
          <div className="flex flex-col">
            <span className="text-label text-muted">{t("storage.systemDisk")}</span>
            {volume ? (
              <Bytes value={used} className="text-hero" unitClassName="text-hero-unit" />
            ) : (
              <span className="text-hero text-faint">—</span>
            )}
            <span className="text-label text-muted">{t("storage.used")}</span>
          </div>
          {volume && (
            <div className="flex flex-col items-end text-label text-muted">
              <span>
                <Bytes value={volume.available} className="text-strong" /> {t("storage.available")}
              </span>
              <span>
                <Bytes value={volume.total} /> {t("storage.total")}
              </span>
            </div>
          )}
        </div>
        <div className="usage-track" role="meter" aria-valuenow={Math.round(fraction * 100)} aria-valuemin={0} aria-valuemax={100} aria-label={t("storage.used")}>
          <motion.div
            className="usage-fill"
            initial={{ scaleX: 0 }}
            animate={{ scaleX: fraction }}
            transition={spring.gentle}
          />
        </div>
      </Glass>

      <Glass variants={listItem} className="relative flex-1 overflow-hidden p-4">
        <div className="placeholder-grid">
          {placeholderTiles.map(([col, row, w, h], i) => (
            <span
              key={i}
              className="placeholder-tile"
              style={{ gridColumn: `${col} / span ${w}`, gridRow: `${row} / span ${h}` }}
            />
          ))}
        </div>
        <span className="chip absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2">
          {t("storage.breakdownArrives")}
        </span>
      </Glass>
    </motion.div>
  );
}
