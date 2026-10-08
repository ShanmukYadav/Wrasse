import { AnimatePresence, motion } from "motion/react";
import { t } from "../i18n";
import { screen as screenVariants } from "../motion/variants";
import { EmptyScreen } from "../screens/EmptyScreen";
import { screenById, type ScreenId } from "../screens/registry";
import { SmartScan } from "../screens/SmartScan";
import { Storage } from "../screens/Storage";
import { useApp } from "../store/app";

const screens: Partial<Record<ScreenId, () => React.JSX.Element>> = {
  home: SmartScan,
  storage: Storage,
};

export function ScreenHost() {
  const id = useApp((s) => s.screen);
  const def = screenById(id);
  const Screen = screens[id];
  return (
    <main className="relative flex-1 overflow-hidden">
      <div data-tauri-drag-region className="absolute inset-x-0 top-0 z-10 h-10" />
      <AnimatePresence mode="wait">
        <motion.section
          key={id}
          variants={screenVariants}
          initial="initial"
          animate="animate"
          exit="exit"
          className="h-full"
          aria-label={t(def.title)}
        >
          {Screen ? <Screen /> : <EmptyScreen def={def} />}
        </motion.section>
      </AnimatePresence>
    </main>
  );
}
