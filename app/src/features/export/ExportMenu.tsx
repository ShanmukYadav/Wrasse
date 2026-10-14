import { t } from "../../i18n";
import type { ExportTable } from "../../api";
import { exportTable, FORMATS } from "./exports";

/** "Export…" as a compact select: picking a format saves the report to Downloads. */
export function ExportMenu({ build, label }: { build: () => ExportTable; label: string }) {
  return (
    <select
      className="select focus-ring"
      aria-label={label}
      value=""
      onChange={(e) => {
        const f = FORMATS.find((x) => x.id === e.target.value);
        if (f) exportTable(build(), f.id);
      }}
    >
      <option value="" disabled>
        {t("export.button")}…
      </option>
      {FORMATS.map((f) => (
        <option key={f.id} value={f.id}>
          {f.label}
        </option>
      ))}
    </select>
  );
}
