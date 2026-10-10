import { t } from "../i18n";
import { useApp } from "../store/app";
import { emptyMessageKey, scopeWhere } from "./scope";

/** Shown when the current folder has no row for the active category and filters. */
export function ScopeEmpty({ banner = false }: { banner?: boolean }) {
  const disk = useApp((s) => s.disk);
  const focus = useApp((s) => s.focus);
  const lens = useApp((s) => s.lens);
  const filters = useApp((s) => s.filters);
  const where = scopeWhere(disk, focus, t("home.title"));
  return (
    <div className={banner ? "scope-empty scope-banner" : "scope-empty scope-empty-list"}>
      <p>{t(emptyMessageKey(filters), { where, lens: t(`lens.${lens}`) })}</p>
      <button type="button" className="scope-empty-link focus-ring" onClick={() => useApp.getState().showAll()}>
        {t("scope.showAll", { name: t("home.title") })}
      </button>
    </div>
  );
}
