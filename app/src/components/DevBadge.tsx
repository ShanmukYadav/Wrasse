import { t } from "../i18n";
import { useApp } from "../store/app";

// Dev builds show a red DEV badge and default to dry-run (PLAN.md §4.3 rule 13).
export function DevBadge() {
  const dev = useApp((s) => s.info?.dev);
  if (!dev) return null;
  return (
    <span
      title={t("app.devTooltip")}
      className="dev-badge"
    >
      {t("app.dev")}
    </span>
  );
}
