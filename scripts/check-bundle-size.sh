#!/usr/bin/env bash
# Installer size budgets (PLAN.md §8): macOS .dmg < 20 MB, Windows NSIS .exe < 15 MB.
set -euo pipefail
cd "$(dirname "$0")/.."

bundle_dir=target/release/bundle
case "$(uname -s)" in
  Darwin) file=$(ls "$bundle_dir"/dmg/*.dmg | head -1); limit_mb=20 ;;
  MINGW*|MSYS*|CYGWIN*) file=$(ls "$bundle_dir"/nsis/*.exe | head -1); limit_mb=15 ;;
  *) echo "No installer budget for $(uname -s)"; exit 0 ;;
esac

bytes=$(wc -c < "$file" | tr -d ' ')
limit=$((limit_mb * 1024 * 1024))
echo "$(basename "$file"): $((bytes / 1024)) KB (budget ${limit_mb} MB)"
if (( bytes > limit )); then
  echo "Installer exceeds the ${limit_mb} MB budget."
  exit 1
fi
