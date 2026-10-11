# Wrasse handoff

You're taking over Wrasse, an offline, open-source disk cleaner and storage analyzer for macOS
(Windows later). This file describes the state of the repo as of 2026-10-10. Read it fully, then
read the files under "Read first".

## Read first

1. `PLAN.md`: the source of truth for what to build (phases, safety rules, budgets). It's
   **untracked on purpose** (listed in `.git/info/exclude`). Never commit or delete it.
2. `AGENTS.md`: repo rules and the checks to run before finishing.
3. `docs/phase3-plan.md`: the current phase's plan and the 11 decisions (all accepted with the
   recommended option).
4. `SECURITY_AUDIT.md`: what Wrasse may delete and how. It's kept in sync with the code by a check.
5. `docs/frontend.md`: every screen and flow in the UI.

## Rules the owner set (non-negotiable)

- **Never push.** Commit locally after each completed step, with clear messages grouped per phase
  (`Phase 3: …`). The owner pushes. Don't push the local branch `backup/before-history-clean` or
  the tag `phase-2-approved` either.
- **Run everything through `./dev.sh`** (from `app/`, `../dev.sh`). The toolchain (Rust, Node)
  lives in `.toolchain/`. Never install anything globally.
- **Mac only for now.** Windows parity is deferred. The pre-commit hook still runs a Windows
  `cargo check` on wrasse-core, wrasse-fixtures and wrasse-cli (see PLAN.md status note).
- **Stop and report at each phase checkpoint.** Don't start the next phase until the owner says
  "start". Phase 3 is accepted. The owner started Phase 4: do the notes in this file first, then
  stop before AI models, diagnostics, and developer junk.
- **Test only on dummy apps and files you create, never the owner's real apps or data.** Tests run
  in temp folders (guarded by `platform::remove_test_dir`). Tests can never reach a real delegated
  OS action (`#[cfg(test)]` stop in `platform::run_delegated`).
- **Safety architecture:** only `safety::remove` and `safety::delegate` remove anything. Code
  outside `crates/wrasse-core/src/{safety,quarantine,platform}/` may not delete, rename or start
  programs (`scripts/check-forbidden-apis.sh`). Tests that need those live under `safety/`
  (`safety/uninstall_tests.rs`, `safety/dupes_tests.rs`).
- **Design is locked:** D1 "Reef Blocks" light tile grid (`design/D1-home.html`, tokens in
  `app/src/design/tokens.ts`). Navigation is a plain crossfade (no zoom or stagger). Tiles use
  square-root sizing (biggest capped at 40%, a minimum size). Whole home categories have no tick.
- **Documents, Desktop and iCloud Drive:** hand-pick only, tray only, never preselected anywhere.
  iCloud-only (dataless) files are untouchable.
- **Dry run:** dev builds start every launch in dry run; while it's off the DEV badge is a red LIVE
  badge. `clean.sh` keeps the tray unless `--all` (asks first).
- **Idle memory budget:** 100 MB on macOS, and a first-minute peak of 200 MB, both for Wrasse's
  own tree only (`scripts/check-idle-memory.sh`, `scripts/wrasse-tree.py`). Don't keep scans in
  memory; Tauri commands read the saved scan or tree on demand.
- Repo-local git identity is the owner's noreply address (already configured). Commit messages
  end with your tool's attribution line if it adds one; no model names in commits.

## Checks (all must pass before a commit; the hook runs the first three)

```sh
scripts/check-forbidden-apis.sh
scripts/check-security-audit.sh
scripts/check-windows-compile.sh
./dev.sh cargo fmt --all --check
./dev.sh cargo clippy --workspace --all-targets -- -D warnings
./dev.sh cargo test --workspace          # 81 tests in wrasse-core, all passing
./dev.sh node --experimental-strip-types app/src/ui/scope.test.ts
(cd app && ../dev.sh npm run build)
scripts/check-idle-memory.sh             # with Wrasse quit; peak 200 MB, idle 100 MB
```

The hook is enabled with `git config core.hooksPath scripts/hooks` (already set). Clippy isn't in
the hook, so run it yourself.

## Layout

- `crates/wrasse-core`: everything that matters, no Tauri dependency.
  - `scan/` (parallel walk, tree, breakdown into categories, cache, FSEvents rescans)
  - `safety/` (prepare → plan, remove re-checks every item at the last moment, delegate, audit log, never-touch list)
  - `quarantine/` (the tray: crash-safe manifest, restore, purge after 7 days)
  - `rules/` (junk rule packs, `rules/macos/junk.toml`)
  - `apps/` (app model, uninstall routes)
  - `rank/` (Free Up Space ranker)
  - `files/` (large files, duplicates)
  - `platform/` (all OS specifics; macOS in `platform/macos/`: discovery, Spotlight/LaunchServices FFI, commands)
