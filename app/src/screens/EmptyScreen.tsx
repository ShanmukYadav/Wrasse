import { motion } from "motion/react";
import { Glass } from "../components/Glass";
import { SectionIcon } from "../components/SectionIcon";
import { t } from "../i18n";
import { list, listItem } from "../motion/variants";
import type { ScreenDef } from "./registry";

// Placeholder for screens whose features land in later phases.
export function EmptyScreen({ def }: { def: ScreenDef }) {
  return (
    <div className="flex h-full items-center justify-center px-10">
      <Glass
        variants={list}
        initial="initial"
        animate="animate"
        className="flex w-full max-w-md flex-col items-center gap-4 px-10 py-12 text-center"
      >
        <motion.div variants={listItem}>
          <SectionIcon def={def} size="lg" />
        </motion.div>
        <motion.h1 variants={listItem} className="text-display">
          {t(def.title)}
        </motion.h1>
        <motion.p variants={listItem} className="text-body text-muted">
          {t(def.body)}
        </motion.p>
        <motion.span variants={listItem} className="chip">
          {t("screen.arrivesIn", { phase: def.phase })}
        </motion.span>
      </Glass>
    </div>
  );
}
