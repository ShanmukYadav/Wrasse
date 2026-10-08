import { AnimatePresence, motion } from "motion/react";
import { screen as screenVariants } from "../motion/variants";
import { EmptyScreen } from "../screens/EmptyScreen";
import { screenById } from "../screens/registry";
import { useApp } from "../store/app";

export function ScreenHost() {
  const id = useApp((s) => s.screen);
  const def = screenById(id);
  return (
    <main className="content relative flex-1 overflow-hidden">
      <div data-tauri-drag-region className="absolute inset-x-0 top-0 z-10 h-10" />
      <AnimatePresence mode="wait" initial={false}>
        <motion.section
          key={id}
          variants={screenVariants}
          initial="initial"
          animate="animate"
          exit="exit"
          className="h-full"
          aria-label={def.id}
        >
          <EmptyScreen def={def} />
        </motion.section>
      </AnimatePresence>
    </main>
  );
}
