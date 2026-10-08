import { Icon } from "../design/icons/Icon";
import type { IconName } from "../design/icons/nodes";
import { motion } from "motion/react";
import { t } from "../i18n";
import { pressable } from "../motion/variants";
import { useApp, type Overlay } from "../store/app";

const ITEMS: { id: Exclude<Overlay, null>; icon: IconName }[] = [
  { id: "startup", icon: "startup" },
  { id: "history", icon: "history" },
  { id: "audit", icon: "audit" },
  { id: "settings", icon: "settings" },
];

export function IconCluster() {
  const setOverlay = useApp((s) => s.setOverlay);
  return (
    <div className="icon-cluster glass-css">
      {ITEMS.map(({ id, icon }) => (
        <motion.button key={id} type="button" className="icon-btn focus-ring" aria-label={t(`overlay.${id}`)} title={t(`overlay.${id}`)} onClick={() => setOverlay(id)} {...pressable}>
          <Icon name={icon} size={20} />
        </motion.button>
      ))}
    </div>
  );
}
