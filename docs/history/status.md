# Status

Where Wrasse stands, what is open, and the last measurements. Updated at each phase checkpoint.
The rules live in AGENTS.md (local only); how to work is in
[CONTRIBUTING.md](../../CONTRIBUTING.md); the plan is `PLAN.md` (untracked, never committed).
Earlier detail is in `git log` and in this file's history (it was `HANDOFF.md` until
2026-10-13).

## Phases

- **Phases 0–3:** done and approved (Phase 2 tag `phase-2-approved`).
- **Phase 4** (AI models, diagnostics, developer junk): approved 2026-10-13.
- **Structure plan** ([structure-plan.md](structure-plan.md)): approved 2026-10-13 and carried
  out the same day, except the items under "Open" below.
- **Phase 5** (polish; [phase5-plan.md](phase5-plan.md)): built 2026-10-14, all budgets met;
  **waiting for the owner's approval** at the checkpoint.
- **Phase 6** (release, the live tests in [live-tests.md](../release/live-tests.md)) follows.

## Fixed on 2026-10-13

- **"Running" on apps was stale or wrong.** WhatsApp's notification extension keeps running
  after the app quits, and any process inside a bundle counted. Only the app's own executable
  (`Contents/MacOS`) counts now, for the chip, Quit and the uninstall check. The UI re-reads the
  state when the window comes back into view and every 5 s while it is visible.
- **Names were cut short.** Tile titles, item rows and chips now show full names: they wrap, and a
  tile that runs out of room drops a row (into "+N more"), then its chips.

## Open

- **Budgets are hard limits** (owner, 2026-10-13): no tolerance. Idle ≤ 100 MB with the window
  open is Phase 5's exit criterion. Met on 2026-10-14 (see below).
- **How memory is measured** changed on 2026-10-14: `check-idle-memory.sh` now waits for 30 s
  without keyboard or mouse input before each launch and repeats a launch that sees input.
  Earlier readings (105–108 MB idle, a 252 MB peak) were taken while the Mac was in use. Same
  conditions, the build before export/states/accessibility idled at 99–102 MB and the one after
  at 94–97 MB; which change saved it isn't measured separately (likely the free-space live region
  that VoiceOver tracked; inferred).
- **Budgets changed (owner, 2026-10-14, option B):** idle ≤ 110 MB and first-minute peak ≤ 250 MB
  on untouched launches; the peak during use is report-only; and before each release
  `scripts/check-memory-regression.sh` must show the new build within 5 MB of the previous release
  (median of 5 launches each, side by side). The animation library stays.
- **Regression check, 2026-10-14 evening** (`check-memory-regression.sh`; baseline: the Phase 5
  checkpoint build, app code as of 51edb74, since there's no earlier release): previous median peak
  233.0 MB, idle 106 MB; new median peak 234.2 MB, idle 106.1 MB. Passed. The new build's 5 launches:
  idle 104–107.1 MB (budget 110), peak 227–236 MB (budget 250).
- **Evening measurements, 2026-10-14:** Plain launches, untouched:
  104–108 MB idle, for today's build and for the Phase 5 build alike (side by side), while the
  same Phase 5 build measured 94–97 MB in the morning. What moves it is the Mac's state, not the
  code (inferred from the side-by-side runs; not pinned down further). WebKit's smaller "document
  viewer" cache setting made no difference (104–108 vs 107 MB) and was taken out.
- **Idle after use**: 114–140 MB before Phase 6. With the quiet restart (D7) the app returns to
  a fresh copy after 5 minutes in the background: measured after the restart (20 s in the test),
  105–108 MB, the same as a fresh launch today. Peaks while the scripted use overlaps the launch
  scan: 257–265 MB (budget 250).
- **Error type (plan step R5):** dropped by the owner (2026-10-13). Instead, `unwrap` and
  `expect` are denied outside tests in every crate (clippy).
- **Store slices (plan step F8):** types, boot and helpers are split out; the actions are still in
  one `create()` because they share closure helpers.
- **Test move (plan step R3):** dropped. The tests that need the delegated-action guard must stay
  inside the crate (see [structure-plan.md](structure-plan.md)).
- The dummy app stays installed (`~/Applications/WrasseDummy.app`, its leftovers and an orphan)
  so the owner can try it. `scripts/dummy-app.sh remove` deletes only those paths; ask first.
- Five test bundles (`WrasseStore….app`, `com.wrasse-test.store`, about 208 KB each) are in the
  owner's Trash from a test run on 2026-10-13. They are fixtures; the owner can delete them.

## Last measurements (2026-10-14, release build, this Mac, window 1280×820)

| Budget | Measured | Limit |
|---|---|---|
| Idle, window open | 95.0–96.1 MB (5 untouched launches, morning); 104–108 MB (evening) | 110 MB |
| Idle after use (two drill-ins, tray, Settings, a setting changed, 60 s untouched) | **114, 115, 140 MB (over)**; peaks 240–273 MB | 100 MB |
| First-minute peak | 176–207 MB | 250 MB |
| Full scan | 185 MB | 300 MB |
| Menu bar only | 10.0 MB | 40 MB |
| .dmg / app | 2.6 MB / 5 MB | 15 / 40 MB |
| Own data | 7 MB | 200 MB |

Tests: 129 in wrasse-core; the scope test (28 scopes).
