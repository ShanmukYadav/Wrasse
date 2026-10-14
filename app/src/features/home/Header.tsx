import { AnimatePresence, motion } from "motion/react";
import { ancestors } from "../../api";
import { activeFilters } from "../browse/scope";
import { Bytes } from "../../shared/Bytes";
import { Icon } from "../../design/icons/Icon";
import { trafficLights } from "../../design/tokens";
import { t } from "../../i18n";
import { inTauri } from "../../api/env";
import { duration, ease } from "../../motion/tokens";
import { pressable } from "../../motion/variants";
import { trayBytes, useApp, usedBytes } from "../../store/app";
import { CommandBar } from "../command/CommandBar";

// D1 header: window controls, location, command bar, free space and Scan.
export function Header() {
  return (
    <header className="d1-header" data-tauri-drag-region>
      <div className="d1-header-left">
        {!inTauri && (
          <span className="traffic" aria-hidden>
            {trafficLights.map((c) => (
              <span key={c} style={{ background: c }} />
            ))}
          </span>
        )}
        {inTauri && <span className="traffic-space" aria-hidden />}
        <Location />
      </div>
      <CommandBar />
      <div className="d1-header-right">
        <FreeSpace />
        <HistoryButton />
        <ScanButton />
      </div>
    </header>
  );
}

function Location() {
  const disk = useApp((s) => s.disk);
  const focus = useApp((s) => s.focus);
  const filters = useApp((s) => s.filters);
  const { setFocus, zoomOut, toggleFilter } = useApp.getState();
  const listOpen = useApp((s) => s.viewMode === "list");
  const atRoot = focus === disk.root && !listOpen;
  const trail = [...ancestors(disk, focus), focus];
  const on = activeFilters(filters);
  return (
    <nav className="location" aria-label={t("breadcrumb.label")}>
      <AnimatePresence initial={false}>
        {!atRoot && (
          <motion.button
            type="button"
            className="back-btn focus-ring"
            aria-label={t("home.back")}
            onClick={zoomOut}
            initial={{ opacity: 0, x: -6 }}
            animate={{ opacity: 1, x: 0, transition: { duration: duration.base, ease: ease.out } }}
            exit={{ opacity: 0, x: -6, transition: { duration: duration.fast, ease: ease.inOut } }}
          >
            <Icon name="chevronLeft" size={20} />
          </motion.button>
        )}
      </AnimatePresence>
      {trail.map((id, i) => {
        const last = i === trail.length - 1;
        // Skip folders the view passes straight through (single-child chains).
        if (!last && i > 0 && disk.nodes[id].children.length === 1) return null;
        return (
          <span key={id} className="crumb-wrap">
            {i > 0 && <Icon name="chevronRight" size={16} className="crumb-sep" />}
            <button
              type="button"
              className={`crumb focus-ring ${last ? "is-current" : ""}`}
              aria-current={last ? "location" : undefined}
              onClick={() => setFocus(id)}
            >
              {i === 0 ? t("home.title") : disk.nodes[id].name}
            </button>
          </span>
        );
      })}
      {on.length > 0 && (
        <span className="crumb-filters">
          <span className="crumb-dot" aria-hidden>·</span>
          {on.map((id) => (
            <button
              key={id}
              type="button"
              className="crumb-filter focus-ring"
              aria-label={t("scope.removeFilter", { name: t(`lens.${id}`) })}
              onClick={() => toggleFilter(id)}
            >
              {t(`lens.${id}`)}
              <Icon name="close" size={16} />
            </button>
          ))}
        </span>
      )}
    </nav>
  );
}

function FreeSpace() {
  const disk = useApp((s) => s.disk);
  useApp((s) => s.diskVersion);
  const tray = useApp((s) => s.tray);
  const scanned = useApp((s) => s.scan === "done");
  const free = Math.max(0, disk.capacity - usedBytes(disk) - trayBytes(tray));
  return (
    // Not a live region: the number counts up, and a screen reader would read every step.
    <span className="free">
      <Bytes value={scanned ? free : 0} className="free-num" unitClassName="sr-only" />
      <span className="free-unit">{t("home.freeUnit")}</span>
    </span>
  );
}

/** Scan, or Stop while a scan runs. */
function ScanButton() {
  const scanning = useApp((s) => s.scan === "scanning" && s.scanProgress?.phase === "walking");
  const { startScan, cancelScan } = useApp.getState();
  return (
    <motion.button type="button" className="btn-scan focus-ring" onClick={scanning ? cancelScan : startScan} {...pressable}>
      {scanning ? t("home.stop") : t("home.scan")}
    </motion.button>
  );
}

/** Opens History (everything deleted). Shown once something has been deleted. */
function HistoryButton() {
  const count = useApp((s) => s.deleted.length);
  if (!count) return null;
  return (
    <button type="button" className="icon-btn header-history focus-ring" aria-label={t("history.open")} title={t("history.open")} onClick={() => useApp.getState().setOverlay("history")}>
      <Icon name="history" size={20} />
    </button>
  );
}
