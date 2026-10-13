import { AnimatePresence, motion } from "motion/react";
import { useEffect } from "react";
import { duration, ease } from "../motion/tokens";
import { useApp } from "../store/app";

export function Toast() {
  const toast = useApp((s) => s.toast);
  useEffect(() => {
    if (!toast) return;
    // A toast with a button (Undo) stays long enough to reach it.
    const id = setTimeout(() => useApp.setState({ toast: null }), toast.action ? 7000 : 2600);
    return () => clearTimeout(id);
  }, [toast]);
  return (
    <div className="toast-anchor" role="status" aria-live="polite">
      <AnimatePresence>
        {toast && (
          <motion.div
            key={toast.key}
            className={`toast glass-css ${toast.action ? "has-action" : ""}`}
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0, transition: { duration: duration.base, ease: ease.out } }}
            exit={{ opacity: 0, y: 8, transition: { duration: duration.fast, ease: ease.inOut } }}
          >
            <span>{toast.text}</span>
            {toast.action && (
              <button
                type="button"
                className="toast-action focus-ring"
                onClick={() => {
                  toast.action?.run();
                  useApp.setState({ toast: null });
                }}
              >
                {toast.action.label}
              </button>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
