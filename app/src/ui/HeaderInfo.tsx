import { motion } from "motion/react";
import { Bytes } from "../components/Bytes";
import { t } from "../i18n";
import { spring } from "../motion/tokens";
import { trayBytes, useApp, usedBytes } from "../store/app";

const R = 15;
const C = 2 * Math.PI * R;

export function HeaderInfo() {
  const disk = useApp((s) => s.disk);
  useApp((s) => s.diskVersion);
  const tray = useApp((s) => s.tray);
  const scanned = useApp((s) => s.scan === "done");
  const pending = trayBytes(tray);
  const free = Math.max(0, disk.capacity - usedBytes(disk) - pending);
  const usedFraction = 1 - free / disk.capacity;

  return (
    <div className="header-info" aria-live="polite">
      <div className="flex flex-col items-end">
        <span className="flex items-baseline gap-1.5">
          <Bytes value={scanned ? free : 0} className="text-hero" unitClassName="text-hero-unit" />
          <span className="text-label text-muted">{t("header.free")}</span>
        </span>
        <motion.span className="text-caption text-muted tabular" animate={{ opacity: pending > 0 ? 1 : 0 }}>
          + <Bytes value={pending} /> {t("header.inTray")}
        </motion.span>
      </div>
      <svg className="usage-ring" viewBox="0 0 38 38" role="img" aria-label={t("header.ring", { percent: Math.round(usedFraction * 100) })}>
        <circle cx="19" cy="19" r={R} className="ring-track" />
        <motion.circle
          cx="19"
          cy="19"
          r={R}
          className="ring-used"
          strokeDasharray={C}
          initial={{ strokeDashoffset: C }}
          animate={{ strokeDashoffset: C * (1 - (scanned ? usedFraction : 0)) }}
          transition={spring.gentle}
        />
      </svg>
    </div>
  );
}