- `crates/wrasse-cli`: thin CLI, used for checkpoints (`wrasse scan|clean|restore|tray|apps|uninstall|quit|free|large|dupes`).
- `crates/wrasse-fixtures`: fake homes and dummy apps for tests (`mac_home`, `mac_apps`).
- `app/src-tauri`: Tauri commands (`scanning.rs`, `cleanup.rs`, `files.rs`).
- `app/src`: React 19 + TS + Vite UI. `api/` is the only backend boundary: `tauri.ts` real, `mock/` for the browser preview (mock mode must mirror core rules, e.g. `mock/rank.ts`). State is in `store/app.ts` (zustand).
- `scripts/dummy-app.sh`: creates, lists or removes the Phase 3 dummy app on the real Mac.

## Status

- Phase 0 to 2: done and approved. Phase 2 was approved after a manual tray round trip of
  `~/WrasseTest`, where every file matched its SHA-256 (`~/WrasseTest-checksums.txt`).
- **Phase 3 (macOS): steps 1 to 7 done and accepted, with the follow-up fixes below.**
  The owner started Phase 4. The first step is the UI fixes and the idle-memory note below.
  Don't open AI models, diagnostics, or developer junk until that checkpoint is accepted.
  Windows stays deferred.
  - Done: app discovery (sources: folder, Homebrew cask, App Store, system; owner; Spotlight last
    use; leftovers in every Library place; shared and name-only leftovers start unticked; system
    Library items "Needs admin"); uninstaller (routes: tray, `brew uninstall --cask`, Finder Move
    to Trash; quit first; refuses while running); orphaned leftovers; the Free Up Space ranker in
    the core (freed now vs pending in tray); Large & Old and Duplicates (size → sampled BLAKE3 →
    full BLAKE3; you pick the copy to keep); UI wiring in D1.
  - Two bugs fixed along the way: "Smaller items" carried its folder's path and could have
    removed the whole folder (now `aggregate`, blocked); the ranker offered single files deep
    in ~/Library (now each source is taken whole, the Library only through junk rules).

### Phase 3 checkpoint (step 7): done, waiting for approval

Real-Mac dummy check with `scripts/dummy-app.sh` and `./target/release/wrasse` (2026-10-10):
- Discovery finds `~/Applications/WrasseDummy.app` (folder, user-owned, 6 bundle-id leftovers
  plus 1 name-only), and the orphan `com.wrasse-gone-test.app`.
- Uninstall while it was running was refused ("WrasseDummy is open; quit it first").
- `wrasse quit` closed it in 0.36 s. Quit is `NSRunningApplication terminate` (the Dock's polite
  quit). It returns once the request is sent, gives up after 10 s with "couldn't ask it to quit",
  and never calls `forceTerminate` or signals the app. The dummy is an accessory app
  (`NSApplicationActivationPolicyAccessory`) so that request can reach it.
- Dry run reported the tray route and removed nothing. Uninstall moved the bundle and the six
  ticked leftovers to the tray, left the name-only leftover and the orphan, and restore of those
  seven entries was byte-identical (12/12 SHA-256).
- The dummy is installed again, with its leftovers and the orphan, so it can be tried in the app.
  `scripts/dummy-app.sh remove` deletes only those paths. Ask the owner first.

Homebrew and Finder routes: unit tests only. Homebrew is not installed; a root-owned dummy needs
sudo. Neither was installed or run.

Checks on this Mac, all passing: forbidden-API, SECURITY_AUDIT sync, Windows `cargo check` of
wrasse-core / wrasse-fixtures / wrasse-cli, fmt, clippy `-D warnings`, **80 tests** in wrasse-core
(was 74), app `npm run build`, idle memory **122.0 MB** (budget 140).

Numbers carried from the earlier real-data runs (not re-measured this step): full scan with app
discovery ~2.5 s; duplicate search 221 files, 2.2 GB hashed, 20 groups, 0.76 s, 35 MB peak;
Free Up Space offers 0.43 GB, 9.57 GB short of 10 GB.

Bugs fixed in Phase 3: "Smaller items" could remove its whole folder (now `aggregate`, blocked);
the ranker offered single files deep in ~/Library (now each source is taken whole); quit and the
Finder AppleScripts could hang (quit no longer uses `osascript`; emptying the Trash and Finder
Move to Trash give `osascript` 2 minutes, then only that process is stopped).

Follow-up fixes accepted with the checkpoint:

