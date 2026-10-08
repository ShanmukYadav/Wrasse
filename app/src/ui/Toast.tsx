import { AnimatePresence, motion } from "motion/react";
import { useEffect } from "react";
import { duration, ease } from "../motion/tokens";
import { useApp } from "../store/app";

export function Toast() {
  const toast = useApp((s) => s.toast);
  useEffect(() => {
    if (!toast) return;
    const id = setTimeout(() => useApp.setState({ toast: null }), 2600);
    return () => clearTimeout(id);
  }, [toast]);
  return (
    <div className="toast-anchor" role="status" aria-live="polite">
      <AnimatePresence>
        {toast && (
          <motion.div
            key={toast.key}
            className="toast glass-css"
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0, transition: { duration: duration.base, ease: ease.out } }}
            exit={{ opacity: 0, y: 8, transition: { duration: duration.fast, ease: ease.inOut } }}
          >
            {toast.text}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
