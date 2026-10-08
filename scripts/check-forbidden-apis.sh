#!/usr/bin/env bash
# Fails if destructive or process-spawning APIs appear outside the modules allowed to use them
# (PLAN.md §4.1). Only safety/, quarantine/ and platform/ in wrasse-core may delete, rename or spawn.
set -euo pipefail
cd "$(dirname "$0")/.."

pattern='remove_file|remove_dir|fs::rename|std::process::Command|process::Command|Command::new|ShellExecute'
allowed='^crates/wrasse-core/src/(safety|quarantine|platform)/'

violations=$(
  git ls-files --cached --others --exclude-standard '*.rs' \
    | grep -Ev "$allowed" \
    | xargs grep -nE "$pattern" 2>/dev/null || true
)

if [[ -n "$violations" ]]; then
  echo "Forbidden API used outside safety/, quarantine/ or platform/ (see PLAN.md §4.1):"
  echo "$violations"
  exit 1
fi
echo "Forbidden-API check passed."
