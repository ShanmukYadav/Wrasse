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
  "start". Phase 3 is accepted. Don't start Phase 4 until the owner says "start".
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
- **Idle memory budget:** 140 MB on macOS (`scripts/check-idle-memory.sh`). Don't keep scans in
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
./dev.sh cargo test --workspace          # 74 tests in wrasse-core, all passing
(cd app && ../dev.sh npm run build)
scripts/check-idle-memory.sh             # with Wrasse quit
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
  Don't start Phase 4 until the owner says "start". Windows stays deferred.
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
wrasse-core / wrasse-fixtures / wrasse-cli, fmt, clippy `-D warnings`, **74 tests** in wrasse-core
(was 72), app `npm run build`, idle memory **118.0 MB** (budget 140).

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
- A file or model in Free Up Space whose scan date is over a year (`last_opened_days` > 365)
  stays in the plan and starts unticked. Large & Old still starts with nothing ticked, including
  rows marked "approx". Select all is an explicit tick.
- Before release, live-test the Homebrew cask uninstall route and the root-owned Finder Move to
  Trash route. Both are unit-tested only. Homebrew is not installed on this Mac; the Finder route
  needs an administrator password. The same note is in `docs/phase3-plan.md` §9 and PLAN.md Phase 6.
- `~/WrasseTest/big-file.bin` was restored from the tray (not purged) and is back at that path.
  The tray no longer lists it.

**Stop here.** Windows is deferred. Don't start Phase 4 until the owner says "start".

### Other open issues

- The dummy app stays installed (`~/Applications/WrasseDummy.app`, its leftovers, and the orphan)
  so the owner can try it in the app. `scripts/dummy-app.sh remove` deletes only those paths. Ask
  the owner first.
- Homebrew and the root-owned Finder route still need a live test before release (see above).


## Phase 4 and later

See PLAN.md §11 (Phase 4: AI models, diagnostics, developer junk; Phase 5: polish; Phase 6:
release). Plan each phase in `docs/phaseN-plan.md` with decisions and recommendations, and wait for
"start".
