import { motion, type HTMLMotionProps } from "motion/react";

// Frosted-glass card floating over the section gradient.
export function Glass({ className = "", ...props }: HTMLMotionProps<"div">) {
  return <motion.div className={`glass ${className}`} {...props} />;
}
