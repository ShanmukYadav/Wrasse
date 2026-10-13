// Mock mode's copy of the core ranker (crates/wrasse-core/src/rank): same rules, so the browser
// preview behaves like the app. The app itself always ranks in the core. Keep the two in step.
import { attached, type Disk, type DiskNode, type FreeCandidate, type FreePlan, type FreeWhy } from "../api/types";

const RECENT_DAYS = 7;
const HAND_PICK = /^(~|\/Users\/[^/]+)\/(Documents|Desktop|Library\/Mobile Documents|Library\/CloudStorage)\//;

function unusedFactor(days: number): number {
  if (days <= 0) return 0.1;
  if (days <= 30) return 0.1 + (0.4 * days) / 30;
  if (days < 180) return 0.5 + (0.5 * (days - 30)) / 150;
  return 1;
}

/** An approximate file or model date over a year starts unticked. A known date stays ticked. */
function approxOld(n: DiskNode): boolean {
  return n.approx === true && n.lastOpenedDays > 365 && (n.kind === "file" || n.kind === "model");
}

function allowed(disk: Disk, n: DiskNode): boolean {
  return (
    n.size > 0 &&
    n.safety !== "keep" &&
    !n.readOnly &&
    !n.aggregate &&
    !n.delegate &&
    n.kind !== "ghost" &&
    n.kind !== "volume" &&
    n.depth > 1 &&
    n.note !== "In iCloud only" &&
    !HAND_PICK.test(n.path) &&
    attached(disk, n.id)
  );
}

export function planFree(disk: Disk, target: number): FreePlan {
  const out: (FreeCandidate & { score: number })[] = [];
  let heldRecent = 0;
  let heldRunning = 0;
  const push = (c: FreeCandidate & { score: number }) => {
    if (c.lastUsedDays < RECENT_DAYS && c.why !== "rebuilds") heldRecent += c.bytes;
    else out.push(c);
  };
  const single = (n: DiskNode, why: FreeWhy) => ({
    id: n.id,
    parts: [n.id],
    bytes: n.size,
    lastUsedDays: n.lastOpenedDays,
    why,
    defaultOff: approxOld(n),
    score: n.size * unusedFactor(n.lastOpenedDays) * (n.safety === "safe" ? 1 : 0.7),
  });
  const stack = [disk.root];
  while (stack.length) {
    const n = disk.nodes[stack.pop()!];
    if (!attached(disk, n.id)) continue;
    const descend = () => stack.push(...n.children);
    if ((n.category === "system" || n.category === "other") && n.depth >= 1) continue;
    if (n.category === "apps") {
      if (n.kind === "app") {
        if (!n.app || n.app.route.kind === "notRemovable") continue;
        const parts = n.children.map((c) => disk.nodes[c]).filter((c) => allowed(disk, c) && (c.appBundle || (c.leftoverMeta && !c.leftoverMeta.defaultOff)));
        if (!parts.some((p) => p.appBundle)) continue;
        const bytes = parts.reduce((s, p) => s + p.size, 0);
        if (n.running) {
          heldRunning += bytes;
          continue;
        }
        push({ id: n.id, parts: parts.map((p) => p.id), bytes, lastUsedDays: n.lastOpenedDays, why: n.app.neverOpened ? "neverOpened" : "unusedApp", defaultOff: false, score: bytes * unusedFactor(n.lastOpenedDays) * 0.7 });
      } else if (n.orphan && n.kind === "dir" && n.depth >= 3) {
        const parts = n.children.map((c) => disk.nodes[c]).filter((c) => allowed(disk, c));
        const bytes = parts.reduce((s, p) => s + p.size, 0);
        if (parts.length) push({ id: n.id, parts: parts.map((p) => p.id), bytes, lastUsedDays: n.lastOpenedDays, why: "appRemoved", defaultOff: false, score: bytes * unusedFactor(n.lastOpenedDays) * 0.7 });
      } else descend();
    } else if (n.category === "junk") {
      if (n.ruleId && allowed(disk, n)) push(single(n, n.safety === "safe" ? "rebuilds" : "unused"));
      else descend();
    } else if (n.category === "developer") {
      if (n.rebuildable && allowed(disk, n)) push(single(n, "rebuilds"));
      else descend();
    } else if (n.category === "aiModels") {
      if (n.kind === "model" && allowed(disk, n)) push(single(n, n.size >= 1e9 && n.lastOpenedDays >= 180 ? "largeAndOld" : "unused"));
      else descend();
    } else if (n.category === "files") {
      if (n.children.length === 0) {
        if (allowed(disk, n)) push(single(n, n.size >= 1e9 && n.lastOpenedDays >= 180 ? "largeAndOld" : "unused"));
      } else descend();
    } else descend();
  }
  out.sort((a, b) => b.score - a.score || a.id - b.id);
  const items: FreeCandidate[] = [];
  let total = 0;
  for (const { score: _, ...c } of out) {
    if (total >= target) break;
    total += c.bytes;
    items.push(c);
  }
  return { target, items, total, shortfall: Math.max(0, target - total), heldRecent, heldRunning };
}
