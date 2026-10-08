import { AnimatePresence, motion } from "motion/react";
import { ancestors } from "../../api";
import { Bytes } from "../../components/Bytes";
import { Icon } from "../../design/icons/Icon";
import { trafficLights } from "../../design/tokens";
import { t } from "../../i18n";
import { inTauri } from "../../lib/backend";
import { duration, ease } from "../../motion/tokens";
import { pressable } from "../../motion/variants";
import { trayBytes, useApp, usedBytes } from "../../store/app";
import { CommandBar } from "../CommandBar";

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
        <motion.button type="button" className="btn-scan focus-ring" onClick={() => useApp.getState().startScan()} {...pressable}>
          {t("home.scan")}
        </motion.button>
      </div>
    </header>
  );
}

function Location() {
  const disk = useApp((s) => s.disk);
  const focus = useApp((s) => s.focus);
  const { setFocus, zoomOut } = useApp.getState();
  const atRoot = focus === disk.root;
  const trail = [...ancestors(disk, focus), focus];
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
    <span className="free" aria-live="polite">
      <Bytes value={scanned ? free : 0} className="free-num" unitClassName="sr-only" />
      <span className="free-unit">{t("home.freeUnit")}</span>
    </span>
  );
}
