# Status

Where Wrasse stands, what is open, and the last measurements. Updated at each phase checkpoint.
The rules live in [AGENTS.md](../AGENTS.md); how to work is in
[CONTRIBUTING.md](../CONTRIBUTING.md); the plan is `PLAN.md` (untracked, never committed).
Earlier detail is in `git log` and in this file's history (it was `HANDOFF.md` until
2026-10-13).

## Phases

- **Phases 0–3:** done and approved (Phase 2 tag `phase-2-approved`).
- **Phase 4** (AI models, diagnostics, developer junk): built; **waiting for the owner's
  approval.**
- **Structure plan** ([structure-plan.md](structure-plan.md)): approved 2026-10-13 and carried
  out the same day, except the items under "Open" below.
- **Phase 5** (polish, menu-bar mode) and **Phase 6** (release, the live tests in
  [live-tests.md](live-tests.md)) follow Phase 4's approval.

## Fixed on 2026-10-13

- **"Running" on apps was stale or wrong.** WhatsApp's notification extension keeps running
  after the app quits, and any process inside a bundle counted. Only the app's own executable
  (`Contents/MacOS`) counts now, for the chip, Quit and the uninstall check. The UI re-reads the
  state when the window comes back into view and every 5 s while it is visible.
- **Names were cut short.** Tile titles, item rows and chips now show full names: they wrap, and a
  tile that runs out of room drops a row (into "+N more"), then its chips.

## Open

- **Idle memory: 105–108 MB against a 100 MB budget** (5 launches, 2026-10-13). Removing the
  unused native glass view and the mock data from the app bundle did not lower it (the build
  before them measured 104–106 MB in the same conditions). Most of the remainder is WebKit's own
  JavaScript heap (a blank page measures 83 MB). A Phase 5 item.
- **Error type (plan step R5):** not done. Errors are `io::Error` (refusals use
  `PermissionDenied`) and become strings at the Tauri boundary. A `wrasse_core::Error` enum would
  touch about 125 call sites for no visible change. Waiting for the owner's call.
- **Store slices (plan step F8):** types, boot and helpers are split out; the actions are still in
  one `create()` because they share closure helpers.
- **Test move (plan step R3):** dropped. The tests that need the delegated-action guard must stay
  inside the crate (see [structure-plan.md](structure-plan.md)).
- The dummy app stays installed (`~/Applications/WrasseDummy.app`, its leftovers and an orphan)
  so the owner can try it. `scripts/dummy-app.sh remove` deletes only those paths; ask first.
- Five test bundles (`WrasseStore….app`, `com.wrasse-test.store`, about 208 KB each) are in the
  owner's Trash from a test run on 2026-10-13. They are fixtures; the owner can delete them.

## Last measurements (2026-10-13, final release build of the day, this Mac)

| Budget | Measured | Limit |
|---|---|---|
| First-minute peak | 234–240 MB in 4 launches, 252 MB in 1 (0.8% over; the owner accepts ~1%) | 250 MB |
| Idle | 105–108 MB (5 launches) | 100 MB |
| Full scan | 228 MB | 300 MB |
| .dmg / app | 2.4 MB / 4 MB | 15 / 40 MB |
| Own data | 7 MB | 200 MB |

Tests: 118 in wrasse-core; the scope test (28 scopes).
