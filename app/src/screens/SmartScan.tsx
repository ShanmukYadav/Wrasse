import { AnimatePresence, motion } from "motion/react";
import { useState } from "react";
import { Bytes } from "../components/Bytes";
import { t } from "../i18n";
import { ambient, duration, ease } from "../motion/tokens";
import { list, listItem, pressable } from "../motion/variants";
import { useApp } from "../store/app";

export function SmartScan() {
  const volume = useApp((s) => s.volume);
  const [pressed, setPressed] = useState(false);

  return (
    <motion.div
      variants={list}
      initial="initial"
      animate="animate"
      className="flex h-full flex-col items-center justify-center gap-20"
    >
      <motion.div variants={listItem} className="scan-stage">
        {Array.from({ length: ambient.ringCount }, (_, i) => (
          <motion.span
            key={i}
            className="scan-ring"
            aria-hidden
            initial={{ scale: 1, opacity: 0 }}
            animate={{ scale: [1, 1, ambient.ringScale], opacity: [0, 0.55, 0] }}
            transition={{
              duration: ambient.ringDuration,
              ease: ease.out,
              times: [0, 0.08, 1],
              repeat: Infinity,
              delay: (i * ambient.ringDuration) / ambient.ringCount,
            }}
          />
        ))}
        <motion.span
          className="scan-glow"
          aria-hidden
          animate={{ opacity: [0.55, 1, 0.55] }}
          transition={{ duration: ambient.breathe, ease: ease.inOut, repeat: Infinity }}
        />
        <motion.button
          type="button"
          className="scan-button focus-ring"
          onClick={() => setPressed(true)}
          {...pressable}
        >
          {t("home.scan")}
        </motion.button>
      </motion.div>

      <motion.div variants={listItem} className="flex flex-col items-center gap-1">
        {volume ? (
          <>
            <Bytes value={volume.available} className="text-hero" unitClassName="text-hero-unit" />
            <span className="text-label text-muted">
              {t("home.availableOf")} <Bytes value={volume.total} />
            </span>
          </>
        ) : (
          <span className="text-hero text-faint">—</span>
        )}
      </motion.div>

      <div className="h-5">
        <AnimatePresence>
          {pressed && (
            <motion.p
              className="text-caption text-muted"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1, transition: { duration: duration.base, ease: ease.out } }}
              exit={{ opacity: 0 }}
            >
              {t("home.scanArrives")}
            </motion.p>
          )}
        </AnimatePresence>
      </div>
    </motion.div>
  );
}
