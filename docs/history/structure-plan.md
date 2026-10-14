# Codebase audit and structure plan

Status: **approved 2026-10-13; in progress.** Steps done are listed in `git log`. Audited on
2026-10-13 at commit `d90e17f`. Every finding below was checked with a
tool or by reading the code; where a finding is an inference, it says so.

Tools run:
- `knip@5` (unused TS files, exports, types, dependencies)
- `cargo machete` (unused Rust dependencies: none found)
- `cargo clippy --workspace --all-targets -D warnings` (clean: no dead-code warnings)
- `wc -l` (file sizes)
- `grep` (imports across boundaries, `unwrap`/`expect`, debug output, TODOs)

## 1. Dead code and clutter

### 1.1 Frontend (`knip`)

**Unused exports (21).** Each is either removed, or made private where it's used inside its own
file:
- `api/caution.ts` `isOsFolder`; `api/types.ts` `lensMatches`, `isAncestor`
- `command/commands.ts` `fuzzy`
- `design/tilePaint.ts` `fillImage`, `glowImage` (used only inside the file: make private)
- `design/tokens.ts` `typeScale`
- `lib/backend.ts` `getAppInfo`, `getGlassSupport`, `setGlassRegions`
- `mock/rank.ts` `unusedFactor`
- `motion/variants.ts` `screen`, `list`, `listItem`
- `store/app.ts` `visibleTileOf`, `DEFAULT_FREE_TARGET`, `whatItWas`
- `ui/home/gridLayout.ts` `tileWeights`; `ui/LargeOld.tsx` `LARGE_MIN_KEY`
- `ui/scope.ts` `FILTERS_OFF`, `scopeSummary`

**Unused exported types (15):** `Caution`, `Suggestion`, `PlatformKey`, `IconElement`, `IconNode`,
`ChipTone`, `CategoryStyle`, `GlassSupport`, `GlassRegion`, `ThemePref`, `MotionPref`,
`AuditEntry`, `Batch`, `Toast`, `EmptyMessageKey`. Remove the `export` keyword; keep the type
where the file uses it.

**"Unused file" `ui/scope.test.ts`:** a false positive. It's run with
`node --experimental-strip-types`. Move it under `app/tests/` and tell knip about it.

### 1.2 Leftovers found by reading

- **Glass (Phase 0.5 "Liquid Glass" prototype):**
  - `lib/backend.ts` `getGlassSupport`/`setGlassRegions` have no callers.
  - `app/src-tauri/src/glass/` (285 lines) still registers `glass_support` and `glass_regions`,
    and **installs a native view on the window at startup** (`glass::install`).
  - D1 uses CSS blur only. Remove the module, both commands and the frontend functions, and
    re-measure idle memory: a native view behind the web view may cost memory (inferred, not
    measured).
- **Mock data in the production bundle:** `api/index.ts` imports `mock/api` eagerly, so the fake
  disk, apps, files and duplicates (`mock/disk.ts`, `files.ts`, `rank.ts`) are parsed at every
  launch (checked: "Iceland Trip", "DaVinci Resolve" are in `dist/assets/index-*.js`). Load the
  mock with a dynamic `import()` only in mock mode. This is also an idle-memory candidate (the
  open Phase 5 item).
- **Stub module:** `crates/wrasse-core/src/startup/mod.rs` is one comment line (Startup Items is
  Phase 5). Remove it until that phase.
- **Phase docs:** `docs/phase3-plan.md` is a finished phase plan. Move it to
  `docs/history/phase3-plan.md` (kept for the decisions record).
