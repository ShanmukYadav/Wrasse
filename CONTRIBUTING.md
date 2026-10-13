# Contributing

Read [AGENTS.md](AGENTS.md) first: it holds the rules for this repository (safety, tokens,
platform code, and the rules for AI agents). [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) shows
how the pieces fit.

## Toolchain

Everything lives in this folder (`.toolchain/`, `.npm-cache/`); nothing is installed globally.
Run every command through `./dev.sh` (from `app/`, `../dev.sh`). Setup is in the README.

```sh
./dev.sh cargo test --workspace
cd app && ../dev.sh npm install && ../dev.sh npm run tauri dev
```

The browser preview (`cd app && ../dev.sh npm run dev`) runs on mock data.

## Checks

Enable the pre-commit hook once per clone: `git config core.hooksPath scripts/hooks`. It runs the
first three checks below. Run the rest before you finish:

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

If a change breaks a budget (PLAN.md §8), fix the change. Never raise a budget or remove design
without asking the owner.

## Commits

- Small commits, one step each, with a subject that says what changed for the user or the code.
- Never push; the owner pushes.
- `PLAN.md` is untracked on purpose. Never commit it.

## Tests and real data

Tests run on fixtures in temp folders. Never point a test, script or manual check at the owner's
real files; destructive actions are tested only on fixtures and dummy apps you create
(`scripts/dummy-app.sh`). Irreversible live tests are for the owner, before a release
([docs/live-tests.md](docs/live-tests.md)).
