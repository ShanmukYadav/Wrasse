// Mock mode's copy of the core ranker (crates/wrasse-core/src/rank): same rules, so the browser
// preview behaves like the app. The app itself always ranks in the core.
import { attached, type Disk, type DiskNode, type FreeCandidate, type FreePlan, type FreeWhy } from "../api/types";

const RECENT_DAYS = 7;
const HAND_PICK = /^(~|\/Users\/[^/]+)\/(Documents|Desktop|Library\/Mobile Documents|Library\/CloudStorage)\//;

export function unusedFactor(days: number): number {
  if (days <= 0) return 0.1;
  if (days <= 30) return 0.1 + (0.4 * days) / 30;
  if (days < 180) return 0.5 + (0.5 * (days - 30)) / 150;
  return 1;
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
    if (c.lastUsedDays < RECENT_DAYS) heldRecent += c.bytes;
    else out.push(c);
  };
  for (const n of disk.nodes) {
    if (n.kind === "app") {
      if (!n.app || n.app.route.kind === "notRemovable" || !attached(disk, n.id)) continue;
      const parts = n.children.map((c) => disk.nodes[c]).filter((c) => allowed(disk, c) && (c.appBundle || (c.leftoverMeta && !c.leftoverMeta.defaultOff)));
      if (!parts.some((p) => p.appBundle)) continue;
      const bytes = parts.reduce((s, p) => s + p.size, 0);
      if (n.running) {
        heldRunning += bytes;
        continue;
      }
      push({ id: n.id, parts: parts.map((p) => p.id), bytes, lastUsedDays: n.lastOpenedDays, why: n.app.neverOpened ? "neverOpened" : "unusedApp", score: bytes * unusedFactor(n.lastOpenedDays) * 0.7 });
      continue;
    }
    if (n.children.length || !allowed(disk, n) || disk.nodes[n.parent].kind === "app") continue;
    const why: FreeWhy = n.orphan ? "appRemoved" : n.rebuildable || (n.ruleId && n.safety === "safe") ? "rebuilds" : n.size >= 1e9 && n.lastOpenedDays >= 180 ? "largeAndOld" : "unused";
    push({ id: n.id, parts: [n.id], bytes: n.size, lastUsedDays: n.lastOpenedDays, why, score: n.size * unusedFactor(n.lastOpenedDays) * (n.safety === "safe" ? 1 : 0.7) });
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
