# Instructions for AI coding agents

**PLAN.md is the source of truth.** Read it before changing anything. If something in it is wrong, fix PLAN.md and say so in your phase report.

## Non-negotiable rules

1. **Work phase by phase** (PLAN.md §11). Implement on macOS first, then Windows parity. Stop and report at each checkpoint; do not start the next phase until both checkpoints pass and CI is green on both OSes.
2. **All deletion goes through `safety::remove` or `safety::delegate`** in `wrasse-core`. Never add another path. `remove_file`, `remove_dir*`, `fs::rename`, `std::process::Command` and `ShellExecute*` are allowed only in `safety/`, `quarantine/` and `platform/`; `scripts/check-forbidden-apis.sh` enforces this in CI.
3. **Never touch the real home folder or the Windows registry in development.** Destructive code is tested only against `wrasse-fixtures` temp dirs, fakes and dry-run.
4. **Tokens only.** Every animation uses `app/src/motion/tokens.ts` (or the variants built from it), and every color, gradient, radius, shadow, blur and type size comes from `app/src/design/tokens.ts`. No ad-hoc values in components or CSS. Animate only `transform` and `opacity`.
5. `wrasse-core` has **no Tauri dependency**; the app and CLI are thin wrappers.
6. Platform-specific code lives in `crates/wrasse-core/src/platform/` behind the `Platform` trait. The UI has no `if mac / if windows` logic beyond labels and shortcuts.
7. Every cleanup target is a TOML rule with a test, or a `safety::delegate` action with a test.
8. Every user-facing string goes through `t()` in `app/src/i18n`.
9. Keep dependencies minimal; justify additions against the budgets in PLAN.md §8.
10. Verify OS APIs, paths and command flags against current documentation before relying on them.

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
scripts/check-idle-memory.sh   # macOS, with Wrasse quit: first-minute peak under 200 MB, idle under 100 MB
```
