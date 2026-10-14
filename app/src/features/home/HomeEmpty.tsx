import { api } from "../../api";
import { BrandMark } from "../../design/brand/BrandMark";
import { t } from "../../i18n";
import { useApp } from "../../store/app";

/** Home with nothing to show yet: before the first scan, or after a scan that failed. */
export function HomeEmpty() {
  const scanError = useApp((s) => s.scanError);
  const fullDiskAccess = useApp((s) => s.fullDiskAccess);
  const startScan = useApp((s) => s.startScan);
  return (
    <div className="home-empty" role="status">
      <BrandMark size={56} />
      <h2 className="text-title">{scanError ? t("home.emptyErrorTitle") : t("home.emptyTitle")}</h2>
      <p className="text-label text-muted home-empty-body">{scanError ? t("home.emptyErrorBody", { reason: scanError }) : t("home.emptyBody")}</p>
      <button type="button" className="btn btn-primary focus-ring" onClick={startScan}>
        {scanError ? t("home.emptyRetry") : t("home.emptyScan")}
      </button>
      {fullDiskAccess === false && <AccessNote />}
    </div>
  );
}

/** Full Disk Access is off: some folders can't be read. A line and a way to fix it. */
export function AccessNote() {
  return (
    <p className="text-caption text-muted access-note">
      {t("home.accessOff")}{" "}
      <button type="button" className="link focus-ring" onClick={() => void api.openFullDiskAccessSettings()}>
        {t("home.accessOpen")}
      </button>
    </p>
  );
}
