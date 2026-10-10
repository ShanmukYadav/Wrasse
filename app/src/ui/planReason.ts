import type { Disk, DiskNode, FreeWhy } from "../api";
import { t } from "../i18n";

/** Why the planner picked an item, in one short line. */
export function planReason(disk: Disk, n: DiskNode, why?: FreeWhy): string {
  if (why === "neverOpened") return t("plan.why.neverOpened");
  if (why === "unusedApp") return t("plan.why.unusedApp", { count: n.lastOpenedDays });
  if (why === "appRemoved") return t("plan.why.appRemoved");
  // A file inside a duplicated folder (one shard of a duplicate model) is a duplicate too.
  for (let d: DiskNode | undefined = n; d && d.parent >= 0; d = disk.nodes[d.parent]) {
    if (d.duplicateOf === undefined) continue;
    const o = disk.nodes[d.duplicateOf];
    return t("plan.why.duplicate", { name: o.name === d.name ? disk.nodes[o.parent].name : o.name });
  }
  if (n.rebuildable) return t("plan.why.rebuildable");
  if (n.category === "junk") return t("plan.why.junk");
  // Only an approximate file or model date says "approx". A known date over a year is just unused.
  if ((n.kind === "file" || n.kind === "model") && n.approx && n.lastOpenedDays > 365) return t("plan.why.approxOld", { count: n.lastOpenedDays });
  if (n.lastOpenedDays >= 180) return t("plan.why.unused", { count: n.lastOpenedDays });
  if (n.safety === "safe") return t("plan.why.safe");
  return t("plan.why.large");
}
