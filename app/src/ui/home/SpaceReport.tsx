import { t } from "../../i18n";
import { formatBytes } from "../../lib/format";
import { useApp } from "../../store/app";

function sizeText(bytes: number, units: "decimal" | "binary") {
  const f = formatBytes(bytes, units);
  return `${f.value} ${f.unit}`;
}

/** Expected versus actual free space. Dashed, like the snapshot tile. No animation. */
export function SpaceReportCard() {
  const report = useApp((s) => s.spaceReport);
  const disk = useApp((s) => s.disk);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const { requestDelete, requestEmptyTray, freeGhost } = useApp.getState();
  if (!report) return null;
  const fmt = (n: number) => sizeText(n, units);
  const trash = disk.nodes.find((n) => n.delegateAction === "emptyTrash" && n.size > 0);
  return (
    <section className="space-report" aria-label={t("space.title")}>
      {report.expectedAvailable !== undefined && (
        <p className="space-line">
          <span className="space-strong">{t("space.expected", { size: fmt(report.expectedAvailable) })}</span>
          <span className="text-muted">{t("space.actual", { size: fmt(report.available) })}</span>
          {report.gap > 0 && <span>{t("space.gap", { size: fmt(report.gap) })}</span>}
        </p>
      )}
      <p className="space-line text-muted">{t("space.purgeable", { size: fmt(report.purgeable) })}</p>
      <div className="space-actions">
        <Row label={t("space.trash", { size: fmt(report.trashBytes) })} action={trash ? t("space.emptyTrash") : undefined} onAction={trash ? () => requestDelete([trash.id]) : undefined} />
        <Row label={t("space.tray", { size: fmt(report.trayBytes) })} action={report.trayBytes > 0 ? t("space.emptyTray") : undefined} onAction={report.trayBytes > 0 ? requestEmptyTray : undefined} />
        {report.snapshotBytes > 0 && <Row label={t("space.snapshots", { size: fmt(report.snapshotBytes), count: report.snapshots.length })} action={t("home.ghostFree")} onAction={freeGhost} />}
      </div>
    </section>
  );
}

function Row({ label, action, onAction }: { label: string; action?: string; onAction?: () => void }) {
  return (
    <p className="space-line">
      <span>{label}</span>
      {action && onAction && (
        <button type="button" className="link focus-ring" onClick={onAction}>
          {action}
        </button>
      )}
    </p>
  );
}
