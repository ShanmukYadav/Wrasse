// The rows a folder shows for the current category and filters, and the count on the See button.
// Both come from here so they stay the same number. Parent `deep` bits are OR'd across children,
// so two filters match only when one leaf carries every required bit.
import { LENS_BIT, type Disk } from "../../api/types.ts";
import type { CategoryId, DiskFilters, FilterId } from "../../design/tokens";

const EMPTY_KEY = {
  both: "scope.empty.both",
  grew: "scope.empty.grew",
  largeOld: "scope.empty.largeOld",
  category: "scope.empty.category",
} as const;

type EmptyMessageKey = (typeof EMPTY_KEY)[keyof typeof EMPTY_KEY];

export function scopeBits(category: CategoryId, filters: DiskFilters): number {
  let bits = 0;
  if (category !== "all") bits |= LENS_BIT[category];
  if (filters.largeOld) bits |= LENS_BIT.largeOld;
  if (filters.grew) bits |= LENS_BIT.grew;
  return bits;
}

export function filtersOn(filters: DiskFilters): boolean {
  return filters.largeOld || filters.grew;
}

/** True when `id` itself (one bit) or one leaf under it (several bits) carries every bit. */
export function matchesScope(disk: Disk, id: number, bits: number): boolean {
  if (bits === 0) return true;
  const n = disk.nodes[id];
  if (!n || n.kind === "ghost" || n.size <= 0) return false;
  if ((n.deep & bits) !== bits) return false;
  if ((bits & (bits - 1)) === 0) return true;
  return leafHasAll(disk, id, bits);
}

function leafHasAll(disk: Disk, id: number, bits: number): boolean {
  const n = disk.nodes[id];
  if (!n || n.kind === "ghost") return false;
  if ((n.deep & bits) !== bits) return false;
  if (n.children.length === 0) return (n.lenses & bits) === bits;
  for (const child of n.children) if (leafHasAll(disk, child, bits)) return true;
  return false;
}

/** Direct children of `focus` that the list shows for this scope. Unsorted. */
export function visibleRows(disk: Disk, focus: number, category: CategoryId, filters: DiskFilters): number[] {
  const parent = disk.nodes[focus];
  if (!parent) return [];
  const bits = scopeBits(category, filters);
  return parent.children.filter((id) => {
    const n = disk.nodes[id];
    return !!n && n.size > 0 && n.kind !== "ghost" && matchesScope(disk, id, bits);
  });
}

/** The See button's count: the number of rows `visibleRows` returns for the same scope. */
export function buttonCount(disk: Disk, focus: number, category: CategoryId, filters: DiskFilters): number {
  return visibleRows(disk, focus, category, filters).length;
}

/** Sum of those rows' sizes, so the button's size is what the list shows. */
export function buttonBytes(disk: Disk, focus: number, category: CategoryId, filters: DiskFilters): number {
  return visibleRows(disk, focus, category, filters).reduce((sum, id) => sum + disk.nodes[id].size, 0);
}

export function scopeSummary(disk: Disk, focus: number, category: CategoryId, filters: DiskFilters): { count: number; bytes: number } {
  const ids = visibleRows(disk, focus, category, filters);
  return { count: ids.length, bytes: ids.reduce((sum, id) => sum + disk.nodes[id].size, 0) };
}

export function emptyMessageKey(filters: DiskFilters): EmptyMessageKey {
  if (filters.largeOld && filters.grew) return EMPTY_KEY.both;
  if (filters.grew) return EMPTY_KEY.grew;
  if (filters.largeOld) return EMPTY_KEY.largeOld;
  return EMPTY_KEY.category;
}

export function activeFilters(filters: DiskFilters): FilterId[] {
  const on: FilterId[] = [];
  if (filters.largeOld) on.push("largeOld");
  if (filters.grew) on.push("grew");
  return on;
}

/** "Show all of Macintosh HD": category All on the disk root, with the same filters. */
export function showAllScope(filters: DiskFilters): { category: "all"; filters: DiskFilters } {
  return { category: "all", filters: { largeOld: filters.largeOld, grew: filters.grew } };
}

/** The folder name used in the empty sentence. The root uses the volume label passed in. */
export function scopeWhere(disk: Disk, focus: number, rootLabel: string): string {
  if (focus === disk.root) return rootLabel;
  return disk.nodes[focus]?.name || rootLabel;
}
