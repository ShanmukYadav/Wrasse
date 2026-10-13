// The See button count is the number of rows the list shows for the same scope.
// Run: ./dev.sh node --experimental-strip-types app/tests/scope.test.ts (or npm test)
import { LENS_BIT, type Disk, type DiskNode } from "../src/api/types.ts";
import type { CategoryId, DiskFilters } from "../src/design/tokens";
import { buttonBytes, buttonCount, scopeSummary, showAllScope, visibleRows } from "../src/ui/scope.ts";

const AI = LENS_BIT.aiModels;
const GREW = LENS_BIT.grew;
const LARGE = LENS_BIT.largeOld;
const APPS = LENS_BIT.apps;

function assert(cond: boolean, msg: string) {
  if (!cond) throw new Error(msg);
}

function leaf(id: number, name: string, parent: number, size: number, lenses: number, depth: number): DiskNode {
  return {
    id, name, path: name, size, parent, children: [], depth,
    category: "other", kind: "file", lastOpenedDays: 400, safety: "yourCall", why: "",
    lenses, deep: lenses, matchBytes: {}, grewBy: lenses & GREW ? size : 0, shade: 0,
  };
}

function dir(id: number, name: string, parent: number, depth: number, children: number[], category: DiskNode["category"] = "other"): DiskNode {
  return {
    id, name, path: name, size: 0, parent, children, depth,
    category, kind: depth === 0 ? "volume" : "dir", lastOpenedDays: 0, safety: "yourCall", why: "",
    lenses: 0, deep: 0, matchBytes: {}, grewBy: 0, shade: 0,
  };
}

/** Parent `deep` is the OR of descendant leaves, matching the scanner. */
function finish(nodes: DiskNode[]): Disk {
  for (let i = nodes.length - 1; i >= 0; i--) {
    const n = nodes[i];
    if (n.children.length === 0) {
      n.deep = n.lenses;
      continue;
    }
    let size = 0;
    let deep = n.lenses;
    for (const c of n.children) {
      size += nodes[c].size;
      deep |= nodes[c].deep;
    }
    n.size = size;
    n.deep = deep;
  }
  return { nodes, root: 0, capacity: 0, ghostId: -1, leafCount: nodes.filter((n) => n.children.length === 0).length };
}

const off: DiskFilters = { largeOld: false, grew: false };
const grew: DiskFilters = { largeOld: false, grew: true };
const largeOld: DiskFilters = { largeOld: true, grew: false };
const both: DiskFilters = { largeOld: true, grew: true };

// The reported bug: something grew elsewhere (6.7 GB), and AI Models itself grew nothing.
// A root-wide grew count is 1. The rows inside AI Models with Grew on are 0.
const bug = finish([
  dir(0, "Macintosh HD", -1, 0, [1, 2]),
  dir(1, "AI Models", 0, 1, [3], "aiModels"),
  dir(2, "Your Files", 0, 1, [4], "files"),
  leaf(3, "llama", 1, 6e9, AI, 2),
  leaf(4, "grew.bin", 2, 6.7e9, GREW, 2),
]);

assert(buttonCount(bug, 1, "aiModels", grew) === 0, "AI Models + Grew is empty");
assert(buttonBytes(bug, 1, "aiModels", grew) === 0, "AI Models + Grew bytes stay 0");
assert(visibleRows(bug, 1, "aiModels", grew).length === 0, "AI Models list has no grew row");
assert(buttonCount(bug, 0, "all", grew) === 1, "the whole disk still has the grew folder");
assert(buttonBytes(bug, 0, "all", grew) === 6.7e9, "the grew row's size is the file, not a second total");
assert(visibleRows(bug, 2, "all", grew).length === 1, "drilling into Your Files shows the grew file");
assert(buttonCount(bug, 2, "all", grew) === 1, "drilled grew count is that file");
assert(buttonCount(bug, 1, "aiModels", off) === 1, "AI Models without a filter shows its model");

// Both filters are AND. One child of each does not count; one leaf with both bits does.
const split = finish([
  dir(0, "Macintosh HD", -1, 0, [1]),
  dir(1, "Folder", 0, 1, [2, 3, 4]),
  leaf(2, "only-large", 1, 2e9, LARGE, 2),
  leaf(3, "only-grew", 1, 3e9, GREW, 2),
  leaf(4, "both", 1, 4e9, LARGE | GREW, 2),
]);
assert(visibleRows(split, 1, "all", both).join() === "4", "AND keeps only the leaf with both bits");
assert(buttonCount(split, 1, "all", both) === 1, "AND count is 1");
assert(buttonBytes(split, 1, "all", both) === 4e9, "AND bytes are that leaf");
assert(visibleRows(split, 1, "all", grew).sort().join() === "3,4", "Grew alone keeps both grew leaves");
assert(visibleRows(split, 1, "all", largeOld).sort().join() === "2,4", "Large & Old alone keeps both large leaves");
assert(buttonCount(split, 0, "all", both) === 1, "the parent folder counts once because one leaf has both");

