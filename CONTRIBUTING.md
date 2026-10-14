# Contributing

Thanks for helping. [docs/architecture.md](docs/architecture.md) shows how the pieces fit; the
rules below keep Wrasse safe to run on real disks.

## Rules

1. **All deletion goes through `safety::remove` or `safety::delegate`** in `wrasse-core`. File
   removal, renames and starting programs are allowed only in `safety/`, `quarantine/` and
   `platform/`; `scripts/check-forbidden-apis.sh` enforces this.
2. **Never point a test or script at your real home folder.** Destructive code is tested only on
   `wrasse-fixtures` temp folders, fakes, dummy apps (`scripts/dummy-app.sh`) and dry runs.
3. **Every cleanup target** is a TOML rule (`rules/`) with a test, or a `safety::delegate` action
   with a test, and [docs/security-audit.md](docs/security-audit.md) lists it.
4. **`wrasse-core` has no Tauri dependency.** The app and the CLI are thin wrappers.
5. **OS-specific code lives in `crates/wrasse-core/src/platform/`**, one file per area, dispatching
   with `cfg` to `macos/` (or `windows/`).
6. **Design tokens only.** Colors, sizes and type come from `app/src/design/tokens.ts`, durations
   and easings from `app/src/motion/tokens.ts`. Animate only `transform` and `opacity`.
7. **Every user-facing string goes through `t()`** in `app/src/i18n`.
8. **Keep dependencies minimal**, and keep within the budgets in
   [docs/architecture.md](docs/architecture.md#budgets-and-where-they-are-checked). If a change
   breaks one, fix the change.

## Toolchain

Everything lives in this folder (`.toolchain/`, `.npm-cache/`); nothing is installed globally.
Run every command through `./dev.sh` (from `app/`, `../dev.sh`). Setup is in the README.

```sh
./dev.sh cargo test --workspace
cd app && ../dev.sh npm install && ../dev.sh npm run tauri dev
```

The browser preview (`cd app && ../dev.sh npm run dev`) runs on mock data.
`scripts/clean.sh` removes the dev app's data from `~/Library` and keeps the tray.

## Checks

Enable the pre-commit hook once per clone: `git config core.hooksPath scripts/hooks`. It runs the
first three checks below. Run the rest before you open a pull request:

```sh
scripts/check-forbidden-apis.sh
scripts/check-security-audit.sh
scripts/check-windows-compile.sh
./dev.sh cargo fmt --all --check
./dev.sh cargo clippy --workspace --all-targets -- -D warnings
./dev.sh cargo test --workspace
(cd app && ../dev.sh npm test && ../dev.sh npm run build)
(cd app && ../dev.sh npx -y knip@5)
```

UI or scan changes also need the memory checks (Wrasse quit):
`scripts/check-idle-memory.sh --launches 5` and `scripts/check-idle-memory.sh --full-scan`.
Release builds need `scripts/check-bundle-size.sh` and `scripts/check-data-size.sh`.

## Pull requests

- Small commits, one step each, with a subject that says what changed for the user or the code.
- CI must be green.
- Irreversible live tests run by hand before a release
  ([docs/release/live-tests.md](docs/release/live-tests.md)).
