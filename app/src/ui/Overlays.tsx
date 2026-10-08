import { Icon } from "../design/icons/Icon";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { Bytes } from "../components/Bytes";
import { t, type MessageKey } from "../i18n";
import { duration, ease, spring } from "../motion/tokens";
import { useApp, type Overlay } from "../store/app";

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
      {overlay && overlay !== "design" && (
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
            className="overlay-panel glass-css"
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

function Panel({ overlay }: { overlay: Exclude<Overlay, null | "design"> }) {
  const setOverlay = useApp((s) => s.setOverlay);
  const closeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => closeRef.current?.focus(), []);
  return (
    <div className="flex max-h-full flex-col gap-4">
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

function History() {
  const batches = useApp((s) => s.batches);
  const restoreTray = useApp((s) => s.restoreTray);
  return batches.length === 0 ? (
    <p className="text-label text-muted">{t("history.empty")}</p>
  ) : (
    <ul className="flex flex-col gap-1">
      {batches.map((b) => (
        <li key={b.id} className="row">
          <div className="flex flex-col">
            <span className="text-label text-strong">{t("tray.items", { count: b.items.length })}</span>
            <span className="text-caption text-muted">
              {new Date(b.at).toLocaleTimeString()} · {t(`history.${b.state}`)}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <Bytes value={b.items.reduce((s, i) => s + i.size, 0)} className="text-caption text-muted" />
            {b.state === "inTray" && (
              <button type="button" className="btn focus-ring" onClick={restoreTray}>
                {t("tray.restore")}
              </button>
            )}
          </div>
        </li>
      ))}
    </ul>
  );
}
