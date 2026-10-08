import { motion } from "motion/react";
import { t } from "../i18n";
import { list, listItem } from "../motion/variants";
import type { ScreenDef } from "./registry";

// Placeholder for screens whose features land in later phases.
export function EmptyScreen({ def }: { def: ScreenDef }) {
  const Icon = def.icon;
  return (
    <motion.div
      variants={list}
      initial="initial"
      animate="animate"
      className="flex h-full flex-col items-center justify-center gap-4 px-10 text-center"
    >
      <motion.div
        variants={listItem}
        className="grid size-16 place-items-center rounded-[var(--radius)] bg-[var(--accent-soft)] text-[var(--accent)]"
      >
        <Icon size={30} strokeWidth={1.75} aria-hidden />
      </motion.div>
      <motion.h1 variants={listItem} className="text-2xl font-semibold tracking-tight">
        {t(def.title)}
      </motion.h1>
      <motion.p variants={listItem} className="max-w-md text-[15px] text-[var(--text-muted)]">
        {t(def.body)}
      </motion.p>
      <motion.span
        variants={listItem}
        className="rounded-full border border-[var(--border)] px-3 py-1 text-xs text-[var(--text-muted)]"
      >
        {t("screen.comingSoon")}
      </motion.span>
    </motion.div>
  );
}