// A match outside the folder you drilled into does not count.
const drilled = finish([
  dir(0, "Macintosh HD", -1, 0, [1, 2]),
  dir(1, "Has grew", 0, 1, [3]),
  dir(2, "Quiet", 0, 1, [4]),
  leaf(3, "grew-file", 1, 8e9, GREW, 2),
  leaf(4, "plain", 2, 1e9, 0, 2),
]);
assert(buttonCount(drilled, 2, "all", grew) === 0, "Quiet + Grew ignores the grew file next door");
assert(buttonCount(drilled, 0, "all", grew) === 1, "the disk root still shows the folder that grew");
assert(visibleRows(drilled, 1, "all", grew).join() === "3", "the grew folder shows its file");

// "Show all of Macintosh HD" keeps the filters and only widens to the disk root.
const shown = showAllScope(grew);
assert(shown.category === "all", "Show all uses category All");
assert(shown.filters.grew && !shown.filters.largeOld, "Show all keeps Grew and leaves Large & Old off");
assert(buttonCount(drilled, 2, "all", grew) === 0, "Quiet + Grew is empty before Show all");
const wideRows = visibleRows(drilled, 0, shown.category, shown.filters);
assert(wideRows.join() === "1", "Show all from Quiet shows the folder that grew");
assert(buttonCount(drilled, 0, shown.category, shown.filters) === wideRows.length, "Show all count matches those rows");
const fromModels = showAllScope(grew);
assert(buttonCount(bug, 1, "aiModels", fromModels.filters) === 0, "AI Models + Grew stays empty");
const bugWide = visibleRows(bug, 0, fromModels.category, fromModels.filters);
assert(bugWide.join() === "2", "Show all from AI Models shows where things grew");
assert(buttonCount(bug, 0, fromModels.category, fromModels.filters) === bugWide.length, "that widened count matches the rows");
const bothKept = showAllScope(both);
assert(bothKept.filters.largeOld && bothKept.filters.grew, "Show all keeps both filters");
assert(visibleRows(split, 0, bothKept.category, bothKept.filters).join() === "1", "widened AND still needs one leaf with both bits");

// Category and filter together: a grew file outside AI Models does not count.
const mixed = finish([
  dir(0, "Macintosh HD", -1, 0, [1, 2]),
  dir(1, "AI Models", 0, 1, [3, 4], "aiModels"),
  dir(2, "Apps", 0, 1, [5], "apps"),
  leaf(3, "grew-model", 1, 5e9, AI | GREW, 2),
  leaf(4, "still-model", 1, 2e9, AI, 2),
  leaf(5, "grew-app", 2, 9e9, APPS | GREW, 2),
]);
assert(visibleRows(mixed, 1, "aiModels", grew).join() === "3", "AI Models ∩ Grew is the model that grew");
assert(buttonCount(mixed, 1, "aiModels", grew) === 1, "that scope's count is 1");
assert(buttonBytes(mixed, 1, "aiModels", grew) === 5e9, "that scope's size is the model");
assert(buttonCount(mixed, 0, "aiModels", grew) === 1, "from the root, only the AI Models folder matches both");
assert(visibleRows(mixed, 0, "all", grew).sort().join() === "1,2", "Grew alone shows both folders");
assert(buttonCount(mixed, 2, "apps", largeOld) === 0, "Apps ∩ Large & Old is empty here");

const scopes: [Disk, number, CategoryId, DiskFilters][] = [
  [bug, 0, "all", off],
  [bug, 0, "all", grew],
  [bug, 0, "aiModels", grew],
  [bug, 1, "aiModels", grew],
  [bug, 1, "aiModels", off],
  [bug, 1, "aiModels", both],
  [bug, 2, "all", grew],
  [bug, 2, "all", largeOld],
  [split, 0, "all", both],
  [split, 1, "all", both],
  [split, 1, "all", grew],
  [split, 1, "all", largeOld],
  [split, 1, "junk", grew],
  [drilled, 0, "all", grew],
  [drilled, 0, showAllScope(grew).category, showAllScope(grew).filters],
  [drilled, 1, "all", grew],
  [drilled, 2, "all", grew],
  [bug, 0, showAllScope(grew).category, showAllScope(grew).filters],
  [bug, 0, showAllScope(both).category, showAllScope(both).filters],
  [drilled, 2, "apps", both],
  [mixed, 0, "all", grew],
  [mixed, 0, "aiModels", grew],
  [mixed, 0, "apps", grew],
  [mixed, 1, "aiModels", grew],
  [mixed, 1, "aiModels", largeOld],
  [mixed, 1, "apps", grew],
  [mixed, 2, "apps", grew],
  [mixed, 2, "apps", off],
];

for (const [disk, focus, category, filters] of scopes) {
  const rows = visibleRows(disk, focus, category, filters);
  const summary = scopeSummary(disk, focus, category, filters);
  const label = `${category} large=${filters.largeOld} grew=${filters.grew} @${focus}`;
  assert(buttonCount(disk, focus, category, filters) === rows.length, `count matches rows (${label})`);
  assert(summary.count === rows.length, `summary count matches rows (${label})`);
  const bytes = rows.reduce((sum, id) => sum + disk.nodes[id].size, 0);
  assert(buttonBytes(disk, focus, category, filters) === bytes, `bytes match rows (${label})`);
  assert(summary.bytes === bytes, `summary bytes match rows (${label})`);
}

console.log(`scope tests ok (${scopes.length} scopes)`);
