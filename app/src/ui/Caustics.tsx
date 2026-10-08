import { motion } from "motion/react";
import { ease, mapMotion } from "../motion/tokens";

// Soft caustic light on the backdrop: very low contrast, drifting very slowly (transform only).
// Reduced motion keeps it static.
export function Caustics() {
  return (
    <motion.div
      className="caustics"
      aria-hidden
      animate={{ x: ["-2%", "2%", "-1%"], y: ["-1%", "1.5%", "-1%"], scale: [1, 1.04, 1] }}
      transition={{ duration: mapMotion.caustics, ease: ease.inOut, repeat: Infinity, repeatType: "mirror" }}
    />
  );
}
