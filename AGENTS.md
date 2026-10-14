# Instructions for AI coding agents

**PLAN.md is the source of truth.** Read it before changing anything. If something in it is wrong, fix PLAN.md and say so in your phase report.

## Non-negotiable rules

1. **Work phase by phase** (PLAN.md §11). Implement on macOS first, then Windows parity. Stop and report at each checkpoint; do not start the next phase until both checkpoints pass and CI is green on both OSes.
2. **All deletion goes through `safety::remove` or `safety::delegate`** in `wrasse-core`. Never add another path. `remove_file`, `remove_dir*`, `fs::rename`, `std::process::Command` and `ShellExecute*` are allowed only in `safety/`, `quarantine/` and `platform/`; `scripts/check-forbidden-apis.sh` enforces this in CI.
3. **Never touch the real home folder or the Windows registry in development.** Destructive code is tested only against `wrasse-fixtures` temp dirs, fakes and dry-run.
4. **Tokens only.** Every animation uses `app/src/motion/tokens.ts` (or the variants built from it), and every color, gradient, radius, shadow, blur and type size comes from `app/src/design/tokens.ts`. No ad-hoc values in components or CSS. Animate only `transform` and `opacity`.
5. `wrasse-core` has **no Tauri dependency**; the app and CLI are thin wrappers.
6. Platform-specific code lives in `crates/wrasse-core/src/platform/`: one file per area (`scanning`, `apps`, `protect`, `commands`, `power`, `ollama`), each a small function that dispatches with `cfg` to `macos/` or `windows/`, plus the `Platform` trait for labels (name, file manager, units). Outside `platform/`, only the choice of bundled rule pack (`rules/`) looks at the OS. The UI has no `if mac / if windows` logic beyond labels and shortcuts.
7. Every cleanup target is a TOML rule with a test, or a `safety::delegate` action with a test.
8. Every user-facing string goes through `t()` in `app/src/i18n`.
9. Keep dependencies minimal; justify additions against the budgets in PLAN.md §8.
10. Verify OS APIs, paths and command flags against current documentation before relying on them.

## Live actions on the owner's files (agents)

Set by the owner on 2026-10-13, after a test file was moved to the tray and the tray emptied 24
seconds later, and no record says who did it.

- AI agents never empty the tray, run Delete now, or run any delegated action (emptying the Trash,
  Homebrew, Finder Move to Trash, `ollama rm`, `docker system prune`, `brew cleanup`, snapshot
  thinning, simulator deletion) outside fixture folders they created for a test. This holds for
  the app, the CLI and any script.
- Live actions on the owner's real files happen only when the owner clicks them in the app.
- Moving the owner's files to the tray, or restoring them, also needs the owner's explicit
  request for that step.
- Every audit-log line records its origin (`ui`, `cli`, `script`, `test`). Run scripts with
  `WRASSE_ORIGIN=script`.
- Reports state only what was verified (a command, its output, a measurement, a checksum), never
  what was assumed. If something wasn't checked, say so.

## Local toolchain

Everything lives inside this folder; nothing is installed globally. Run every build and run command through `./dev.sh`:

```sh
./dev.sh cargo test --workspace
cd app && ../dev.sh npm install && ../dev.sh npm run tauri dev
```

`./clean.sh` removes the dev app's data from `~/Library` (identifier `com.shanmukyadav.wrasse`).

## Git hook

Enable the local pre-commit checks once per clone: `git config core.hooksPath scripts/hooks`.
The hook runs the forbidden-API, SECURITY_AUDIT.md and Windows compile checks.

## Before you finish

```sh
scripts/check-forbidden-apis.sh
scripts/check-security-audit.sh   # SECURITY_AUDIT.md matches the code
scripts/check-windows-compile.sh  # core crates still compile for Windows (only macOS is built for now)
./dev.sh cargo fmt --all --check
./dev.sh cargo clippy --workspace --all-targets -- -D warnings
./dev.sh cargo test --workspace
(cd app && ../dev.sh npm run build)
scripts/check-idle-memory.sh             # macOS, Wrasse quit: first-minute peak ≤ 250 MB, idle ≤ 110 MB (untouched launches)
scripts/check-memory-regression.sh       # macOS, before a release: no more than 5 MB above the previous release
scripts/check-idle-memory.sh --full-scan # peak during a full scan ≤ 300 MB (throwaway data folder)
scripts/check-data-size.sh               # Wrasse's own data ≤ 200 MB
(cd app && ../dev.sh npm run tauri build) && scripts/check-bundle-size.sh   # .dmg ≤ 15 MB, app ≤ 40 MB
```

Budgets are in PLAN.md §8. If a change breaks one, fix the change; never raise a budget or remove
design without asking the owner.
