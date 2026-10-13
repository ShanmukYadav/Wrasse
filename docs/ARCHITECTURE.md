# Architecture

Wrasse is a Rust core with two thin front ends: a Tauri app (React UI) and a CLI. All logic,
and everything that touches the disk, lives in `crates/wrasse-core`.

```
app/src (React)                      crates/wrasse-cli
  component → store action             command
        ↓                                 ↓
  app/src/api  (tauri.ts | mock/)         │
        ↓                                 │
  app/src-tauri  (thin commands)          │
        └──────────────┬──────────────────┘
                       ↓
              crates/wrasse-core
   scan/ ──→ breakdown ──→ UI tree (UiDisk)
   features/ (apps, rank, files, llm, devjunk, diagnostics)
        ↓ every change on disk
   safety/ ──→ quarantine/ (the tray)   or   delegate (OS / tool, allowlist)
        ↓                                       ↓
   audit log                              platform/ (OS calls)
```

## Crates

| Crate | What it is |
|---|---|
| `wrasse-core` | Scanning, rules, cleanup, the tray. No Tauri dependency. |
| `wrasse-cli` | Command-line wrapper over the core, for checkpoints and scripts. |
| `wrasse-fixtures` | Fake homes and dummy apps in temp folders for tests. Never real data. |
| `app/src-tauri` | Tauri commands: parse arguments, hold state, call one core function. |

## wrasse-core modules

- `scan/`: parallel walk (`walk/`), the arena tree (`tree.rs`), the saved scan (`cache.rs`), and
  `breakdown/`, which turns the tree into what the UI shows: `types.rs` (the shapes), `sources.rs`
  (where tools keep things), `models.rs`, `dev.rs`, `app_nodes.rs`, and the builder in `mod.rs`.
- `safety/`: the only way anything is removed. `check.rs` vets each request (protected places,
  Never touch, scan match, owner, running apps) and re-checks it at the last moment; `remove.rs`
  moves to the tray or deletes; `delegate.rs` is the allowlist of OS and tool actions; `audit.rs`
  records every action with its origin.
- `quarantine/`: the tray. A crash-safe manifest per batch, restore, purge after the keep period.
- `rules/`: junk rule packs (`rules/macos/junk.toml`) and their validation.
- `features/`: `apps` (discovery, uninstall routes, quit), `rank` (Free Up Space), `files` (Large
  & Old, duplicates), `llm` (AI models), `devjunk` (developer caches and projects),
  `diagnostics` (snapshots, free space).
- `platform/`: every OS-specific call, one file per area (`scanning`, `apps`, `protect`,
  `commands`, `power`, `ollama`), dispatching with `cfg` to `macos/` (or `windows/`).
- `session.rs`: a scan for the app (cache, rescans) and its decoration with live data.

## UI (`app/src`)

- `api/`: the only way the UI reaches the backend. `tauri.ts` calls the core; `mock/` serves the
  browser preview and is loaded only in mock mode.
- `features/`: one folder per area (home, browse, cleanup, apps, large-old, models, command,
  settings, onboarding, design-page), each with its components and CSS.
- `shared/`: components used by several features.
- `design/` and `motion/`: tokens. Every color, size and duration comes from here.
- `store/`: zustand state (`app.ts`), its types, the launch sequence (`boot.ts`) and helpers.
- `i18n/`: every user-facing string.

## Safety boundaries, and where they are enforced

| Rule | Enforced by |
|---|---|
| Only `safety/`, `quarantine/` and `platform/` may delete, rename or start programs | `scripts/check-forbidden-apis.sh` (pre-commit hook) |
| SECURITY_AUDIT.md lists exactly what the code may run and delete | `scripts/check-security-audit.sh` (pre-commit hook) |
| Tests never run a real delegated action | `#[cfg(test)]` stop in `platform::run_delegated`; removal tests stay inside the crate (`safety/*_tests.rs`) |
| Tests touch only temp folders | `platform::remove_test_dir` refuses anything else |
| The core compiles for Windows | `scripts/check-windows-compile.sh` (pre-commit hook) |

## Budgets (PLAN.md §8), and where they are checked

| Budget | Check |
|---|---|
| .dmg ≤ 15 MB, app ≤ 40 MB | `scripts/check-bundle-size.sh` |
| Wrasse's own data ≤ 200 MB | `scripts/check-data-size.sh` |
| First-minute peak ≤ 250 MB, idle ≤ 100 MB | `scripts/check-idle-memory.sh` |
| Full scan ≤ 300 MB | `scripts/check-idle-memory.sh --full-scan` |
| Menu bar ≤ 40 MB | Phase 5 |