- Large & Old files in Documents or Desktop open the full caution sheet (list, iCloud warning,
  "I understand"). A selection that is only in iCloud Drive keeps the one-line note. Duplicates
  keep that note too. Before each move, allocated size and modification time are checked against
  the scan (`WRSTREE3`); a file that changed is skipped and reported as changed since the scan.
- A 2-minute stop of emptying the Trash says "Emptying the Trash stopped after 2 minutes and may
  be partly done." Finder Move to Trash says "Moving it to the Trash stopped after 2 minutes and
  may be partly done." Both are failures, never success.
- A file or model in Free Up Space starts unticked only when its date is approximate (`approx`)
  and over a year (`last_opened_days` > 365). A known last-used date over a year stays ticked.
  Large & Old still starts with nothing ticked, including rows marked "approx". Select all is
  an explicit tick.
- Before release, live-test the Homebrew cask uninstall route and the root-owned Finder Move to
  Trash route. Both are unit-tested only. Homebrew is not installed on this Mac; the Finder route
  needs an administrator password. The same note is in `docs/phase3-plan.md` §9 and PLAN.md Phase 6.
- `~/WrasseTest/big-file.bin` was restored from the tray (not purged) and is back at that path.
  The tray no longer lists it.
- With a lens on, the list button is a filled accent button with the live count and size
  ("See 1 file · 629 MB →"). With All selected it stays "List & duplicates →". Choosing a lens
  plays that glow once (opacity and transform, about 600 ms); Reduce Motion skips it. Tiles
  that hold no match for the lens dim.
- A file is not an AI model from its extension alone. `.gguf` and `.safetensors` still have to
  carry their header. `.bin` and the other ambiguous extensions need a known header (GGUF,
  safetensors, PyTorch, ONNX) or a `config.json` in the same folder whose JSON object has
  `model_type` or `architectures`. A config without those keys, including `{}`, does not count.
  A zero-filled `.bin` is not a model, so it no longer wears the chip or adds to the AI Models count.
- A ticked box shows a check on the accent fill, and the row takes a light accent tint. That
  is the same control in Large & Old, Duplicates (the copy you keep), Free Up Space, System
  Junk and uninstall leftovers. Once something in Large & Old is ticked, the note says how
  many. The tray has no row checkbox; each item has Restore. Keyboard focus outlines the box,
  and Space toggles it.
- Categories and filters are separate. All, Junk, Apps, AI Models, and Dev are one choice and
  still open that category. Large & Old and Grew are filters on the folder you're in: each
  toggles on its own (both can be on), they stay on when you change category, and they show as
  removable chips in the breadcrumb. Turning a filter on does not navigate. The See button
  counts the same rows the list shows for that category, those filters, and that folder
  (`app/src/ui/scope.test.ts`, 25 scopes). An empty scope says "Nothing here" on the button and
  explains it in the folder, with "Show all of Macintosh HD". The Large & Old overlay still
  opens from that button when the Large & Old filter is on. Re-checked on this Mac: forbidden-API,
  SECURITY_AUDIT sync, Windows `cargo check`, fmt, clippy `-D warnings`, 80 wrasse-core tests,
  the scope test, app `npm run build`, idle memory **120.0 MB** (budget 140).
- The lens bar shows one filter button. Large & Old and Grew appear only while that button is
  open. The button fills while a filter is on, and the breadcrumb chips still remove a filter.
- "Show all of Macintosh HD" keeps the active filters and only widens to the whole disk (category
  All, disk root, map). Grew stays on, so the map shows where things grew. The scope test covers
  that widen (`app/src/ui/scope.test.ts`, 28 scopes).
- Free Up Space starts a file or model unticked only when `approx` is set and `last_opened_days`
  is over 365. A known last-used date over a year stays ticked. The walk still marks file and
  model rows approximate: their age is the newer of access time and modification time, and
  Spotlight is not queried per file. Apps keep their Spotlight date and stay ticked.
- A `.bin` (and the other ambiguous extensions) beside `config.json` is a model only when that
  file is a JSON object with `model_type` or `architectures`. `{}` and a config such as
  `{"name":"not-a-model"}` do not count. A header still counts on its own. `.gguf` still needs
  its own header.
- Live check on `~/WrasseTest/changed-since-scan.bin` (created with `dd` of 500,000,000 bytes,
  then removed through the tray). The scan recorded 500,002,816 allocated bytes. After appending
  4,096 bytes the move was skipped as changed since the scan and the file stayed put. A rescan
  moved it to the tray and purge removed that entry only. Purge reported 516,780,032 bytes.
  `df -k` on `/System/Volumes/Data` showed Available 373,918,780 blocks before purge and
  374,423,448 after, a rise of 516,780,032 bytes. The other files in that folder, and
  `~/Documents/WrasseTest/old-big.bin`, were left in place.
