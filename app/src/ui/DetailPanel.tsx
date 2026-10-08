import { Icon } from "../design/icons/Icon";
import { iconFor, toneFor } from "../design/icons/semantic";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { Bytes } from "../components/Bytes";
import { t } from "../i18n";
import { duration, ease, spring } from "../motion/tokens";
import { pressable } from "../motion/variants";
import { useApp } from "../store/app";
import { SafetyLabel } from "./Safety";

export function DetailPanel() {
  const open = useApp((s) => s.detailOpen);
  const selected = useApp((s) => s.selected);
  const closeDetail = useApp((s) => s.closeDetail);
  // Esc closes the panel (the command bar handles its own Esc).
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && (e.target as HTMLElement).tagName !== "INPUT" && !useApp.getState().overlay) closeDetail();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, closeDetail]);
  return (
    <AnimatePresence>
      {open && selected !== null && (
        <motion.aside
          key="detail"
          className="detail-panel glass-css"
          aria-label={t("detail.label")}
          initial={{ opacity: 0, x: 40 }}
          animate={{ opacity: 1, x: 0, transition: spring.soft }}
          exit={{ opacity: 0, x: 40, transition: { duration: duration.base, ease: ease.inOut } }}
        >
          <Detail id={selected} />
        </motion.aside>
      )}
    </AnimatePresence>
  );
}

function Detail({ id }: { id: number }) {
  const disk = useApp((s) => s.disk);
  const fileManager = useApp((s) => s.info?.fileManager ?? "Finder");
  const { closeDetail, addToTray, deleteNow, freeGhost, notify, setFocus } = useApp.getState();
  const [why, setWhy] = useState(false);
  const n = disk.nodes[id];
  const ghost = n.kind === "ghost";
  const opened = n.lastOpenedDays === 0 ? t("detail.today") : t("detail.daysAgo", { count: n.lastOpenedDays });

  return (
    <div className="flex h-full flex-col gap-4">
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <span className="item-icon"><Icon name={iconFor(n)} tone={toneFor(n)} size={32} /></span>
          <h2 className="text-title break-words">{n.name}</h2>
        </div>
        <button type="button" className="icon-btn focus-ring" aria-label={t("detail.close")} onClick={closeDetail}>
          <Icon name="close" size={16} />
        </button>
      </div>
      <Bytes value={n.size} className="text-hero" unitClassName="text-hero-unit" />
      <dl className="detail-grid">
        <dt>{t("detail.path")}</dt>
        <dd className="break-all">{n.path}</dd>
        <dt>{t("detail.category")}</dt>
        <dd>{t(`category.${n.category}`)}</dd>
        <dt>{t("detail.lastOpened")}</dt>
        <dd>{ghost ? "—" : opened}</dd>
        <dt>{t("detail.safety")}</dt>
        <dd>
          <SafetyLabel safety={n.safety} />
        </dd>
      </dl>

      <div>
        <button type="button" className="why-toggle focus-ring" aria-expanded={why} onClick={() => setWhy(!why)}>
          {n.safety === "safe" ? t("detail.whySafe") : t("detail.whyThis")}
          <motion.span animate={{ rotate: why ? 180 : 0 }} transition={spring.snappy} className="inline-flex">
            <Icon name="chevronDown" size={16} />
          </motion.span>
        </button>
        <AnimatePresence initial={false}>
          {why && (
            <motion.p
              className="text-label text-muted why-text"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1, transition: { duration: duration.base, ease: ease.out } }}
              exit={{ opacity: 0, transition: { duration: duration.fast } }}
            >
              {n.why}
            </motion.p>
          )}
        </AnimatePresence>
      </div>

      <div className="mt-auto flex flex-col gap-2">
        {ghost ? (
          <motion.button type="button" className="btn btn-primary focus-ring" onClick={freeGhost} {...pressable}>
            {t("detail.freeNow")}
          </motion.button>
        ) : (
          <>
            {n.children.length > 0 && (
              <motion.button type="button" className="btn focus-ring" onClick={() => setFocus(n.id)} {...pressable}>
                {t("detail.zoomIn")}
              </motion.button>
            )}
            <motion.button type="button" className="btn focus-ring" onClick={() => notify(t("detail.revealToast", { app: fileManager }))} {...pressable}>
              <Icon name="reveal" size={16} /> {t("detail.reveal", { app: fileManager })}
            </motion.button>
            <motion.button type="button" className="btn btn-primary focus-ring" disabled={n.safety === "keep"} onClick={() => addToTray([n.id])} {...pressable}>
              <Icon name="tray" size={16} /> {t("detail.addToCleanup")}
            </motion.button>
            {n.safety === "safe" && (
              <motion.button type="button" className="btn btn-danger focus-ring" onClick={() => deleteNow([n.id])} {...pressable}>
                <Icon name="trash" size={16} /> {t("detail.deleteNow")}
                <span className="text-caption">{t("detail.notUndoable")}</span>
              </motion.button>
            )}
            {n.safety === "keep" && <p className="text-caption text-muted">{t("detail.keepNote")}</p>}
          </>
        )}
      </div>
    </div>
  );
}