- **Old prototype screens:** no old sidebar, Reef Map canvas or abandoned screens remain in the
  tree (checked with `git ls-files` and grep for `canvas`/`sidebar`). `ui/DesignPage.tsx`
  (hidden "design system" page) is reachable from ⌘K: keep it, or move it behind dev builds
  (owner's call).
- **No leftover debug code:** no `console.log`/`debugger` in `app/src`, and no
  TODO/FIXME/HACK markers in tracked files.

### 1.3 Rust

- `clippy -D warnings` is clean, so there are no unused private items. Unused **public** items
  aren't reported by rustc. Run `cargo +nightly udeps` or a `pub` → `pub(crate)` pass per module,
  as part of step R4 below.
- `cargo machete` (v0.9.2, installed in `.toolchain/`): **no unused dependencies** in any crate.

## 2. Target folder layout

```
crates/
  wrasse-core/        the engine: no UI, no Tauri; everything below is platform-neutral except platform/
    src/scan/         walk, tree, breakdown into categories, cache, model sizes (hf), weights
    src/safety/       the only removal entrypoints (remove, delegate), audit log, exclusions; removal tests
    src/quarantine/   the tray: manifest, store, restore, purge, crash recovery
    src/rules/        junk rule engine and validation (packs live in /rules)
    src/features/     one folder per feature: apps/, rank/ (Free Up Space), files/ (large, dupes),
                      llm/ (AI models), devjunk/, diagnostics/
    src/platform/     OS specifics behind small traits: macos/, windows/ (stub until Windows parity)
    src/error.rs      the crate's one error type
  wrasse-cli/         thin command-line wrapper over wrasse-core (checkpoints, scripts)
  wrasse-fixtures/    fake homes and dummy apps for tests; never touches real data
app/
  src-tauri/          thin command layer: one file per area (scan, cleanup, apps, files, models),
                      argument parsing and state only, no business logic
  src/
    api/              the only way the UI reaches the backend (tauri.ts real, mock/ for the browser)
    features/         UI by feature: home/ (map, tiles, sonar), apps/ (uninstall), cleanup/
                      (caution, selection, tray), free-space/, large-old/, models/, diagnostics/,
                      settings/ (overlays), onboarding/
    shared/           components used by several features (Bytes, Icon, MiddleEllipsis, Toast, Deferred)
    design/           tokens, theme application, icons, brand, tile paint (no components with logic)
    store/            app state (zustand), split by slice
    i18n/             strings
    lib/              pure helpers (format, prefs)
  tests/              scope.test.ts and future UI tests
rules/                junk rule packs per OS (TOML)
scripts/              checks (forbidden APIs, audit sync, Windows compile, memory, sizes, data)
                      and dev tools (dummy-app, memory tree)
docs/                 ARCHITECTURE.md, frontend.md, live-tests.md, history/
design/               locked visual references (D1-home.html)
```

Root keeps `README.md`, `SECURITY_AUDIT.md`, `CONTRIBUTING.md` (new), `AGENTS.md`,
`LICENSE`, `dev.sh`, `clean.sh`, and the Cargo workspace files. `PLAN.md` stays untracked, by the
owner's decision.

## 3. Architecture and boundaries

Intended flow:

```
UI component → store action → api/ (tauri.ts | mock) → Tauri command (thin)
  → wrasse-core feature → safety/quarantine (any change on disk) → platform (OS calls)
```

Places that break it today:
1. **`lib/backend.ts` calls Tauri directly** (`invoke` for `app_info`, glass, `dev_log`) outside
   `src/api/`. Move `appInfo` and `devLog` into `api/` and delete the glass calls (§1.2).
2. **Business logic in `src-tauri`:**
   - `cleanup.rs` (498 lines) builds Free Up Space limits, polls for an app to quit (10 s loop)
     and looks apps up again.
   - `files.rs` builds hand-pick places.
   - `scanning.rs` decorates the scan with snapshots and developer previews.
   Move each into wrasse-core (`apps::quit_and_wait`, `rank::limits_for`, `session::decorate`), so
   the commands only parse arguments and call one core function.
3. **The `Platform` trait covers only labels.** Everything else in `platform/mod.rs` (894 lines) is
   free functions dispatched by `cfg`. AGENTS.md rule 6 says OS code is behind the trait. Either:
   - split `platform/mod.rs` into small traits per area (fs, apps, power, commands) with macOS and
     Windows implementations; or
   - fix the rule to describe `cfg` dispatch.
   Recommended: per-area files with `cfg` dispatch (simpler, no dynamic dispatch), and update the
   rule.
4. **Tests that need forbidden APIs live in `safety/`** (`uninstall_tests.rs`, `dupes_tests.rs`)
   only because of the forbidden-API check. Move them to `crates/wrasse-core/tests/`, and let the
   check allow `tests/` explicitly.
5. **Design tokens:** colors and sizes come from `design/tokens.ts`. One exception: `index.css` has
   a few literal values in experiment-era rules (e.g. `rgba` in chip styles). List them and move
   them to tokens (step F5).

## 4. Consistency

- **Naming:** Rust follows snake_case modules and CamelCase types (ok). TS mixes `ui/` flat files
  with `ui/home/`; the feature folders in §2 fix that. Use one name per idea ("tray" everywhere,
  not "quarantine" in the UI and "tray" in strings), and keep `quarantine` only as the core
  module name.
- **Errors:** wrasse-core returns `io::Error` built ad hoc with messages, and the Tauri layer maps
  errors to `String`. Add one `wrasse_core::Error` enum (kinds: Refused(Reason), NotFound,
  Busy, Os(io::Error)), keep the message text, and map it once in the Tauri layer.
- **No `unwrap` in production paths:** checked. Outside tests the only `expect`s are:
  - Tauri's `run()` (acceptable);
  - 31 in `wrasse-cli` (argument handling: replace with error messages and exit codes).
- **Logging:** the audit log is the only structured log. Add a `log` facade (`tracing` or
  `log` + `oslog`) for diagnostics, and keep debug prints out of release builds (they're `cfg!(debug_assertions)` today).
- **Byte formatting:** 7 local `fmt` helpers in UI files (`Overlays`, `CautionSheet`,
  `UninstallSheet`, `CommandBar`, `LargeOld`, …). Use one `useFormatBytes()` hook in `lib/format`.
- **Files over ~400 lines to split:**

  | File | Lines | Split into |
  |---|---|---|
  | `scan/breakdown.rs` | 3153 | categories, apps, models, ai/hf, lenses/finish, tests |
  | `app/src/index.css` | 2317 | one CSS file per feature next to its components, plus base and tokens |
  | `safety/mod.rs` | 1442 | plan/check, remove, delegate, tests |
  | `rules/mod.rs` | 1177 | load/validate, find, tests |
  | `store/app.ts` | 932 | slices: map, cleanup, tray, plan, apps, prefs |
  | `devjunk/mod.rs` | 920 | discovery, previews, tests |
  | `platform/macos/apps.rs` | 910 | discovery, leftovers, orphans, fake probe and tests |
  | `platform/mod.rs` | 894 | per-area files (see §3.3) |
  | `scan/walk.rs` | 793 | walk, rescan, control/power, tests |
  | `llm/mod.rs` | 773 | discovery, duplicates, loaded models |
  | `rank/tests.rs` | 734 | fixture builder in a shared test helper |
  | `platform/macos/commands.rs` | 711 | read-only programs, delegated actions, quit |
  | `quarantine/mod.rs` | 613 | manifest, batch writer, restore/purge |
  | `llm/ollama.rs`, `llm/remove.rs`, `cleanup.rs` | ~500 each | as part of the moves above |

- **Comments:** module headers explain intent throughout. Add comments where logic is
  non-obvious and currently bare: grid layout DP (`gridLayout.ts`), breakdown claiming order,
  rescan merge.

## 5. Docs

- **New `docs/ARCHITECTURE.md`:** one diagram of crates, modules and the data flow above (scan →
  breakdown → UI; removal → safety → quarantine/delegate → audit), plus where each budget is
  enforced.
- **`HANDOFF.md` → `docs/status.md`:** current status, open items and measurements, updated per phase.
  Its rules move into AGENTS.md (most are there already).
- **New `CONTRIBUTING.md`:** toolchain (`dev.sh`), checks, commit style, the agent rules.
- **Keep at root:** README, SECURITY_AUDIT.md, CONTRIBUTING.md, AGENTS.md, LICENSE.
- **`docs/history/`:** finished phase plans (`phase3-plan.md`).

## 6. Refactor plan (small commits, no behavior change)

Every commit is verified by:
- all tests (`cargo test --workspace`, the scope test);
- all checks (forbidden APIs, audit sync, Windows compile, fmt, clippy `-D warnings`, `npm run
  build`, bundle size, data size);
- the memory budgets (`check-idle-memory.sh` 5 launches and `--full-scan`) for commits touching
  the UI or the scan;
- a screenshot comparison of the home map, Apps, Large & Old and the tray against the previous
  commit (pixel diff of Wrasse's own window), for UI commits.

**Frontend**
- F1. Remove unused exports and types (knip list), make `tilePaint` helpers private.
- F2. Move `appInfo`/`devLog` into `api/`; delete the glass calls.
- F3. Load the mock only in mock mode (dynamic import). Re-measure idle.
- F4. One `useFormatBytes()`; replace the 7 local helpers.
- F5. Move literal CSS values to tokens.
- F6. Create `features/` and `shared/`; move files one feature per commit (home, apps, cleanup,
  free-space, large-old, models, diagnostics, settings, onboarding), imports only.
- F7. Split `index.css` per feature (one commit per feature, same rules, same order).
- F8. Split `store/app.ts` into slices (same state shape and actions).
- F9. Move `scope.test.ts` to `app/tests/` and configure knip.

**Rust**
- R1. Remove `glass/` and its commands (after F2). Re-measure idle.
- R2. Remove the `startup` stub; move `docs/phase3-plan.md` to `docs/history/`.
- R3. ~~Move the forbidden-API tests to `crates/wrasse-core/tests/` and adjust the check.~~
  **Dropped (2026-10-13).** Tried and reverted: the guard that stops delegated actions from
  running in tests is `#[cfg(test)]`, which an integration test doesn't set. The moved Finder test
  really moved its fixture app to the Trash. The tests stay in `safety/`.
- R4. `pub` → `pub(crate)` pass; delete what nothing uses.
- R5. Introduce `wrasse_core::Error`, one module per commit.
- R6. Move logic out of `src-tauri` into core functions (quit-and-wait, Free Up Space limits,
  scan decoration), one per commit.
- R7. Split the files over ~400 lines (table in §4), one file per commit, no logic changes.
- R8. Split `platform/mod.rs` into per-area files; update AGENTS.md rule 6.
- R9. Group features under `src/features/` (module paths re-exported to keep the public API).

**Docs**
- D1. `docs/ARCHITECTURE.md`. D2. `CONTRIBUTING.md`. D3. `HANDOFF.md` → `docs/status.md`.

Order: F1–F3, R1–R2 first (small, remove code), then R3–R6, then the moves (F6–F8, R7–R9), then
docs.
