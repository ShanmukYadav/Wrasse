import { useEffect, useState } from "react";
import { api, type StartupItem } from "../../api";
import { t } from "../../i18n";
import { useApp } from "../../store/app";

const GROUPS: { scope: StartupItem["scope"]; title: "startup.yours" | "startup.allUsers" | "startup.system" }[] = [
  { scope: "user", title: "startup.yours" },
  { scope: "allUsers", title: "startup.allUsers" },
  { scope: "system", title: "startup.system" },
];

/** Startup Items (PLAN.md §6.10): what starts at login or with the Mac. Only your own agents can
 *  be turned off; they go to the tray, and restoring one turns it back on at the next login. */
export function Startup() {
  const [items, setItems] = useState<StartupItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const moveFilesToTray = useApp((s) => s.moveFilesToTray);
  const load = () =>
    void api
      .startupItems()
      .then((list) => {
        setItems(list);
        setError(null);
      })
      .catch((e) => setError(e instanceof Error ? e.message : String(e)));
  useEffect(load, []);

  if (error) return <p className="text-label text-muted">{t("startup.error", { reason: error })}</p>;
  if (!items) return <p className="text-label text-muted">{t("startup.loading")}</p>;
  if (items.length === 0) return <p className="text-label text-muted">{t("startup.empty")}</p>;

  const turnOff = async (item: StartupItem) => {
    setBusy(item.plist);
    await moveFilesToTray(() => api.disableStartupItem(item.plist));
    setBusy(null);
    load();
  };

  return (
    <div className="flex flex-col gap-4">
      {GROUPS.map(({ scope, title }) => {
        const group = items.filter((i) => i.scope === scope);
        if (!group.length) return null;
        return (
          <section key={scope} className="flex flex-col gap-1" aria-label={t(title)}>
            <h3 className="text-label text-strong">{t(title)}</h3>
            {scope !== "user" && <p className="text-caption text-muted">{t("startup.needsAdmin")}</p>}
            <ul className="flex flex-col">
              {group.map((item) => (
                <li key={item.plist} className="row">
                  <div className="flex min-w-0 flex-col">
                    <span className="text-label text-strong startup-name">{item.app ?? item.label}</span>
                    <span className="text-caption text-muted startup-name">
                      {item.label} · {item.enabled ? t("startup.on") : t("startup.off")}
                      {item.signed !== null && <> · {item.signed ? t("startup.signed") : t("startup.unsigned")}</>}
                    </span>
                  </div>
                  {item.canDisable && (
                    <button type="button" className="btn focus-ring" disabled={busy !== null} onClick={() => void turnOff(item)}>
                      {busy === item.plist ? t("startup.turningOff") : t("startup.turnOff")}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </section>
        );
      })}
      <p className="text-caption text-muted">{t("startup.note")}</p>
    </div>
  );
}
