import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { cautionFor, strongest } from "../api";
import { Icon } from "../design/icons/Icon";
import { iconFor, toneFor } from "../design/icons/semantic";
import { t } from "../i18n";
import { formatBytes } from "../lib/format";
import { duration, ease, spring } from "../motion/tokens";
import { pressable } from "../motion/variants";
import { useApp } from "../store/app";

// The caution before deleting an OS folder (Documents, Desktop, …), a whole category, or data
// marked Keep. Lists what's affected and needs an explicit "I understand" before it goes ahead.
export function CautionSheet() {
  const ids = useApp((s) => s.confirmDelete);
  return <AnimatePresence>{ids && <Sheet key={ids.join(",")} ids={ids} />}</AnimatePresence>;
}

function Sheet({ ids }: { ids: number[] }) {
  const disk = useApp((s) => s.disk);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const platform = useApp((s) => s.info?.platform ?? "macOS");
  const { confirmDeleteRequest, cancelDelete } = useApp.getState();
  const [understood, setUnderstood] = useState(false);
  const kind = strongest(disk, ids);
  const items = ids.map((id) => disk.nodes[id]);
  const total = items.reduce((s, n) => s + n.size, 0);
  const fmt = (b: number) => {
    const f = formatBytes(b, units);
    return `${f.value} ${f.unit}`;
  };
  const first = items[0];
  const title = items.length === 1 ? t("caution.titleOne", { name: first.name }) : t("caution.titleMany", { count: items.length });
  const osFolders = items.filter((n) => cautionFor(disk, n.id).kind === "osFolder").map((n) => n.name);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && cancelDelete();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [cancelDelete]);

  return (
    <motion.div
      className="overlay-scrim"
      onClick={cancelDelete}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1, transition: { duration: duration.base, ease: ease.out } }}
      exit={{ opacity: 0, transition: { duration: duration.base, ease: ease.inOut } }}
    >
      <motion.div
        role="alertdialog"
        aria-modal="true"
        aria-label={title}
        className="overlay-panel caution glass-css"
        onClick={(e) => e.stopPropagation()}
        initial={{ opacity: 0, scale: 0.96 }}
        animate={{ opacity: 1, scale: 1, transition: spring.soft }}
        exit={{ opacity: 0, scale: 0.96, transition: { duration: duration.fast, ease: ease.inOut } }}
      >
        <div className="caution-head">
          <span className="caution-icon">
            <Icon name="warning" tone="warning" size={24} />
          </span>
          <h2 className="text-title">{title}</h2>
        </div>
        <p className="text-label">
          {kind === "osFolder"
            ? t("caution.osFolder", { names: osFolders.join(", "), platform })
            : t("caution.protected")}
        </p>
        <ul className="caution-list">
          {items.slice(0, 5).map((n) => (
            <li key={n.id} className="caution-item">
              <Icon name={iconFor(n)} tone={toneFor(n)} size={16} />
              <span className="caution-name">{n.name}</span>
              <span className="text-caption text-muted">{n.path}</span>
              <span className="caution-size tabular">{fmt(n.size)}</span>
            </li>
          ))}
          {items.length > 5 && <li className="text-caption text-muted">{t("caution.more", { count: items.length - 5 })}</li>}
        </ul>
        <p className="text-caption text-muted">{t("caution.trayNote")}</p>
        <label className="caution-check">
          <input type="checkbox" checked={understood} onChange={(e) => setUnderstood(e.target.checked)} />
          <span className="text-label">{t("caution.understand")}</span>
        </label>
        <div className="caution-actions">
          <button type="button" className="btn focus-ring" onClick={cancelDelete} autoFocus>
            {t("caution.cancel")}
          </button>
          <motion.button type="button" className="btn btn-danger focus-ring" disabled={!understood} onClick={confirmDeleteRequest} {...pressable}>
            <Icon name="trash" size={16} /> {t("caution.confirm", { size: fmt(total) })}
          </motion.button>
        </div>
      </motion.div>
    </motion.div>
  );
}
