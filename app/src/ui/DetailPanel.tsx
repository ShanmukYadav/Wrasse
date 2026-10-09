import { AnimatePresence, motion } from "motion/react";
import { useEffect, type CSSProperties } from "react";
import { deletable, type DiskNode } from "../api";
import { Icon } from "../design/icons/Icon";
import { iconFor } from "../design/icons/semantic";
import { categories } from "../design/tokens";
import { t } from "../i18n";
import { formatBytes } from "../lib/format";
import { useScheme } from "../lib/prefs";
import { duration, ease, spring } from "../motion/tokens";
import { pressable } from "../motion/variants";
import { useApp } from "../store/app";
import { isDrillable } from "./home/nodes";

// Detail panel (D1): white glass, 330px, slides in from the right.
export function DetailPanel() {
  const open = useApp((s) => s.detailOpen);
  const selected = useApp((s) => s.selected);
  const closeDetail = useApp((s) => s.closeDetail);
  // Esc closes the panel (the command bar handles its own Esc).
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && (e.target as HTMLElement).tagName !== "INPUT" && !useApp.getState().overlay) {
        e.stopImmediatePropagation();
        closeDetail();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
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

/** The label chip: Duplicate · Your call, Safe, Your call, or Keep. */
function labelFor(n: DiskNode): { tone: "amber" | "green" | "protected"; text: string } {
  if (n.duplicateOf !== undefined) return { tone: "amber", text: t("detail.duplicateLabel") };
  if (n.safety === "safe") return { tone: "green", text: t("safety.safe") };
  if (n.safety === "yourCall") return { tone: "amber", text: t("safety.yourCall") };
  return { tone: "protected", text: t("safety.keep") };
}

function Detail({ id }: { id: number }) {
  const disk = useApp((s) => s.disk);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const fileManager = useApp((s) => s.info?.fileManager ?? "Finder");
  const scheme = useScheme();
  const { closeDetail, requestDeleteNow, freeGhost, notify, setFocus, openUninstall, requestDelete } = useApp.getState();
  const blocked = !deletable(disk, id);
  const n = disk.nodes[id];
  const ghost = n.kind === "ghost";
  const size = formatBytes(n.size, units);
  const sizeText = `${size.value} ${size.unit}`;
  const c = categories[scheme][n.category];
  const original = n.duplicateOf !== undefined ? disk.nodes[n.duplicateOf] : null;
  const label = labelFor(n);
  const lastUsed = ghost ? "—" : n.lastOpenedDays === 0 ? t("detail.today") : t("detail.daysAgo", { count: n.lastOpenedDays });
  const explanation = original ? t("detail.duplicateWhy", { name: original.name, path: original.path }) : n.why;

  return (
    <div className="detail" style={{ "--acc": c.accent, "--ink": c.ink } as CSSProperties}>
      <div className="detail-head">
        <span className="tile-badge" style={{ background: c.accent, boxShadow: `0 6px 14px ${c.shadow}` }}>
          <Icon name={iconFor(n)} tone="white" size={20} />
        </span>
        <h2 className="detail-name">{n.name}</h2>
        <button type="button" className="icon-btn focus-ring" aria-label={t("detail.close")} onClick={closeDetail}>
          <Icon name="close" size={16} />
        </button>
      </div>

      <div className="tile-size">
        <span className="tile-size-num detail-size">{size.value}</span>
        <span className="tile-size-unit">{size.unit}</span>
      </div>
      <div className="flex flex-wrap gap-2">
        <span className={`chip chip-${label.tone}`}>{label.text}</span>
        {n.note && <span className="chip chip-protected">{n.note}</span>}
      </div>

      <dl className="detail-grid">
        <dt>{t("detail.location")}</dt>
        <dd className="break-all">{n.path}</dd>
        <dt>{t("detail.lastUsed")}</dt>
        <dd>{lastUsed}</dd>
        <dt>{t("detail.category")}</dt>
        <dd>{t(`category.${n.category}`)}</dd>
      </dl>

      {n.ruleId ? (
        <div className="detail-rule">
          <span className="text-label text-strong">{t("detail.whySafe")}</span>
          <p className="detail-why">{explanation}</p>
          <span className="text-caption text-muted">{t("detail.rule", { id: n.ruleId })}</span>
        </div>
      ) : (
        <p className="detail-why">{explanation}</p>
      )}

      <div className="detail-actions">
        {ghost ? (
          <motion.button type="button" className="btn btn-primary btn-wide focus-ring" onClick={freeGhost} {...pressable}>
            {t("detail.freeNowSize", { size: sizeText })}
          </motion.button>
        ) : (
          <>
            {n.appBundle ? (
              <>
                {disk.nodes[n.parent].running && <p className="detail-note">{t("detail.runningNote", { name: disk.nodes[n.parent].name })}</p>}
                <motion.button type="button" className="btn btn-primary btn-wide focus-ring" onClick={() => openUninstall(n.parent)} {...pressable}>
                  <Icon name="trash" size={16} /> {t("detail.uninstall", { name: disk.nodes[n.parent].name })}
                </motion.button>
                <motion.button type="button" className="btn btn-wide focus-ring" onClick={() => requestDelete([n.id])} {...pressable}>
                  <Icon name="tray" size={16} /> {t("detail.removeAppOnly", { size: sizeText })}
                </motion.button>
              </>
            ) : n.leftover ? (
              <motion.button type="button" className="btn btn-primary btn-wide focus-ring" onClick={() => requestDelete([n.id])} {...pressable}>
                <Icon name="tray" size={16} /> {t("detail.remove", { size: sizeText })}
              </motion.button>
            ) : (
              !blocked && (
                <motion.button type="button" className="btn btn-primary btn-wide focus-ring" onClick={() => requestDelete([n.id])} {...pressable}>
                  <Icon name="trash" size={16} /> {t("delete.button", { size: sizeText })}
                </motion.button>
              )
            )}
            {n.safety === "safe" && (
              <motion.button type="button" className="btn btn-danger btn-wide focus-ring" onClick={() => requestDeleteNow([n.id])} {...pressable}>
                <Icon name="trash" size={16} /> {t("detail.deleteNow")} · {t("detail.notUndoable")}
              </motion.button>
            )}
            {isDrillable(n) && (
              <motion.button type="button" className="btn btn-wide focus-ring" onClick={() => setFocus(n.id)} {...pressable}>
                {t("detail.zoomIn")}
              </motion.button>
            )}
            <button type="button" className="link detail-link focus-ring" onClick={() => notify(t("detail.revealToast", { app: fileManager }))}>
              {t("detail.reveal", { app: fileManager })}
            </button>
            {blocked && <p className="text-caption text-muted">{t("detail.keepNote")}</p>}
          </>
        )}
      </div>
    </div>
  );
}
