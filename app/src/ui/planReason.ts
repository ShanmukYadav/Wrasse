import type { Disk, DiskNode } from "../api";
import { t } from "../i18n";

/** Why the planner picked an item, in one short line. */
export function planReason(disk: Disk, n: DiskNode): string {
  // A file inside a duplicated folder (one shard of a duplicate model) is a duplicate too.
  for (let d: DiskNode | undefined = n; d && d.parent >= 0; d = disk.nodes[d.parent]) {
    if (d.duplicateOf === undefined) continue;
    const o = disk.nodes[d.duplicateOf];
    return t("plan.why.duplicate", { name: o.name === d.name ? disk.nodes[o.parent].name : o.name });
  }
  if (n.rebuildable) return t("plan.why.rebuildable");
  if (n.category === "junk") return t("plan.why.junk");
  if (n.lastOpenedDays >= 180) return t("plan.why.unused", { count: n.lastOpenedDays });
  if (n.safety === "safe") return t("plan.why.safe");
  return t("plan.why.large");
}
