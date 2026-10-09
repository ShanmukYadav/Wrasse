import { mockMode } from "../api";
import { t } from "../i18n";
import { useApp } from "../store/app";

// Dev builds show a red DEV badge and start every launch in dry-run (PLAN.md §4.3 rule 13).
// With dry-run switched off the badge says LIVE: cleanups really move and delete files.
export function DevBadge() {
  const dev = useApp((s) => s.info?.dev);
  // Mock data is never real, whatever the switch says.
  const live = useApp((s) => !s.dryRun) && !mockMode();
  if (!dev) return null;
  return (
    <span title={t(live ? "app.liveTooltip" : "app.devTooltip")} className={`dev-badge ${live ? "is-live" : ""}`}>
      {t(live ? "app.live" : "app.dev")}
    </span>
  );
}
