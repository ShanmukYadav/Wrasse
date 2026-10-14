// Report export (PLAN.md §6.14): the app builds a table from what it shows; the core writes it to
// Downloads as Markdown, CSV or JSON and shows it in Finder.
import { api, type BigFile, type ExportFormat, type ExportTable } from "../../api";
import { t } from "../../i18n";
import { bytesText, type UnitSystem } from "../../lib/format";
import { useApp, type DeletedEntry } from "../../store/app";
import { planReason } from "../cleanup/planReason";

/** Today's local date, for the file name. */
function today(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** Writes the table and says where it went. */
export function exportTable(table: ExportTable, format: ExportFormat) {
  const { notify } = useApp.getState();
  void api
    .exportTable(table, format, today())
    .then((path) => notify(t("export.done", { name: path.split("/").pop() ?? path })))
    .catch((e) => notify(t("export.failed", { reason: e instanceof Error ? e.message : String(e) })));
}

const units = (): UnitSystem => useApp.getState().info?.units ?? "decimal";

/** The disk by category, and the biggest items in each. */
export function scanSummary(): ExportTable {
  const { disk } = useApp.getState();
  const root = disk.nodes[disk.root];
  // The same share the tiles show: snapshot space isn't part of what's used.
  const ghost = disk.nodes[disk.ghostId];
  const used = Math.max(1, root.size - (ghost?.parent === disk.root ? ghost.size : 0));
  const rows: string[][] = [];
  for (const id of root.children) {
    const c = disk.nodes[id];
    if (!c || c.size <= 0) continue;
    rows.push([c.name, "", bytesText(c.size, units()), `${Math.round((c.size / used) * 100)}%`, c.path]);
    const kids = c.children
      .map((k) => disk.nodes[k])
      .filter((k) => k && k.size > 0)
      .sort((a, b) => b.size - a.size)
      .slice(0, 5);
    for (const k of kids) rows.push([c.name, k.name, bytesText(k.size, units()), `${Math.round((k.size / used) * 100)}%`, k.path]);
  }
  return { title: t("export.summaryTitle"), columns: [t("export.col.category"), t("export.col.item"), t("export.col.size"), t("export.col.share"), t("export.col.path")], rows };
}

/** The Free Up Space plan on screen, ticked items first. */
export function freePlan(): ExportTable | null {
  const { plan, disk } = useApp.getState();
  if (!plan) return null;
  const rows = plan.ids.map((id) => {
    const n = disk.nodes[id];
    return [n?.name ?? "", bytesText(plan.bytes[id] ?? n?.size ?? 0, units()), n ? planReason(disk, n, plan.why[id]) : "", plan.off.includes(id) ? t("export.no") : t("export.yes"), n?.path ?? ""];
  });
  return { title: t("export.planTitle"), columns: [t("export.col.item"), t("export.col.size"), t("export.col.why"), t("export.col.ticked"), t("export.col.path")], rows };
}

/** History: everything removed this session. */
export function history(entries: DeletedEntry[]): ExportTable {
  const rows = entries.map((e) => [new Date(e.at).toLocaleString(), e.name, bytesText(e.size, units()), t(`history.status.${e.status}`), e.path]);
  return { title: t("export.historyTitle"), columns: [t("export.col.when"), t("export.col.item"), t("export.col.size"), t("export.col.status"), t("export.col.path")], rows };
}

/** Large & Old: the files listed. */
export function largeOld(files: BigFile[]): ExportTable {
  const rows = files.map((f) => [
    f.path.split("/").pop() ?? f.path,
    bytesText(f.bytes, units()),
    f.lastUsedDays === 0 ? t("detail.today") : t("detail.daysAgo", { count: f.lastUsedDays }),
    f.path,
  ]);
  return { title: t("export.largeTitle"), columns: [t("export.col.item"), t("export.col.size"), t("export.col.lastUsed"), t("export.col.path")], rows };
}

export const FORMATS: { id: ExportFormat; label: string }[] = [
  { id: "markdown", label: "Markdown" },
  { id: "csv", label: "CSV" },
  { id: "json", label: "JSON" },
];
