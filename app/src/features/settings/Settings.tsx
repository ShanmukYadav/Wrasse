import { motion } from "motion/react";
import { useEffect, useState } from "react";
import { api, type OwnData, type SettingsView } from "../../api";
import { t, type MessageKey } from "../../i18n";
import { bytesText } from "../../lib/format";
import { spring } from "../../motion/tokens";
import { useApp } from "../../store/app";

/** Settings (PLAN.md §7.6): look and motion, dry run, the tray, login, Never touch, custom rules,
 *  the audit log, and removing all of Wrasse's data. */
export function Settings() {
  const s = useApp();
  const [view, setView] = useState<SettingsView | null>(null);
  const [never, setNever] = useState<string[]>([]);
  useEffect(() => {
    void api.settings().then(setView).catch(() => setView(null));
    void api.neverTouchList().then(setNever).catch(() => setNever([]));
  }, []);
  const fail = (e: unknown) => s.notify(e instanceof Error ? e.message : String(e));
  const units = s.info?.units ?? "decimal";
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
      <Setting title={t("settings.keepDays")} note={t("settings.keepDaysNote")}>
        <select
          className="select focus-ring"
          aria-label={t("settings.keepDays")}
          value={view?.keepDays ?? s.keepDays}
          onChange={(e) => {
            const days = Number(e.target.value);
            void api
              .setKeepDays(days)
              .then((keepDays) => {
                setView((v) => (v ? { ...v, keepDays } : v));
                useApp.setState({ keepDays });
              })
              .catch(fail);
          }}
        >
          {KEEP_CHOICES.map((d) => (
            <option key={d} value={d}>
              {t("settings.days", { count: d })}
            </option>
          ))}
        </select>
      </Setting>
      <Setting title={t("settings.login")} note={view?.loginItem === null ? t("settings.loginUnavailable") : t("settings.loginNote")}>
        <Switch
          checked={!!view?.loginItem}
          label={t("settings.login")}
          disabled={!view || view.loginItem === null}
          onChange={(on) =>
            void api
              .setLoginItem(on)
              .then((loginItem) => setView((v) => (v ? { ...v, loginItem } : v)))
              .catch(fail)
          }
        />
      </Setting>
      <div className="flex flex-col gap-1">
        <span className="text-label text-strong">{t("settings.neverTouch")}</span>
        {never.length === 0 ? (
          <span className="text-caption text-muted">{t("settings.neverTouchEmpty")}</span>
        ) : (
          <ul className="flex flex-col">
            {never.map((path) => (
              <li key={path} className="row">
                <span className="text-caption settings-path">{path}</span>
                <button type="button" className="link focus-ring" onClick={() => void api.neverTouch(path, false).then(setNever).catch(fail)}>
                  {t("settings.neverTouchRemove")}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      <Setting title={t("settings.rules")} note={t("settings.rulesNote")}>
        <button type="button" className="btn focus-ring" onClick={() => void api.openRulesFolder().catch(fail)}>
          {t("settings.rulesOpen", { app: s.info?.fileManager ?? "Finder" })}
        </button>
      </Setting>
      <Setting title={t("overlay.audit")} note={t("settings.auditNote")}>
        <button type="button" className="btn focus-ring" onClick={() => s.setOverlay("audit")}>
          {t("settings.auditOpen")}
        </button>
      </Setting>
      <RemoveAll units={units} onError={fail} />
    </div>
  );
}

const KEEP_CHOICES = [1, 3, 7, 14, 30];

/** "Remove all Wrasse data": shows what goes, says "Not undoable", then removes and restarts. */
function RemoveAll({ units, onError }: { units: "decimal" | "binary"; onError: (e: unknown) => void }) {
  const [preview, setPreview] = useState<OwnData | null>(null);
  const [busy, setBusy] = useState(false);
  const ask = () => void api.ownDataPreview().then(setPreview).catch(onError);
  const run = () => {
    setBusy(true);
    void api
      .removeOwnData()
      .then((r) => {
        if (r.dryRun) {
          setBusy(false);
          setPreview(null);
          useApp.getState().notify(t("settings.removeAllDryRun", { size: bytesText(r.bytes, units) }));
          return;
        }
        try {
          for (const k of Object.keys(localStorage)) if (k.startsWith("wrasse.")) localStorage.removeItem(k);
        } catch {
          // Nothing saved in this browser, or storage is off.
        }
        window.location.reload();
      })
      .catch((e) => {
        setBusy(false);
        onError(e);
      });
  };
  return (
    <div className="settings-danger flex flex-col gap-2">
      <Setting title={t("settings.removeAll")} note={t("settings.removeAllNote")}>
        {!preview && (
          <button type="button" className="btn btn-danger focus-ring" onClick={ask}>
            {t("settings.removeAllButton")}
          </button>
        )}
      </Setting>
      {preview && (
        <div className="flex flex-col gap-2" role="alertdialog" aria-label={t("settings.removeAll")}>
          <p className="text-label">
            {t("settings.removeAllWhat", { size: bytesText(preview.bytes, units), folder: preview.folder })}
          </p>
          {preview.trayItems > 0 && (
            <p className="text-label text-strong">
              {t("settings.removeAllTray", { count: preview.trayItems, size: bytesText(preview.trayBytes, units) })}
            </p>
          )}
          <p className="text-caption text-strong">{t("detail.notUndoable")}</p>
          <div className="flex gap-2">
            <button type="button" className="btn btn-danger focus-ring" disabled={busy} onClick={run}>
              {t("settings.removeAllConfirm")}
            </button>
            <button type="button" className="btn focus-ring" disabled={busy} onClick={() => setPreview(null)}>
              {t("caution.cancel")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export function Switch({ checked, label, onChange, disabled = false }: { checked: boolean; label: string; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button type="button" role="switch" aria-checked={checked} aria-label={label} className={`switch focus-ring ${checked ? "is-on" : ""}`} disabled={disabled} onClick={() => onChange(!checked)}>
      <motion.span className="switch-knob" animate={{ x: checked ? 16 : 0 }} transition={spring.snappy} />
    </button>
  );
}

export function Segmented<T extends string>({ value, options, onChange, label }: { value: T; options: [T, MessageKey][]; onChange: (v: T) => void; label: string }) {
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
