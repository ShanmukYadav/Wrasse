import { Icon } from "../design/icons/Icon";
import { AnimatePresence, motion, useAnimate } from "motion/react";
import { useEffect, useRef } from "react";
import type { TrayItem } from "../api";
import { Bytes } from "../shared/Bytes";
import { t } from "../i18n";
import { formatBytes } from "../lib/format";
import { duration, ease, spring } from "../motion/tokens";
import { pressable } from "../motion/variants";
import { trayBytes, useApp } from "../store/app";
import { MiddleEllipsis } from "../shared/MiddleEllipsis";

const DAY = 86_400_000;

// The tray: a button that is always in the bottom-right corner ("Tray empty" or "N items · X GB",
// the landing spot for tiles that fly in) and, when clicked, a panel with every batch: when it was
// moved, its items, Restore per item and per batch, how long until it empties by itself, and
// "Empty now" (Not undoable).
export function QuarantineTray() {
  const tray = useApp((s) => s.tray);
  const open = useApp((s) => s.trayOpen);
  const flights = useApp((s) => s.flights.length);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const { setTrayOpen } = useApp.getState();
  const [scope, animate] = useAnimate();
  const prevFlights = useRef(flights);
  const rootRef = useRef<HTMLDivElement>(null);

  // The icon bounces once each time a flight lands.
  useEffect(() => {
    if (flights < prevFlights.current && scope.current) {
      void animate(scope.current, { scale: [1, 1.22, 1] }, spring.snappy);
    }
    prevFlights.current = flights;
  }, [flights, animate, scope]);

  // Esc or a click elsewhere closes the panel.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopImmediatePropagation();
        setTrayOpen(false);
      }
    };
    const onDown = (e: PointerEvent) => {
      const target = e.target as HTMLElement;
      // Sheets opened from the panel (Empty now's confirmation) keep it open.
      if (!rootRef.current?.contains(target) && !target.closest(".overlay-scrim")) setTrayOpen(false);
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("pointerdown", onDown, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("pointerdown", onDown, true);
    };
  }, [open, setTrayOpen]);

  const has = tray.length > 0;
  const size = formatBytes(trayBytes(tray), units);
  const summary = has ? `${t("tray.items", { count: tray.length })} · ${size.value} ${size.unit}` : t("tray.empty");
  return (
    <div className="tray-root" ref={rootRef}>
      <AnimatePresence>{open && <TrayPanel key="panel" />}</AnimatePresence>
      <motion.button
        type="button"
        className={`tray glass-css focus-ring ${has ? "has-items" : ""}`}
        aria-label={`${t("tray.label")}: ${summary}`}
        aria-expanded={open}
        title={open ? t("tray.close") : t("tray.open")}
        onClick={() => setTrayOpen(!open)}
        {...pressable}
      >
        <span ref={scope} id="tray-target" className="tray-icon">
          <Icon name="tray" tone={has ? "accent" : "neutral"} size={20} />
        </span>
        <span className="flex flex-col items-start">
          <span className="text-label text-strong">{has ? t("tray.items", { count: tray.length }) : t("tray.empty")}</span>
          <span className="text-caption text-muted tabular">{has ? <Bytes value={trayBytes(tray)} /> : t("tray.hint")}</span>
        </span>
        <Icon name="chevronRight" size={16} className={`tray-chevron ${open ? "is-open" : ""}`} />
      </motion.button>
    </div>
  );
}

function TrayPanel() {
  const tray = useApp((s) => s.tray);
  const keepDays = useApp((s) => s.keepDays);
  const { restoreEntries, requestEmptyTray, setOverlay, setTrayOpen } = useApp.getState();
  // Newest batch first; items of one batch together.
  const batches = new Map<string, TrayItem[]>();
  for (const i of [...tray].sort((a, b) => (b.at ?? 0) - (a.at ?? 0))) {
    const key = i.batch ?? `at-${i.at ?? 0}`;
    batches.set(key, [...(batches.get(key) ?? []), i]);
  }
  return (
    <motion.section
      className="tray-panel glass-css"
      role="dialog"
      aria-label={t("tray.title")}
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0, transition: { duration: duration.base, ease: ease.out } }}
      exit={{ opacity: 0, y: 8, transition: { duration: duration.fast, ease: ease.inOut } }}
    >
      <header className="tray-panel-head">
        <h2 className="text-title">{t("tray.title")}</h2>
        <button type="button" className="icon-btn focus-ring" aria-label={t("history.open")} title={t("history.open")} onClick={() => { setTrayOpen(false); setOverlay("history"); }}>
          <Icon name="history" size={16} />
        </button>
        <button type="button" className="icon-btn focus-ring" aria-label={t("tray.close")} title={t("tray.close")} onClick={() => setTrayOpen(false)}>
          <Icon name="close" size={16} />
        </button>
      </header>
      <p className="text-caption text-muted">{tray.length ? t("tray.keepNote", { days: keepDays }) : t("tray.emptyNote", { days: keepDays })}</p>
      {tray.length > 0 && (
        <>
          <div className="tray-batches">
            {[...batches].map(([key, items]) => (
              <BatchCard key={key} items={items} keepDays={keepDays} onRestore={restoreEntries} />
            ))}
          </div>
          <footer className="tray-panel-foot">
            <span className="chip chip-amber">{t("detail.notUndoable")}</span>
            <motion.button type="button" className="btn btn-danger focus-ring" onClick={requestEmptyTray} {...pressable}>
              <Icon name="trash" size={16} /> {t("tray.emptyNow")}
            </motion.button>
            <motion.button type="button" className="btn focus-ring" onClick={() => restoreEntries(tray)} {...pressable}>
              <Icon name="restore" size={16} /> {t("tray.restoreAll")}
            </motion.button>
          </footer>
        </>
      )}
    </motion.section>
  );
}

function BatchCard({ items, keepDays, onRestore }: { items: TrayItem[]; keepDays: number; onRestore: (items: TrayItem[]) => void }) {
  const at = items[0].at ?? Date.now();
  const left = Math.max(0, keepDays - Math.floor((Date.now() - at) / DAY));
  return (
    <section className="tray-batch">
      <div className="tray-batch-head">
        <span className="flex flex-col">
          <span className="text-label text-strong">{when(at)}</span>
          <span className="text-caption text-muted tabular">
            {t("tray.batchCount", { count: items.length })} · <Bytes value={trayBytes(items)} /> · {left === 0 ? t("tray.emptiesToday") : t("tray.daysLeft", { count: left })}
          </span>
        </span>
        {items.length > 1 && (
          <button type="button" className="link focus-ring" onClick={() => onRestore(items)}>
            {t("tray.restoreAll")}
          </button>
        )}
      </div>
      <ul className="tray-items">
        {items.map((i) => (
          <li key={i.entry ?? i.id} className="tray-item">
            <span className="tray-item-main">
              <span className="tray-item-name">{i.name}</span>
              {i.path && <MiddleEllipsis text={i.path} className="text-caption text-muted" />}
            </span>
            <Bytes value={i.size} className="tray-item-size" />
            <button type="button" className="link focus-ring" onClick={() => onRestore([i])}>
              {t("tray.restore")}
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

function when(at: number): string {
  const d = new Date(at);
  const time = d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  const today = new Date();
  if (d.toDateString() === today.toDateString()) return t("tray.today", { time });
  if (d.toDateString() === new Date(today.getTime() - DAY).toDateString()) return t("tray.yesterday", { time });
  return `${d.toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" })}, ${time}`;
}
