import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState, type CSSProperties } from "react";
import { attached } from "../mock/tree";
import { Icon } from "../design/icons/Icon";
import { iconFor } from "../design/icons/semantic";
import { categories } from "../design/tokens";
import { t } from "../i18n";
import { formatBytes } from "../lib/format";
import { useScheme } from "../lib/prefs";
import { duration, ease, spring } from "../motion/tokens";
import { pressable } from "../motion/variants";
import { useApp } from "../store/app";

// Uninstall confirm sheet: the app bundle plus a checklist of leftovers, all moved to quarantine.
export function UninstallSheet() {
  const appId = useApp((s) => s.uninstall);
  const close = useApp((s) => s.closeUninstall);
  useEffect(() => {
    if (appId === null) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopImmediatePropagation();
        close();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [appId, close]);
  return (
    <AnimatePresence>
      {appId !== null && (
        <motion.div
          key="scrim"
          className="overlay-scrim"
          onClick={close}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1, transition: { duration: duration.base, ease: ease.out } }}
          exit={{ opacity: 0, transition: { duration: duration.base, ease: ease.inOut } }}
        >
          <motion.div
            role="dialog"
            aria-modal="true"
            className="overlay-panel glass-css sheet"
            onClick={(e) => e.stopPropagation()}
            initial={{ opacity: 0, scale: 0.96 }}
            animate={{ opacity: 1, scale: 1, transition: spring.soft }}
            exit={{ opacity: 0, scale: 0.96, transition: { duration: duration.fast, ease: ease.inOut } }}
          >
            <Sheet appId={appId} />
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Sheet({ appId }: { appId: number }) {
  const disk = useApp((s) => s.disk);
  useApp((s) => s.diskVersion);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const scheme = useScheme();
  const { close, quit, confirm } = { close: useApp.getState().closeUninstall, quit: useApp.getState().quitApp, confirm: useApp.getState().confirmUninstall };
  const app = disk.nodes[appId];
  const parts = app.children.map((c) => disk.nodes[c]).filter((c) => attached(disk, c.id) && c.size > 0);
  const bundle = parts.find((p) => p.appBundle);
  const leftovers = parts.filter((p) => !p.appBundle);
  const [checked, setChecked] = useState<Set<number>>(() => new Set(leftovers.map((l) => l.id)));
  const ids = [...(bundle ? [bundle.id] : []), ...leftovers.filter((l) => checked.has(l.id)).map((l) => l.id)];
  const total = ids.reduce((s, id) => s + disk.nodes[id].size, 0);
  const fmt = (b: number) => {
    const f = formatBytes(b, units);
    return `${f.value} ${f.unit}`;
  };
  const c = categories[scheme][app.category];

  return (
    <div className="sheet-body" style={{ "--acc": c.accent } as CSSProperties}>
      <div className="detail-head">
        <span className="tile-badge" style={{ background: c.accent, boxShadow: `0 6px 14px ${c.shadow}` }}>
          <Icon name="app" tone="white" size={20} />
        </span>
        <h2 className="detail-name">{t("uninstall.title", { name: app.name })}</h2>
      </div>
      <p className="detail-why">{t("uninstall.subtitle")}</p>

      {app.running && (
        <div className="sheet-warning" role="alert">
          <Icon name="warning" tone="warning" size={20} />
          <span>{t("uninstall.running", { name: app.name })}</span>
          <button type="button" className="btn focus-ring" onClick={() => quit(appId)}>
            {t("uninstall.quit", { name: app.name })}
          </button>
        </div>
      )}

      <div className="sheet-list">
        {bundle && (
          <div className="sheet-row">
            <span className="sheet-check is-locked" aria-hidden>
              <Icon name="check" tone="white" size={16} />
            </span>
            <Icon name="app" tone={app.category} size={16} />
            <span className="sheet-name">
              {bundle.name} <span className="text-muted">· {t("uninstall.app")}</span>
            </span>
            <span className="tile-row-size">{fmt(bundle.size)}</span>
          </div>
        )}
        <span className="sheet-label">{t("uninstall.leftovers")}</span>
        {leftovers.map((l) => {
          const on = checked.has(l.id);
          return (
            <label key={l.id} className="sheet-row">
              <input
                type="checkbox"
                className="sr-only"
                checked={on}
                onChange={() => {
                  const next = new Set(checked);
                  if (on) next.delete(l.id);
                  else next.add(l.id);
                  setChecked(next);
                }}
              />
              <span className={`sheet-check ${on ? "is-on" : ""}`} aria-hidden>
                {on && <Icon name="check" tone="white" size={16} />}
              </span>
              <Icon name={iconFor(l)} tone={app.category} size={16} />
              <span className="sheet-name" title={l.path}>
                {l.name}
                <span className="sheet-path">{l.path}</span>
              </span>
              <span className="tile-row-size">{fmt(l.size)}</span>
            </label>
          );
        })}
      </div>

      <div className="sheet-actions">
        <button type="button" className="btn focus-ring" onClick={close}>
          {t("uninstall.cancel")}
        </button>
        <motion.button type="button" className="btn btn-primary focus-ring" disabled={app.running || ids.length === 0} onClick={() => confirm(appId, ids)} {...pressable}>
          {t("uninstall.confirm", { size: fmt(total) })}
        </motion.button>
      </div>
    </div>
  );
}
