import { AnimatePresence, motion } from "motion/react";
import { useEffect, type ReactNode } from "react";
import { api } from "../../api";
import { BrandMark } from "../../design/brand/BrandMark";
import { Icon } from "../../design/icons/Icon";
import type { IconName } from "../../design/icons/nodes";
import { t } from "../../i18n";
import { duration, ease, spring } from "../../motion/tokens";
import { pressable } from "../../motion/variants";
import { useApp } from "../../store/app";

// First run (PLAN.md §6.17): what Wrasse does, no telemetry, Full Disk Access, then the scan.
// The access status is the real check from the core; it re-checks when the window regains focus,
// so returning from System Settings updates it.
export function Onboarding() {
  const open = useApp((s) => s.overlay === "onboarding");
  const access = useApp((s) => s.fullDiskAccess);
  const readOnly = useApp((s) => s.info?.readOnly);
  const { checkFullDiskAccess, finishOnboarding } = useApp.getState();

  useEffect(() => {
    if (!open) return;
    void checkFullDiskAccess();
    const onFocus = () => void checkFullDiskAccess();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [open, checkFullDiskAccess]);

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          key="onboarding"
          className="overlay-scrim"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1, transition: { duration: duration.base, ease: ease.out } }}
          exit={{ opacity: 0, transition: { duration: duration.base, ease: ease.inOut } }}
        >
          <motion.div
            role="dialog"
            aria-modal="true"
            aria-label={t("onboarding.label")}
            className="overlay-panel onboarding glass-css"
            initial={{ opacity: 0, scale: 0.96 }}
            animate={{ opacity: 1, scale: 1, transition: spring.soft }}
            exit={{ opacity: 0, scale: 0.96, transition: { duration: duration.fast, ease: ease.inOut } }}
          >
            <div className="onboarding-head">
              <BrandMark size={56} />
              <h2 className="text-title">{t("onboarding.title")}</h2>
              <p className="text-label text-muted">{t("onboarding.lead")}</p>
            </div>
            <ul className="onboarding-steps">
              <Step icon="grid" title={t("onboarding.mapTitle")} body={t("onboarding.mapBody")} />
              <Step icon="safe" title={t("onboarding.privateTitle")} body={t("onboarding.privateBody")} />
              <Step
                icon="keep"
                title={t("onboarding.accessTitle")}
                body={t("onboarding.accessBody")}
                status={
                  <span className={`chip ${access === null ? "chip-protected" : access ? "chip-green" : "chip-amber"}`}>
                    {access === null ? t("onboarding.checking") : access ? t("onboarding.granted") : t("onboarding.notGranted")}
                  </span>
                }
              >
                {access === false && (
                  <div className="onboarding-actions">
                    <button type="button" className="btn focus-ring" onClick={() => void api.openFullDiskAccessSettings()}>
                      {t("onboarding.openSettings")}
                    </button>
                    <button type="button" className="link focus-ring" onClick={() => void checkFullDiskAccess()}>
                      {t("onboarding.checkAgain")}
                    </button>
                  </div>
                )}
              </Step>
            </ul>
            {readOnly && <p className="text-caption text-muted">{t("onboarding.readOnly")}</p>}
            <motion.button type="button" className="btn btn-primary btn-wide focus-ring" onClick={finishOnboarding} {...pressable}>
              <Icon name="scan" size={16} /> {t("onboarding.start")}
            </motion.button>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Step({ icon, title, body, status, children }: { icon: IconName; title: string; body: string; status?: ReactNode; children?: ReactNode }) {
  return (
    <li className="onboarding-step">
      <span className="onboarding-icon">
        <Icon name={icon} tone="accent" size={20} />
      </span>
      <div className="onboarding-text">
        <div className="onboarding-step-head">
          <span className="text-label text-strong">{title}</span>
          {status}
        </div>
        <span className="text-caption text-muted">{body}</span>
        {children}
      </div>
    </li>
  );
}
