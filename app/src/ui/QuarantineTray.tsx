import { Icon } from "../design/icons/Icon";
import { AnimatePresence, motion, useAnimate } from "motion/react";
import { useEffect, useRef } from "react";
import { Bytes } from "../components/Bytes";
import { t } from "../i18n";
import { duration, ease, spring } from "../motion/tokens";
import { pressable } from "../motion/variants";
import { trayBytes, useApp } from "../store/app";

export function QuarantineTray() {
  const tray = useApp((s) => s.tray);
  const flights = useApp((s) => s.flights.length);
  const { emptyTray, restoreTray } = useApp.getState();
  const [scope, animate] = useAnimate();
  const prevFlights = useRef(flights);

  // The icon bounces once each time a flight lands.
  useEffect(() => {
    if (flights < prevFlights.current && scope.current) {
      void animate(scope.current, { scale: [1, 1.22, 1] }, spring.snappy);
    }
    prevFlights.current = flights;
  }, [flights, animate, scope]);

  const has = tray.length > 0;
  // Hidden while empty (the D1 home has no tray); it rises in when the first item flies over.
  return (
    <AnimatePresence>
      {(has || flights > 0) && (
    <motion.section
      layout
      className="tray glass-css"
      aria-label={t("tray.label")}
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0, transition: spring.soft }}
      exit={{ opacity: 0, y: 12, transition: { duration: duration.base, ease: ease.inOut } }}
    >
      <span ref={scope} id="tray-target" className="tray-icon">
        <Icon name="tray" tone="accent" size={20} />
      </span>
      <div className="flex flex-col">
        <span className="text-label text-strong">{has ? t("tray.items", { count: tray.length }) : t("tray.empty")}</span>
        <span className="text-caption text-muted tabular">
          {has ? <Bytes value={trayBytes(tray)} /> : t("tray.hint")}
        </span>
      </div>
      <AnimatePresence>
        {has && (
          <motion.div
            className="flex gap-2"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1, transition: { duration: duration.base, ease: ease.out } }}
            exit={{ opacity: 0, transition: { duration: duration.fast } }}
          >
            <motion.button type="button" className="btn focus-ring" onClick={restoreTray} {...pressable}>
              {t("tray.restore")}
            </motion.button>
            <motion.button type="button" className="btn btn-primary focus-ring" onClick={emptyTray} {...pressable}>
              {t("tray.emptyNow")}
            </motion.button>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.section>
      )}
    </AnimatePresence>
  );
}
