#!/usr/bin/env bash
# Run any command with the project-local toolchain. Nothing here touches global state.
#   ./dev.sh cargo build
#   ./dev.sh npm install
#   source ./dev.sh   (to use the toolchain in your current shell)
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"

export RUSTUP_HOME="$ROOT/.toolchain/rustup"
export CARGO_HOME="$ROOT/.toolchain/cargo"
export npm_config_cache="$ROOT/.npm-cache"
# The GitHub CLI and its settings live in the toolchain too (gh keeps the token in the keychain).
export GH_CONFIG_DIR="$ROOT/.toolchain/gh/config"
export PATH="$CARGO_HOME/bin:$ROOT/.toolchain/node/bin:$ROOT/.toolchain/gh/bin:$PATH"

# Only exec when run directly with arguments, not when sourced.
if [[ "${BASH_SOURCE[0]}" == "$0" && $# -gt 0 ]]; then
  exec "$@"
fi