- Re-checked on this Mac: forbidden-API, SECURITY_AUDIT sync, Windows `cargo check`, fmt,
  clippy `-D warnings`, 81 wrasse-core tests, the scope test (28 scopes), app `npm run build`,
  idle memory **119.0 MB** on all three samples (budget 140).
- Phase 4, first step. Every top-level block shows its share at the top right, including Junk
  ("18% · 4 items"). A short block shows the percent beside the size. The name ellipsizes; the
  percent does not.
- While a filter is on and the filter menu is closed, the button reads "Filters · 1" or
  "Filters · 2". Opening the menu hides that label. The breadcrumb chips still remove a filter.
- Dark mode no longer paints the lens bar, the filter menu, the selection bar, or the detail
  panel white. They use the dark glass, so All, Junk, Apps, AI Models and Dev stay readable.
  Tile shadows drop the white inset at night. The command bar shadow uses the dark pill shadow.
- The memory checker counts only Wrasse's tree: the app pid, child processes, WebKit helpers
  whose responsible pid is the app, and WebContent, GPU and Networking services launchd lists
  for that app (`scripts/wrasse-tree.py`). A WebKit process owned by another app is left out.
  From a shell, macOS names the shell as the responsible process and launchd as the parent, so
  the service list is what still ties the helpers to this app.
- Five cold launches with that checker, before the launch-memory cut, autoscan on. Simultaneous
  peak in the first minute: **493.3, 484.3, 523.3, 471.4, 478.4 MB**, each at 2–3 s. Idle at
  60/70/80 s: **117.0, 117.0, 153.0, 116.9, 118.0 MB**. The 153 MB launch was the app staying at
  its 70 MB scan high-water after WebKit had settled.
- Launch memory, measured before the cut with `vmmap` on Wrasse's own WebContent and GPU
  processes at about 2.5 s. The scan sent to the webview is the whole UI tree and it is small:
  `scan-cache.json` is 0.91 MB, 2,260 nodes; the root plus its children is 2.5 KB. There is no
  full-window canvas (one 300×150 measure canvas for middle-ellipsis). No icon atlas and no
  webfont files (`__FONT_DATA` was 2 KB; no Image I/O region). The spike was graphics surfaces.
  WebContent's physical footprint high-water was 394.5 MB, of which three surfaces were 64.5 MB
  each (4096×4128, the tile size WebKit uses for a `160vmax` sonar ring: about 1888 CSS pixels,
  3776 device pixels at 2×, rounded out to 4096). GPU high-water on that launch was 141.2 MB.
  JS heap inside WebKit Malloc was about 23 MB allocated.
- After the cut (rings laid out at 720px and scaled to the same visual diameter, tile rows and
  chips on the frame after the shell, backdrop blur and the sonar after the first paint, and a
  second `malloc_zone_pressure_relief` 500 ms after the scan drops its tree). Five cold launches,
  autoscan on, 2026-10-11. Simultaneous peaks: **114.9, 116.0, 116.0, 116.1, 116.1 MB**, each
  near 3 s (app 37, WebContent 26, GPU 47, Networking 6). Idle at 60/70/80 s: **75.5, 75.5,
  75.4, 75.5, 75.4 MB**, flat across the three samples, app at 36 MB every time. Both budgets
  pass (peak 400, idle 140). WebContent high-water on the diagnostic launch after the ring cap
  was 26.5 MB, with one 16.4 MB graphics surface. On 2026-10-11 the owner locked the macOS
  budgets to a first-minute peak of 200 MB and idle of 100 MB. One cold launch against those
  limits peaked at **116.1 MB** and idled at **75.5, 75.5, 75.5 MB**. The check passed. A later
  launch over either limit fails it.

**Stop here.** Windows is deferred. Phase 4 is started, and this step is the checkpoint. Don't
open AI models, diagnostics, or developer junk until the owner says to continue.

### Other open issues

- The dummy app stays installed (`~/Applications/WrasseDummy.app`, its leftovers, and the orphan)
  so the owner can try it in the app. `scripts/dummy-app.sh remove` deletes only those paths. Ask
  the owner first.
- Homebrew and the root-owned Finder route still need a live test before release (see above).


## Phase 4 and later

The owner said "start". This checkpoint is the polish notes and the UI fixes above. AI models,
diagnostics, and developer junk wait for the next "continue". See PLAN.md §11 (Phase 4: AI models,
diagnostics, developer junk; Phase 5: polish, including the first-run idle memory; Phase 6:
release). Plan each area in `docs/phaseN-plan.md` with decisions and recommendations, and stop
before the next area.
