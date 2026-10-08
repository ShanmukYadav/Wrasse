import type { Variants } from "motion/react";
import { duration, ease, spring, stagger } from "./tokens";

// Screen change: exit fade + 8px down, enter fade + 8px up (PLAN.md §7.3).
export const screen: Variants = {
  initial: { opacity: 0, y: 8 },
  animate: { opacity: 1, y: 0, transition: { duration: duration.base, ease: ease.out } },
  exit: { opacity: 0, y: 8, transition: { duration: duration.base, ease: ease.inOut } },
};

// Cards and list items: staggered fade + 12px rise.
export const list: Variants = {
  initial: {},
  animate: { transition: { staggerChildren: stagger } },
};

export const listItem: Variants = {
  initial: { opacity: 0, y: 12 },
  animate: { opacity: 1, y: 0, transition: { duration: duration.base, ease: ease.out } },
};

// Buttons: press scale 0.97, hover lift 1px.
export const pressable = {
  whileHover: { y: -1 },
  whileTap: { scale: 0.97 },
  transition: spring.snappy,
} as const;
