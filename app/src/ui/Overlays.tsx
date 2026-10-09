import { Icon } from "../design/icons/Icon";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { Bytes } from "../components/Bytes";
import { t, type MessageKey } from "../i18n";
import { duration, ease, spring } from "../motion/tokens";
import type { DiskNode } from "../api";
import { useApp, type DeletedEntry, type Overlay } from "../store/app";
import { iconFor } from "../design/icons/semantic";
import { categories } from "../design/tokens";
import { useScheme } from "../lib/prefs";
import { MiddleEllipsis } from "./home/MiddleEllipsis";

export function Overlays() {
  const overlay = useApp((s) => s.overlay);
  const setOverlay = useApp((s) => s.setOverlay);
  useEffect(() => {
    if (!overlay) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOverlay(null);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [overlay, setOverlay]);

  return (
    <AnimatePresence>
      {overlay && overlay !== "design" && overlay !== "onboarding" && (
        <motion.div
          key="scrim"
          className="overlay-scrim"
          onClick={() => setOverlay(null)}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1, transition: { duration: duration.base, ease: ease.out } }}
          exit={{ opacity: 0, transition: { duration: duration.base, ease: ease.inOut } }}
        >
          <motion.div
            role="dialog"
            aria-modal="true"
            aria-label={t(`overlay.${overlay}`)}
            className={`overlay-panel glass-css ${overlay === "history" ? "is-wide" : ""}`}
            onClick={(e) => e.stopPropagation()}
            initial={{ opacity: 0, scale: 0.96 }}
            animate={{ opacity: 1, scale: 1, transition: spring.soft }}
            exit={{ opacity: 0, scale: 0.96, transition: { duration: duration.fast, ease: ease.inOut } }}
          >
            <Panel overlay={overlay} />
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Panel({ overlay }: { overlay: Exclude<Overlay, null | "design" | "onboarding"> }) {
  const setOverlay = useApp((s) => s.setOverlay);
  const closeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => closeRef.current?.focus(), []);
  return (
    <div className="flex min-h-0 max-h-full flex-1 flex-col gap-4">
      <div className="flex items-center justify-between">
        <h2 className="text-title">{t(`overlay.${overlay}`)}</h2>
        <button ref={closeRef} type="button" className="icon-btn focus-ring" aria-label={t("detail.close")} onClick={() => setOverlay(null)}>
          <Icon name="close" size={16} />
        </button>
      </div>
      <div className="overlay-body">
        {overlay === "startup" && <Startup />}
        {overlay === "settings" && <Settings />}
        {overlay === "audit" && <Audit />}
        {overlay === "history" && <History />}
      </div>
    </div>
  );
}

const STARTUP = [
  ["Docker Desktop", "com.docker.helper", true, true],
  ["Spotify", "com.spotify.client.startuphelper", true, true],
  ["Google Updater", "com.google.GoogleUpdater.wake", true, true],
  ["Zoom", "us.zoom.updater.login.check", true, false],
  ["Ollama", "com.ollama.ollama", true, true],
  ["Raycast", "com.raycast.macos.launcher", true, true],
  ["Unknown helper", "com.example.helper", false, true],
] as const;

function Startup() {
  const [enabled, setEnabled] = useState<boolean[]>(STARTUP.map((s) => s[3]));
  return (
    <ul className="flex flex-col gap-1">
      {STARTUP.map(([name, id, signed], i) => (
        <li key={id} className="row">
          <div className="flex flex-col">
            <span className="text-label text-strong">{name}</span>
            <span className="text-caption text-muted">
              {id} · {signed ? t("startup.signed") : t("startup.unsigned")}
            </span>
          </div>
          <Switch checked={enabled[i]} label={name} onChange={(v) => setEnabled(enabled.map((e, j) => (j === i ? v : e)))} />
        </li>
      ))}
      <p className="text-caption text-muted">{t("startup.mock")}</p>
    </ul>
  );
}

function Switch({ checked, label, onChange }: { checked: boolean; label: string; onChange: (v: boolean) => void }) {
  return (
    <button type="button" role="switch" aria-checked={checked} aria-label={label} className={`switch focus-ring ${checked ? "is-on" : ""}`} onClick={() => onChange(!checked)}>
      <motion.span className="switch-knob" animate={{ x: checked ? 16 : 0 }} transition={spring.snappy} />
    </button>
  );
}

function Segmented<T extends string>({ value, options, onChange, label }: { value: T; options: [T, MessageKey][]; onChange: (v: T) => void; label: string }) {
  return (
    <div className="segmented" role="radiogroup" aria-label={label}>
      {options.map(([v, key]) => (
        <button key={v} type="button" role="radio" aria-checked={value === v} className="segment focus-ring" onClick={() => onChange(v)}>
          {value === v && <motion.span layoutId={`seg-${label}`} className="segment-indicator" transition={spring.soft} aria-hidden />}
          <span className="relative">{t(key)}</span>
        </button>
      ))}
    </div>
  );
}

function Settings() {
  const s = useApp();
  return (
    <div className="flex flex-col gap-4">
      <Setting title={t("settings.theme")}>
        <Segmented label="theme" value={s.theme} onChange={(theme) => s.setPrefs({ theme })} options={[["system", "settings.system"], ["dark", "settings.night"], ["light", "settings.day"]]} />
      </Setting>
      <Setting title={t("settings.motion")}>
        <Segmented label="motion" value={s.motion} onChange={(motion) => s.setPrefs({ motion })} options={[["system", "settings.system"], ["reduce", "settings.reduce"], ["full", "settings.full"]]} />
      </Setting>
    </div>
  );
}

function Setting({ title, note, children }: { title: string; note?: string; children: React.ReactNode }) {
  return (
    <div className="row">
      <div className="flex flex-col">
        <span className="text-label text-strong">{title}</span>
        {note && <span className="text-caption text-muted">{note}</span>}
      </div>
      {children}
    </div>
  );
}

function Audit() {
  const audit = useApp((s) => s.audit);
  return audit.length === 0 ? (
    <p className="text-label text-muted">{t("audit.empty")}</p>
  ) : (
    <ul className="flex flex-col gap-1">
      {audit.map((a, i) => (
        <li key={i} className="row">
          <div className="flex min-w-0 flex-col">
            <span className="text-label text-strong">{a.action}</span>
            <span className="text-caption text-muted truncate">{a.target}</span>
          </div>
          <span className="text-caption text-muted tabular whitespace-nowrap">
            <Bytes value={a.size} /> · {new Date(a.at).toLocaleTimeString()}
          </span>
        </li>
      ))}
    </ul>
  );
}

type HistoryFilter = "all" | "inTray" | "deleted" | "restored";

/** Everything deleted this session: what, where, how big, when, and whether it can come back. */
function History() {
  const entries = useApp((s) => s.deleted);
  const disk = useApp((s) => s.disk);
  const scheme = useScheme();
  const restoreItem = useApp((s) => s.restoreItem);
  const [filter, setFilter] = useState<HistoryFilter>("all");
  if (entries.length === 0) return <p className="text-label text-muted">{t("history.empty")}</p>;
  const shown = filter === "all" ? entries : entries.filter((e) => e.status === filter);
  const freed = entries.filter((e) => e.status !== "restored").reduce((s, e) => s + e.size, 0);
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <span className="text-label text-muted">
          {t("history.summary", { count: entries.length })} · <Bytes value={freed} /> {t("history.freed")}
        </span>
        <Segmented<HistoryFilter>
          label="history-filter"
          value={filter}
          onChange={setFilter}
          options={[["all", "history.all"], ["inTray", "history.inTray"], ["deleted", "history.deleted"], ["restored", "history.restored"]]}
        />
      </div>
      <ul className="history-list">
        {shown.map((e) => (
          <HistoryRow key={`${e.id}-${e.at}`} entry={e} node={disk.nodes[e.id]} scheme={scheme} onRestore={() => restoreItem(e.id)} />
        ))}
      </ul>
    </div>
  );
}

function HistoryRow({ entry: e, node, scheme, onRestore }: { entry: DeletedEntry; node: DiskNode | undefined; scheme: "light" | "dark"; onRestore: () => void }) {
  const c = categories[scheme][e.category];
  const when = new Date(e.at);
  const today = when.toDateString() === new Date().toDateString();
  const time = today ? when.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }) : when.toLocaleDateString();
  return (
    <li className="history-row">
      <span className="tile-badge history-badge" style={{ background: c.accent, boxShadow: `0 4px 10px ${c.shadow}` }}>
        <Icon name={node ? iconFor(node) : "file"} tone="white" size={16} />
      </span>
      <span className="history-main">
        <span className="history-title">
          <span className="history-name">{e.name}</span>
          <span className="history-kind" style={{ color: c.ink, background: c.to, borderColor: c.border }}>
            {t(`history.what.${e.what}`)}
          </span>
        </span>
        <MiddleEllipsis text={e.path} className="history-path" />
      </span>
      <span className="history-side">
        <Bytes value={e.size} className="history-size" />
        <span className="text-caption text-muted tabular">{time}</span>
      </span>
      <span className="history-status">
        <span className={`chip ${e.status === "inTray" ? "chip-green" : e.status === "restored" ? "chip-violet" : "chip-protected"}`}>{t(`history.status.${e.status}`)}</span>
        {e.status === "inTray" && (
          <button type="button" className="link focus-ring" onClick={onRestore}>
            {t("tray.restore")}
          </button>
        )}
      </span>
    </li>
  );
}
