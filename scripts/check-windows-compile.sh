#!/usr/bin/env bash
# Only macOS is built for now (PLAN.md). This keeps the platform-neutral crates compiling for
# Windows so the port doesn't start with a pile of breakage. Library and binary code only; warnings
# are errors.
#
# Checked:  wrasse-core, wrasse-fixtures, wrasse-cli
# Skipped (can't be checked from macOS):
#   - wrasse-app (app/src-tauri): Tauri's build script rejects the macOS-only `macos-private-api`
#     feature for a Windows target, and Windows resources need the Windows SDK.
#   - wrasse-core tests: they use Unix-only APIs (symlinks, permissions); they run on Windows CI.
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET=x86_64-pc-windows-msvc
if ! ./dev.sh rustup target list --installed | grep -qx "$TARGET"; then
  echo "Windows target missing; install it into the project toolchain:"
  echo "  ./dev.sh rustup target add $TARGET"
  exit 1
fi
RUSTFLAGS="-D warnings" ./dev.sh cargo check --quiet --target "$TARGET" \
  -p wrasse-core -p wrasse-fixtures -p wrasse-cli
echo "Windows compile check passed (wrasse-core, wrasse-fixtures, wrasse-cli)."
