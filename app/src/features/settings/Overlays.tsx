import { Icon } from "../../design/icons/Icon";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { Bytes } from "../../shared/Bytes";
import { bytesText } from "../../lib/format";
import { t, type MessageKey } from "../../i18n";
import { duration, ease, spring } from "../../motion/tokens";
import { api, type DiskNode, type JunkMatch, type JunkReport } from "../../api";
import { useApp, type DeletedEntry, type Overlay } from "../../store/app";
import { iconFor } from "../../design/icons/semantic";
import { LargeOld } from "../large-old/LargeOld";
import { categories } from "../../design/tokens";
import { useScheme } from "../../lib/prefs";
import { MiddleEllipsis } from "../../shared/MiddleEllipsis";

export function Overlays() {
  const overlay = useApp((s) => s.overlay);
  const setOverlay = useApp((s) => s.setOverlay);
  useEffect(() => {
    if (!overlay) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      const s = useApp.getState();
      // The caution sheet is on top. Escape closes that first.
      if (s.confirmDelete || s.fileCaution) return;
      setOverlay(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [overlay, setOverlay]);

  return (
    <AnimatePresence>
      {overlay && overlay !== "design" && overlay !== "onboarding" && (
        <motion.div
          key="scrim"
          className="overlay-scrim"
          onClick={() => {
            const s = useApp.getState();
            if (s.confirmDelete || s.fileCaution) return;
            setOverlay(null);
          }}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1, transition: { duration: duration.base, ease: ease.out } }}
          exit={{ opacity: 0, transition: { duration: duration.base, ease: ease.inOut } }}
        >
          <motion.div
            role="dialog"
            aria-modal="true"
            aria-label={t(`overlay.${overlay}`)}
            className={`overlay-panel glass-css ${overlay === "history" || overlay === "junk" || overlay === "largeOld" ? "is-wide" : ""}`}
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
        {overlay === "junk" && <Junk />}
        {overlay === "largeOld" && <LargeOld />}
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
      <Setting title={t("settings.dryRun")} note={t("settings.dryRunNote")}>
        <Switch checked={s.dryRun} label={t("settings.dryRun")} onChange={(on) => s.setDryRun(on)} />
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
  const trayCount = useApp((s) => s.tray.length);
  const [filter, setFilter] = useState<HistoryFilter>("all");
  const openTray = () => {
    useApp.getState().setOverlay(null);
    useApp.getState().setTrayOpen(true);
  };
  const trayLink = (
    <button type="button" className="btn focus-ring" onClick={openTray}>
      <Icon name="tray" size={16} /> {t("tray.open")}
      {trayCount > 0 && <span className="text-muted"> · {t("tray.items", { count: trayCount })}</span>}
    </button>
  );
  if (entries.length === 0)
    return (
      <div className="flex flex-col items-start gap-3">
        <p className="text-label text-muted">{t("history.empty")}</p>
        {trayLink}
      </div>
    );
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
        {trayLink}
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

/** System Junk: what the junk rules found, grouped by rule, each with "Why is this safe?". Safe
 *  items start ticked, "your call" ones don't. Cleaning moves them to the tray (restorable). */
function Junk() {
  const [report, setReport] = useState<JunkReport | null>(null);
  const [off, setOff] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const dryRun = useApp((s) => s.dryRun);
  const { notify, scanDisk, requestDelete, setOverlay } = useApp.getState();
  const trashNode = useApp((s) => s.disk.nodes.find((n) => n.delegate && n.size > 0));
  useEffect(() => {
    void api.junk().then((r) => {
      setReport(r);
      setOff(new Set(r.matches.filter((m) => m.safety !== "safe").map((m) => m.path)));
    });
  }, []);
  if (!report) return <p className="text-label text-muted">{t("junk.loading")}</p>;
  const fmt = (b: number) => bytesText(b, units);
  const items = report.matches.filter((m) => !m.delegate);
  const groups = [...new Set(items.map((m) => m.ruleId))].map((id) => items.filter((m) => m.ruleId === id));
  const picked = items.filter((m) => !off.has(m.path));
  const pickedBytes = picked.reduce((s, m) => s + m.bytes, 0);
  const toggle = (paths: string[], on: boolean) =>
    setOff((prev) => {
      const next = new Set(prev);
      for (const p of paths) {
        if (on) next.delete(p);
        else next.add(p);
      }
      return next;
    });
  async function clean() {
    setBusy(true);
    const r = await api.removeJunk(picked).catch((e) => {
      notify(String(e));
      return null;
    });
    setBusy(false);
    if (!r) return;
    if (r.dryRun) return notify(t("dryRun.trayToast", { count: r.done, size: fmt(r.bytes) }));
    notify(t("junk.cleanedToast", { count: r.done, size: fmt(r.bytes) }));
    setOverlay(null);
    void scanDisk();
  }
  return (
    <div className="flex flex-col gap-3">
      {items.length === 0 && <p className="text-label text-muted">{t("junk.empty")}</p>}
      {report.held.map((h) => (
        <p key={h.ruleId} className="text-caption text-your-call">
          {t("junk.held", { name: h.ruleName, app: h.app })}
        </p>
      ))}
      <ul className="junk-groups">
        {groups.map((g) => (
          <JunkGroup key={g[0].ruleId} items={g} off={off} fmt={fmt} onToggle={toggle} />
        ))}
      </ul>
      {trashNode && (
        <div className="row">
          <span className="text-label">{t("junk.trash", { size: fmt(trashNode.size) })}</span>
          <button type="button" className="btn focus-ring" onClick={() => requestDelete([trashNode.id])}>
            {t("confirm.emptyTrashTitle")}
          </button>
        </div>
      )}
      {items.length > 0 && (
        <div className="flex items-center gap-3">
          <button type="button" className="btn btn-primary focus-ring" disabled={!picked.length || busy} onClick={() => void clean()}>
            <Icon name="tray" size={16} /> {t("junk.clean", { count: picked.length, size: fmt(pickedBytes) })}
          </button>
          <span className="text-caption text-muted">{dryRun ? t("confirm.dryRunNote") : t("caution.trayNote")}</span>
        </div>
      )}
    </div>
  );
}

function JunkGroup({ items, off, fmt, onToggle }: { items: JunkMatch[]; off: Set<string>; fmt: (b: number) => string; onToggle: (paths: string[], on: boolean) => void }) {
  const [open, setOpen] = useState(false);
  const first = items[0];
  const allOn = items.every((m) => !off.has(m.path));
  const total = items.reduce((s, m) => s + m.bytes, 0);
  return (
    <li className="junk-group">
      <div className={`junk-head ${allOn ? "is-on" : ""}`}>
        <button type="button" role="checkbox" aria-checked={allOn} aria-label={first.ruleName} className={`plan-tick focus-ring ${allOn ? "is-on" : ""}`} onClick={() => onToggle(items.map((m) => m.path), !allOn)}>
          {allOn && <Icon name="check" size={16} />}
        </button>
        <button type="button" className="junk-title focus-ring" onClick={() => setOpen(!open)} aria-expanded={open}>
          <span className="text-label text-strong">{first.ruleName}</span>
          <span className={`chip ${first.safety === "safe" ? "chip-green" : "chip-amber"}`}>{first.safety === "safe" ? t("safety.safe") : t("safety.yourCall")}</span>
          {first.custom && <span className="chip chip-protected">{t("junk.custom")}</span>}
          <span className="flex-1" />
          <span className="text-caption text-muted">{t("junk.count", { count: items.length })}</span>
          <span className="caution-size tabular">{fmt(total)}</span>
          <Icon name={open ? "chevronDown" : "chevronRight"} size={16} />
        </button>
      </div>
      <p className="text-caption text-muted junk-why">
        <span className="text-strong">{t("detail.whySafe")}</span> {first.explanation} <span className="junk-rule">{t("detail.rule", { id: first.ruleId })}</span>
      </p>
      {open && (
        <ul className="junk-items">
          {items.map((m) => (
            <li key={m.path} className={`junk-item ${!off.has(m.path) ? "is-on" : ""}`}>
              <button type="button" role="checkbox" aria-checked={!off.has(m.path)} aria-label={m.path} className={`plan-tick focus-ring ${!off.has(m.path) ? "is-on" : ""}`} onClick={() => onToggle([m.path], off.has(m.path))}>
                {!off.has(m.path) && <Icon name="check" size={16} />}
              </button>
              <MiddleEllipsis text={m.path} className="history-path" />
              <span className="caution-size tabular">{fmt(m.bytes)}</span>
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}
