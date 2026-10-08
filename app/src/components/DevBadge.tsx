import { t } from "../i18n";
import { useApp } from "../store/app";

// Dev builds show a red DEV badge and default to dry-run (PLAN.md §4.3 rule 13).
export function DevBadge() {
  const dev = useApp((s) => s.info?.dev);
  if (!dev) return null;
  return (
    <span
      title={t("app.devTooltip")}
      className="pointer-events-none fixed right-3 top-3 z-50 rounded-md bg-red-600 px-1.5 py-0.5 text-[10px] font-bold tracking-wider text-white"
    >
      {t("app.dev")}
    </span>
  );
}
