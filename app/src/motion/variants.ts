import { spring } from "./tokens";

// Buttons: press scale 0.97, hover lift 1px.
export const pressable = {
  whileHover: { y: -1 },
  whileTap: { scale: 0.97 },
  transition: spring.snappy,
} as const;
