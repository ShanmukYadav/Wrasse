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
  "start". Phase 3 isn't approved yet.
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
./dev.sh cargo test --workspace          # 68 tests in wrasse-core, all passing
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
- **Phase 3 (macOS): steps 1 to 6 committed; step 7 (checkpoint) in progress.**
  - Done: app discovery (sources: folder, Homebrew cask, App Store, system; owner; Spotlight last
    use; leftovers in every Library place; shared and name-only leftovers start unticked; system
    Library items "Needs admin"); uninstaller (routes: tray, `brew uninstall --cask`, Finder Move
    to Trash; quit first; refuses while running); orphaned leftovers; the Free Up Space ranker in
    the core (freed now vs pending in tray); Large & Old and Duplicates (size → sampled BLAKE3 →
    full BLAKE3; you pick the copy to keep); UI wiring in D1.
  - Two bugs fixed along the way: "Smaller items" carried its folder's path and could have
    removed the whole folder (now `aggregate`, blocked); the ranker offered single files deep
    in ~/Library (now each source is taken whole, the Library only through junk rules).

### Phase 3 checkpoint: what's left (step 7)

Real-Mac dummy check, so far, with `scripts/dummy-app.sh` and the CLI (`./target/release/wrasse`):
- ✓ Discovery finds `~/Applications/WrasseDummy.app` (folder, user-owned, 6 bundle-id
  leftovers plus 1 name-only), and the orphan `com.wrasse-gone-test.app` (60 days old).
- ✓ Uninstall moved the bundle and the ticked leftovers to the tray, left the name-only one, and
  restore was byte-identical (12/12 files).
- ✓ Uninstall is refused while the dummy runs ("WrasseDummy is open; quit it first").
- ✗ **Open issue 1:** `wrasse quit` / the Tauri `quit_app` command hung. `osascript … tell
  application id … to quit` waited with no timeout, probably on a macOS Automation permission
  prompt or because the dummy uses `NSApplicationActivationPolicyProhibited`. Fix:
  - give `platform::macos::commands::quit_app` a timeout (spawn, wait about 10 s, then kill
    `osascript` and report "couldn't ask it to quit");
  - consider `NSRunningApplication terminate` through FFI instead of AppleScript (no Automation
    prompt);
  - rebuild the dummy with `…ActivationPolicyAccessory` and retest.
  Never force-quit.

Then finish:
1. Rerun the dummy flow: `scripts/dummy-app.sh files | … shasum` → `open` the dummy →
   uninstall refused → quit works → `wrasse uninstall --dry-run` → `wrasse uninstall` → restore by
   entry ids (`wrasse tray`) → all checksums OK. The dummy is currently installed with its
   leftovers, plus the orphan dummy; the owner can try it in the app too.
2. Homebrew and Finder routes: covered by unit tests only (Homebrew isn't installed on this Mac;
   a root-owned dummy needs sudo). Say so in the report; don't install Homebrew.
3. Run all checks, including `scripts/check-idle-memory.sh` (the always-visible tray button and
   new overlays shouldn't push idle memory over 140 MB; measure it).
4. Report to the owner: test counts, what was verified on the real Mac, numbers (full scan with
   app discovery ~2.5 s; duplicate search on this Mac: 221 files, 2.2 GB hashed, 20 groups,
   0.76 s, 35 MB peak; Free Up Space on real data offers 0.43 GB, 9.57 GB short of 10 GB, since
   this Mac has little junk), the two bugs fixed, open issues, and that Windows is deferred.
   Then **stop** and wait for "start" before Phase 4.
5. Clean up: `scripts/dummy-app.sh remove` deletes only the dummy paths. Ask the owner first if
   they want to try it in the app.

### Other open issues

- `~/WrasseTest/big-file.bin` is in the owner's tray (the owner moved it after the verify). Don't
  touch it.
- The Large & Old list moves files by path through `remove_items`. Documents/Desktop items ask
  once (iCloud note); there's no full caution sheet there.
- PLAN.md §6.9 asks for "old" by Spotlight last use; scan dates are labeled "approx" when
  Spotlight has none. Fine as is.
- The `osascript`-based actions (empty Trash, Finder Move to Trash, quit) have no timeouts;
  apply the same fix as for quit.

## Phase 4 and later

See PLAN.md §11 (Phase 4: AI models, diagnostics, developer junk; Phase 5: polish; Phase 6:
release). Plan each phase in `docs/phaseN-plan.md` with decisions and recommendations, and wait for
"start".
