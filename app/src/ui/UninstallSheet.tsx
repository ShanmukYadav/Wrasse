import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState, type CSSProperties } from "react";
import { Icon } from "../design/icons/Icon";
import { attached } from "../api";
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
  const quitting = useApp((s) => s.quitting === appId);
  const dryRun = useApp((s) => s.dryRun);
  const scheme = useScheme();
  const { close, quit, confirm } = { close: useApp.getState().closeUninstall, quit: useApp.getState().quitApp, confirm: useApp.getState().confirmUninstall };
  const app = disk.nodes[appId];
  const parts = app.children.map((c) => disk.nodes[c]).filter((c) => attached(disk, c.id) && c.size > 0);
  const bundle = parts.find((p) => p.appBundle);
  const leftovers = parts.filter((p) => !p.appBundle);
  // Leftovers matched by the app's name, or shared with another app, start unticked.
  const [checked, setChecked] = useState<Set<number>>(() => new Set(leftovers.filter((l) => !l.leftoverMeta?.defaultOff).map((l) => l.id)));
  const route = app.app?.route ?? { kind: "tray" as const };
  const admin = app.app?.adminLeftovers ?? [];
  const ticked = leftovers.filter((l) => checked.has(l.id)).map((l) => l.id);
  const total = (bundle?.size ?? 0) + ticked.reduce((s, id) => s + disk.nodes[id].size, 0);
  const fmt = (b: number) => {
    const f = formatBytes(b, units);
    return `${f.value} ${f.unit}`;
  };
  const c = categories[scheme][app.category];
  const removable = route.kind !== "notRemovable";
  const confirmLabel =
    route.kind === "homebrew"
      ? t("uninstall.confirmBrew", { size: fmt(total) })
      : route.kind === "finderTrash"
        ? t("uninstall.confirmFinder", { size: fmt(total) })
        : t("uninstall.confirm", { size: fmt(total) });

  return (
    <div className="sheet-body" style={{ "--acc": c.accent } as CSSProperties}>
      <div className="detail-head">
        <span className="tile-badge" style={{ background: c.accent, boxShadow: `0 6px 14px ${c.shadow}` }}>
          <Icon name="app" tone="white" size={20} />
        </span>
        <h2 className="detail-name">{t("uninstall.title", { name: app.name })}</h2>
        {route.kind === "homebrew" && <span className="chip chip-amber">{t("detail.notUndoable")}</span>}
      </div>
      <p className="detail-why">{t(`uninstall.route.${route.kind}`, { token: route.kind === "homebrew" ? route.token : "" })}</p>
      {removable && <p className="text-caption text-muted">{t("uninstall.subtitle")}</p>}

      {app.running && (
        <div className="sheet-warning" role="alert">
          <Icon name="warning" tone="warning" size={20} />
          <span>{quitting ? t("uninstall.quitting", { name: app.name }) : t("uninstall.running", { name: app.name })}</span>
          <button type="button" className="btn focus-ring" disabled={quitting} onClick={() => quit(appId)}>
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
              {bundle.name} <span className="text-muted">· {t(`uninstall.routeShort.${route.kind}`)}</span>
            </span>
            <span className="tile-row-size">{fmt(bundle.size)}</span>
          </div>
        )}
        {(leftovers.length > 0 || admin.length > 0) && <span className="sheet-label">{t("uninstall.leftovers")}</span>}
        {leftovers.map((l) => {
          const on = checked.has(l.id);
          const meta = l.leftoverMeta;
          const hint = meta?.sharedWith.length ? t("uninstall.shared", { names: meta.sharedWith.join(", ") }) : meta?.matchedBy === "name" ? t("uninstall.byName") : null;
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
                <span>
                  {l.name}
                  {hint && <span className="text-your-call"> · {hint}</span>}
                </span>
                <span className="sheet-path">{l.path}</span>
              </span>
              <span className="tile-row-size">{fmt(l.size)}</span>
            </label>
          );
        })}
        {admin.map((p) => (
          <div key={p} className="sheet-row is-disabled" title={t("uninstall.needsAdminNote")}>
            <span className="sheet-check is-off" aria-hidden />
            <Icon name="keep" tone="neutral" size={16} />
            <span className="sheet-name">
              <span>
                {p.split("/").pop()} <span className="text-muted">· {t("uninstall.needsAdmin")}</span>
              </span>
              <span className="sheet-path">{p}</span>
            </span>
          </div>
        ))}
        {admin.length > 0 && <p className="text-caption text-muted">{t("uninstall.needsAdminNote")}</p>}
      </div>

      {dryRun && <p className="text-caption text-muted">{t("confirm.dryRunNote")}</p>}
      <div className="sheet-actions">
        <button type="button" className="btn focus-ring" onClick={close}>
          {t("uninstall.cancel")}
        </button>
        {removable && (
          <motion.button type="button" className={`btn ${route.kind === "homebrew" ? "btn-danger" : "btn-primary"} focus-ring`} disabled={app.running || quitting} onClick={() => confirm(appId, ticked)} {...pressable}>
            {confirmLabel}
          </motion.button>
        )}
      </div>
    </div>
  );
}
