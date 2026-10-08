import { ClockCounterClockwise, Gear, Power, Scroll } from "@phosphor-icons/react";
import { motion } from "motion/react";
import { t } from "../i18n";
import { pressable } from "../motion/variants";
import { useApp, type Overlay } from "../store/app";

const ITEMS: { id: Exclude<Overlay, null>; icon: typeof Gear }[] = [
  { id: "startup", icon: Power },
  { id: "history", icon: ClockCounterClockwise },
  { id: "audit", icon: Scroll },
  { id: "settings", icon: Gear },
];

export function IconCluster() {
  const setOverlay = useApp((s) => s.setOverlay);
  return (
    <div className="icon-cluster glass-css">
      {ITEMS.map(({ id, icon: Icon }) => (
        <motion.button key={id} type="button" className="icon-btn focus-ring" aria-label={t(`overlay.${id}`)} title={t(`overlay.${id}`)} onClick={() => setOverlay(id)} {...pressable}>
          <Icon size={18} weight="duotone" />
        </motion.button>
      ))}
    </div>
  );
}
