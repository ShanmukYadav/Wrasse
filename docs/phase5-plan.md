# Phase 5 plan: polish (macOS)

Status: **proposed 2026-10-13, waiting for the owner's "start".** Windows parity stays deferred.

## Exit criteria (all required)

1. **Idle ≤ 100 MB with the window open**, on 5 cold launches (`scripts/check-idle-memory.sh
   --launches 5`). Measured now: 105–108 MB.
2. **First-minute peak ≤ 250 MB on every launch** (hard limit). Measured now: 234–240 MB, and
   252 MB on 1 of 5.
3. **Menu bar only (window closed) ≤ 40 MB**, with a new check (`--menu-bar`).
4. Full scan ≤ 300 MB, .dmg ≤ 15 MB, app ≤ 40 MB, own data ≤ 200 MB still pass.
5. Every area below is done, with its empty, error and first-run states, reduced motion and
   keyboard/VoiceOver labels, and its tests.

## Order

Memory comes first, because every later feature is measured against it.

| # | Area | What exists | What Phase 5 adds |
|---|---|---|---|
| 1 | Idle and peak memory | Earlier cuts (sonar SVG, tile paint images, lazy screens) | See "Memory work" below |
| 2 | Settings | Theme, motion, dry run | Keep days (1–30), Never touch list, custom rules folder, launch at login, audit log link, Remove all Wrasse data |
| 3 | Command palette & shortcuts | ⌘K, ⌘Z | ⌘R rescan, ⌘, settings, ⌘⌫ add to cleanup, Space Quick Look (D6) |
| 4 | Startup items | A mock list in the UI | Real list and disable (D3) |
| 5 | Menu bar | Nothing | Free space, low-disk alert (< 10%), what grew this week, Scan, tray purge; menu-bar-only mode (D1, D2) |
| 6 | Export | Nothing | Scan summary, Free Up Space plan, Large & Old, History as Markdown, CSV or JSON (D4) |
| 7 | Empty, error, first-run states | Onboarding, some empty states | A designed state on every screen: no Full Disk Access, scan cancelled, tool not installed ("Ollama not found"), nothing to clean |
| 8 | Reduced motion | A setting and the system flag | Audit every animation; under Reduce, crossfades only |
| 9 | Accessibility | Focus rings, list fallback | Full keyboard path through every screen, VoiceOver labels on every control, checked with the accessibility tree |

## Memory work (exit criteria 1 and 2)

Idle is mostly WebKit's JavaScript heap: a blank page in Wrasse's window measures 83 MB in total.
The plan is to measure each step and keep only the ones that help:

1. **Measure the heap.** Take a heap snapshot in a dev build, and record what the scan tree, the
   store, React and the libraries hold.
2. **Smaller JavaScript.** Check what `motion`, React DOM and the app's own code cost at idle.
   (Checked 2026-10-14: the 185 kB "Icon" chunk is mostly React DOM, which the bundler put in
   that chunk; the icons are only the ~45 used.)
3. **Less data in the page.** The UI keeps the whole pruned tree in JavaScript. Keep only the
   levels on screen and fetch the rest on demand from the core, which already holds the tree.
4. **The 252 MB launch.** That launch's app process reached 46 MB at the scan (usually 21–30 MB).
   Find out why with 10 launches and a trace, then fix it.

If the four steps together don't reach 100 MB, I stop and report with the numbers before
trying anything that changes the design.

## Decisions for the owner

**D1. How the menu bar works.**
- **Native menu, same app (recommended).** A menu bar icon whose menu is native (free space,
  alerts, what grew, Scan, Open Wrasse). Closing the window releases the web view, so the 40 MB
  menu-bar-only budget is reachable.
- A small popover drawn with the web view. It looks like the app, but it keeps WebKit loaded,
  which costs about 80 MB.
- A separate helper app. It's the smallest, but it means a second app to sign and keep in sync.

**D2. Launch at login.** **Off by default, a switch in Settings, using macOS's own login-item
service (recommended).** This matches PLAN.md §6.12. The alternative is on by default.

**D3. Startup items: what Wrasse may change.**
- **Recommended:** list your LaunchAgents with their app, signature and state. Disable your own
  (`~/Library/LaunchAgents`) by unloading them and moving the plist to the tray. Restore loads it
  again. System agents and daemons are display-only ("Needs admin"). Apple's are never touched.
  Login items are shown only if macOS lets Wrasse read them without admin.
- The alternative is display-only for Phase 5.
- Either way, SECURITY_AUDIT.md and the delegate allowlist gain `launchctl bootout` and
  `bootstrap` for the user's own agents only, with tests on fixtures.

**D4. Where exports go.**
- **Save to Downloads and show the file in Finder (recommended).** No new dependency.
- A native Save dialog. It needs the Tauri dialog plugin (about 0.3 MB).

**D5. "Remove all Wrasse data" (Settings).** **Recommended:** it shows what will go (scan
cache, logs, settings, the tray) and the tray's size, says "Not undoable", and then deletes
Wrasse's own data folder through `safety`. The alternative keeps the tray and removes only cache
and settings.

**D6. Space for Quick Look.** **Recommended: the native Quick Look panel**, as Finder does,
macOS only. The alternative is to drop it and keep "Reveal in Finder".

**D7. Update check (Settings).** **Recommended: leave it to Phase 6 with the updater.** Wrasse
has no network code today. The alternative is a setting now that does nothing until Phase 6.

## Not in Phase 5

Windows parity; signing, notarization and the updater (Phase 6); the irreversible live tests
(Phase 6, [live-tests.md](live-tests.md)).
